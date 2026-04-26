use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use gix::{ObjectId, Repository};
use sha1::{Digest, Sha1};
use tracing::debug;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct DiscoveredRepository {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub git_dir: PathBuf,
    pub is_bare: bool,
    pub default_ref: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReferenceMetadata {
    pub name: String,
    pub target_hash: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct CommitMetadata {
    pub hash: String,
    pub tree_hash: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub committer_name: Option<String>,
    pub committer_email: Option<String>,
    pub message: Option<String>,
    pub committed_at_seconds: Option<i64>,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TreeEntryMetadata {
    pub path: String,
    pub object_hash: String,
    pub object_type: String,
    pub file_mode: String,
    pub size: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: String,
    pub blob_hash: String,
    pub language: Option<String>,
    pub size: Option<i64>,
    pub is_binary: bool,
}

#[derive(Debug, Clone)]
pub struct CommitSnapshot {
    pub commit: CommitMetadata,
    pub tree_entries: Vec<TreeEntryMetadata>,
    pub files: Vec<FileMetadata>,
}

pub fn discover_repositories(roots: &[PathBuf]) -> Result<Vec<DiscoveredRepository>> {
    let mut repos = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        if is_repo_root(root) {
            if seen.insert(root.to_path_buf()) {
                if let Some(repo) = build_repo_entry(root)? {
                    repos.push(repo);
                }
            }
            continue;
        }

        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            let path = entry.path();
            if path.file_name().and_then(|name| name.to_str()) == Some(".git") {
                let repo_root = path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| path.to_path_buf());
                if seen.insert(repo_root.clone()) {
                    if let Some(repo) = build_repo_entry(&repo_root)? {
                        repos.push(repo);
                    }
                }
            } else if entry.file_type().is_dir() && is_bare_repo_dir(path) {
                let repo_root = path.to_path_buf();
                if seen.insert(repo_root.clone()) {
                    if let Some(repo) = build_repo_entry(&repo_root)? {
                        repos.push(repo);
                    }
                }
            }
        }
    }

    Ok(repos)
}

pub fn open_repository(repo: &DiscoveredRepository) -> Result<Repository> {
    let repository = gix::open(&repo.git_dir)?;
    Ok(repository)
}

pub fn collect_refs(repo: &Repository) -> Result<Vec<ReferenceMetadata>> {
    let mut refs = Vec::new();
    let references = repo.references()?;
    for reference in references.all()? {
        let mut reference = reference.map_err(|err| anyhow!(err))?;
        let name = reference.name().as_bstr().to_string();

        let target_hash = reference
            .target()
            .try_id()
            .map(|id| id.to_string())
            .or_else(|| reference.peel_to_id_in_place().ok().map(|id| id.to_string()))
            .ok_or_else(|| anyhow!("reference {name} has no target"))?;

        let kind = if name.starts_with("refs/heads/") {
            "branch"
        } else if name.starts_with("refs/tags/") {
            "tag"
        } else if name.starts_with("refs/remotes/") {
            "remote"
        } else {
            "other"
        };

        refs.push(ReferenceMetadata {
            name,
            target_hash,
            kind: kind.to_string(),
        });
    }

    Ok(refs)
}

