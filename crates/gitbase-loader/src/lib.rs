use std::path::PathBuf;

use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

use gitbase_db::blobs::{
    fetch_blob_cache, fetch_blob_content, fetch_blob_locations, touch_blob, upsert_blob,
};
use gitbase_db::metadata::{
    upsert_commit, upsert_commit_parent, upsert_file, upsert_ref, upsert_repository,
    upsert_tree_entry, CommitRecord, CommitParentRecord, FileRecord, RefRecord, RepositoryRecord,
    TreeEntryRecord,
};
use gitbase_db::uast::{
    clear_uast_projections, fetch_uast_candidates, insert_uast_function, insert_uast_import,
    uast_cache_exists, upsert_uast_cache, UastFunctionRecord, UastImportRecord,
};
use gitbase_git::{
    collect_commits, collect_refs, discover_repositories, open_repository,
    open_repository_from_path, read_blob,
};
use gitbase_uast::{detect_language, parse_uast, Language as UastLanguage};

#[derive(Debug, Default)]
pub struct SyncReport {
    pub repositories: usize,
    pub refs: usize,
    pub commits: usize,
    pub commit_parents: usize,
    pub tree_entries: usize,
    pub files: usize,
}

#[derive(Debug, Clone)]
pub struct BlobHydrationConfig {
    pub max_blob_bytes: u64,
}

