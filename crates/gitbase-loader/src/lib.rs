use std::path::PathBuf;

use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

use gitbase_db::metadata::{
    upsert_commit, upsert_commit_parent, upsert_file, upsert_ref, upsert_repository,
    upsert_tree_entry, CommitRecord, CommitParentRecord, FileRecord, RefRecord, RepositoryRecord,
    TreeEntryRecord,
};
use gitbase_git::{collect_commits, collect_refs, discover_repositories, open_repository};

#[derive(Debug, Default)]
pub struct SyncReport {
    pub repositories: usize,
    pub refs: usize,
    pub commits: usize,
    pub commit_parents: usize,
    pub tree_entries: usize,
    pub files: usize,
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