pub fn collect_commits(repo: &Repository, ref_targets: &[String]) -> Result<Vec<CommitSnapshot>> {
    let mut visited = HashSet::new();
    let mut snapshots = Vec::new();

    for target in ref_targets {
        let oid = ObjectId::from_hex(target.as_bytes())?;
        let revwalk = repo
            .rev_walk([oid])
            .sorting(gix::traverse::commit::simple::Sorting::BreadthFirst)
            .all()?;

        for info in revwalk {
            let info = info.map_err(|err| anyhow!(err))?;
            let oid = info.id;
            let oid_hex = oid.to_string();
            if !visited.insert(oid_hex.clone()) {
                continue;
            }

            let commit = info.object()?;
            let commit = commit.decode()?;

            let tree_hash = commit.tree().to_string();
            let parents = commit.parents().map(|id| id.to_string()).collect::<Vec<_>>();

            let author_name = Some(commit.author().name.to_string());
            let author_email = Some(commit.author().email.to_string());
            let committer_name = Some(commit.committer().name.to_string());
            let committer_email = Some(commit.committer().email.to_string());

            let message = {
                let msg = commit.message();
                let title = msg.title.to_string();
                if title.trim().is_empty() {
                    None
                } else {
                    Some(title)
                }
            };

            let committed_at_seconds = Some(commit.time().seconds);

            let mut tree_entries = Vec::new();
            let mut files = Vec::new();
            let tree_id = commit.tree();
            walk_tree(repo, &tree_id, Path::new(""), &mut tree_entries, &mut files)?;

            snapshots.push(CommitSnapshot {
                commit: CommitMetadata {
                    hash: oid_hex,
                    tree_hash,
                    author_name,
                    author_email,
                    committer_name,
                    committer_email,
                    message,
                    committed_at_seconds,
                    parents,
                },
                tree_entries,
                files,
            });
        }
    }

    Ok(snapshots)
}

fn walk_tree(
    repo: &Repository,
    tree_id: &ObjectId,
    base: &Path,
    tree_entries: &mut Vec<TreeEntryMetadata>,
    files: &mut Vec<FileMetadata>,
) -> Result<()> {
    let tree = repo.find_object(*tree_id)?.into_tree();
    let tree = tree.decode()?;

    for entry in tree.entries.iter() {
        let name = entry.filename.to_string();
        let path = base.join(&name);
        let path_str = path.to_string_lossy().to_string();
        let object_hash = entry.oid.to_string();
        let mode = entry.mode;
        let file_mode = mode.as_str().to_string();

        if mode.is_tree() {
            tree_entries.push(TreeEntryMetadata {
                path: path_str.clone(),
                object_hash: object_hash.clone(),
                object_type: "tree".to_string(),
                file_mode: file_mode.clone(),
                size: None,
            });

            let entry_id = entry.oid.to_owned();
            walk_tree(repo, &entry_id, &path, tree_entries, files)?;
        } else {
            tree_entries.push(TreeEntryMetadata {
                path: path_str.clone(),
                object_hash: object_hash.clone(),
                object_type: "blob".to_string(),
                file_mode: file_mode.clone(),
                size: None,
            });

            files.push(FileMetadata {
                path: path_str,
                blob_hash: object_hash,
                language: None,
                size: None,
                is_binary: false,
            });
        }
    }

    Ok(())
}

fn is_repo_root(path: &Path) -> bool {
    if is_bare_repo_dir(path) {
        return true;
    }

    let git_dir = path.join(".git");
    git_dir.is_dir()
}

fn is_bare_repo_dir(path: &Path) -> bool {
    path.join("HEAD").is_file() && path.join("objects").is_dir()
}

fn build_repo_entry(path: &Path) -> Result<Option<DiscoveredRepository>> {
    let git_dir = if path.join(".git").is_dir() {
        path.join(".git")
    } else {
        path.to_path_buf()
    };

    let repo = gix::open(&git_dir)?;
    let is_bare = repo.work_dir().is_none();
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo")
        .to_string();
    let default_ref = repo
        .head()
        .ok()
        .map(|head| head.name().as_bstr().to_string());

    let mut hasher = Sha1::new();
    hasher.update(path.to_string_lossy().as_bytes());
    let id = format!("{:x}", hasher.finalize());

    debug!(repo = %name, path = %path.display(), "discovered repository");

    Ok(Some(DiscoveredRepository {
        id,
        name,
        path: path.to_path_buf(),
        git_dir,
        is_bare,
        default_ref,
    }))
}

#[allow(dead_code)]
pub fn summarize_references(refs: &[ReferenceMetadata]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for reference in refs {
        *counts.entry(reference.kind.clone()).or_insert(0) += 1;
    }
    counts
}