impl Default for BlobHydrationConfig {
    fn default() -> Self {
        Self {
            max_blob_bytes: 1_000_000,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BlobHydrationReport {
    pub cached_hits: usize,
    pub hydrated: usize,
    pub skipped_binary: usize,
    pub skipped_oversized: usize,
    pub missing: usize,
}

#[derive(Debug, Clone)]
pub struct UastIndexConfig {
    pub max_candidates: Option<i64>,
}

impl Default for UastIndexConfig {
    fn default() -> Self {
        Self { max_candidates: None }
    }
}

#[derive(Debug, Default, Clone)]
pub struct UastIndexReport {
    pub parsed: usize,
    pub skipped_cached: usize,
    pub skipped_missing_content: usize,
    pub skipped_unsupported_language: usize,
}

pub async fn sync_repositories(pool: &PgPool, roots: &[PathBuf]) -> Result<SyncReport> {
    let repos = discover_repositories(roots)?;
    let mut report = SyncReport::default();

    for repo in repos {
        info!(repo = %repo.name, path = %repo.path.display(), "sync repository");

        let repo_record = RepositoryRecord {
            id: repo.id.clone(),
            name: repo.name.clone(),
            path: repo.path.to_string_lossy().to_string(),
            default_ref: repo.default_ref.clone(),
            is_bare: repo.is_bare,
        };
        upsert_repository(pool, &repo_record).await?;
        report.repositories += 1;

        let handle = open_repository(&repo)?;
        let references = collect_refs(&handle)?;
        for reference in &references {
            let record = RefRecord {
                repository_id: repo.id.clone(),
                name: reference.name.clone(),
                target_hash: reference.target_hash.clone(),
                kind: reference.kind.clone(),
            };
            upsert_ref(pool, &record).await?;
            report.refs += 1;
        }

        let ref_targets = references
            .iter()
            .map(|reference| reference.target_hash.clone())
            .collect::<Vec<_>>();
        let commit_snapshots = collect_commits(&handle, &ref_targets)?;

        for snapshot in commit_snapshots {
            let commit = snapshot.commit;
            let commit_record = CommitRecord {
                repository_id: repo.id.clone(),
                hash: commit.hash.clone(),
                tree_hash: commit.tree_hash.clone(),
                author_name: commit.author_name.clone(),
                author_email: commit.author_email.clone(),
                committer_name: commit.committer_name.clone(),
                committer_email: commit.committer_email.clone(),
                message: commit.message.clone(),
                committed_at_seconds: commit.committed_at_seconds,
            };
            upsert_commit(pool, &commit_record).await?;
            report.commits += 1;

            for (index, parent) in commit.parents.iter().enumerate() {
                let parent_record = CommitParentRecord {
                    repository_id: repo.id.clone(),
                    commit_hash: commit.hash.clone(),
                    parent_hash: parent.clone(),
                    parent_index: index as i32,
                };
                upsert_commit_parent(pool, &parent_record).await?;
                report.commit_parents += 1;
            }

            for entry in snapshot.tree_entries {
                let entry_record = TreeEntryRecord {
                    repository_id: repo.id.clone(),
                    commit_hash: commit.hash.clone(),
                    path: entry.path.clone(),
                    object_hash: entry.object_hash.clone(),
                    object_type: entry.object_type.clone(),
                    file_mode: entry.file_mode.clone(),
                    size: entry.size,
                };
                upsert_tree_entry(pool, &entry_record).await?;
                report.tree_entries += 1;
            }

            for file in snapshot.files {
                let file_record = FileRecord {
                    repository_id: repo.id.clone(),
                    commit_hash: commit.hash.clone(),
                    path: file.path.clone(),
                    blob_hash: file.blob_hash.clone(),
                    language: file.language.clone(),
                    size: file.size,
                    is_binary: file.is_binary,
                };
                upsert_file(pool, &file_record).await?;
                report.files += 1;
            }
        }
    }

    Ok(report)
}

pub async fn hydrate_blobs(
    pool: &PgPool,
    repo_roots: &[PathBuf],
    blob_hashes: &[String],
    config: &BlobHydrationConfig,
) -> Result<BlobHydrationReport> {
    if blob_hashes.is_empty() {
        return Ok(BlobHydrationReport::default());
    }

    let mut report = BlobHydrationReport::default();

    let locations = fetch_blob_locations(pool, blob_hashes).await?;
    let mut by_hash: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for location in locations {
        by_hash
            .entry(location.blob_hash)
            .or_default()
            .push(location.repository_path);
    }

    for hash in blob_hashes {
        if let Some(cache) = fetch_blob_cache(pool, hash).await? {
            if cache.has_content {
                touch_blob(pool, hash).await?;
                report.cached_hits += 1;
                continue;
            }
        }

        let repo_paths = match by_hash.get(hash) {
            Some(paths) => paths,
            None => {
                report.missing += 1;
                continue;
            }
        };

        let mut hydrated = false;
        for repo_path in repo_paths {
            let repo_path_buf = PathBuf::from(repo_path);
            if !repo_roots.is_empty()
                && !repo_roots.iter().any(|root| repo_path_buf.starts_with(root))
            {
                continue;
            }

            let repo = open_repository_from_path(&repo_path_buf)?;

            let blob = read_blob(&repo, hash, config.max_blob_bytes)?;
            let size = blob.size as i64;
            if blob.size > config.max_blob_bytes {
                upsert_blob(pool, hash, size, false, None).await?;
                report.skipped_oversized += 1;
                hydrated = true;
                break;
            }

            if blob.is_binary {
                upsert_blob(pool, hash, size, true, None).await?;
                report.skipped_binary += 1;
                hydrated = true;
                break;
            }

            upsert_blob(pool, hash, size, false, blob.content.as_deref()).await?;
            report.hydrated += 1;
            hydrated = true;
            break;
        }

        if !hydrated {
            report.missing += 1;
        }
    }

    Ok(report)
}

pub async fn index_uast(pool: &PgPool, config: &UastIndexConfig) -> Result<UastIndexReport> {
    let mut report = UastIndexReport::default();
    let candidates = fetch_uast_candidates(pool, config.max_candidates).await?;

    for candidate in candidates {
        if uast_cache_exists(pool, &candidate.blob_hash).await? {
            report.skipped_cached += 1;
            continue;
        }

        let language = match detect_language(&candidate.path) {
            Some(language) => language,
            None => {
                report.skipped_unsupported_language += 1;
                continue;
            }
        };

        let content = fetch_blob_content(pool, &candidate.blob_hash).await?;
        let Some(content) = content else {
            report.skipped_missing_content += 1;
            continue;
        };

        let source = std::str::from_utf8(&content).unwrap_or("");
        let parsed = match language {
            UastLanguage::Go => parse_uast(UastLanguage::Go, source)?,
            UastLanguage::Rust => parse_uast(UastLanguage::Rust, source)?,
        };

        let uast_json = serde_json::to_value(&parsed)?;
        upsert_uast_cache(pool, &candidate.blob_hash, &parsed.language, &uast_json).await?;
        clear_uast_projections(pool, &candidate.blob_hash).await?;

        for function in parsed.functions {
            insert_uast_function(
                pool,
                &UastFunctionRecord {
                    blob_hash: candidate.blob_hash.clone(),
                    name: function.name,
                    start_line: function.start_line,
                    end_line: function.end_line,
                    signature: function.signature,
                },
            )
            .await?;
        }

        for import in parsed.imports {
            insert_uast_import(
                pool,
                &UastImportRecord {
                    blob_hash: candidate.blob_hash.clone(),
                    source: import.source,
                    target: import.target,
                },
            )
            .await?;
        }

        report.parsed += 1;
    }

    Ok(report)
}
