use crate::config::{Config, project_path};
use crate::policy::Policy;
use ignore::WalkBuilder;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct Project {
    pub path: PathBuf,
    pub git: bool,
    pub worktree: bool,
    pub branch: Option<String>,
}

impl Project {
    fn from_path(path: PathBuf) -> Self {
        let marker = path.join(".git");
        if !marker.exists() {
            return Self {
                path,
                git: false,
                worktree: false,
                branch: None,
            };
        }
        let git_dir = if marker.is_dir() {
            Some(marker.clone())
        } else {
            fs::read_to_string(&marker).ok().and_then(|contents| {
                contents
                    .trim()
                    .strip_prefix("gitdir:")
                    .map(str::trim)
                    .map(|git_dir| path.join(git_dir))
            })
        };
        let worktree = git_dir
            .as_ref()
            .is_some_and(|git_dir| git_dir.join("commondir").is_file());
        let branch = git_dir
            .as_ref()
            .and_then(|git_dir| fs::read_to_string(git_dir.join("HEAD")).ok())
            .and_then(|head| {
                let head = head.trim();
                if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
                    Some(branch.to_owned())
                } else if head.len() >= 7 && head.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    Some(format!("@{}", &head[..7]))
                } else {
                    None
                }
            });
        Self {
            path,
            git: true,
            worktree,
            branch,
        }
    }
}

pub fn discover(
    roots: &[PathBuf],
    favorites: &[PathBuf],
    include_roots: bool,
    config: &Config,
    generation: (Arc<AtomicU64>, u64),
) -> Result<Vec<Project>, String> {
    let mut projects = HashMap::new();
    let policy = Arc::new(Policy::new(config)?);

    for root in roots {
        let root = project_path(root);
        if !root.is_dir() {
            continue;
        }
        let shared_root = cfg!(target_os = "macos")
            && (root == Path::new("/") || root == Path::new("/Users/Shared"));
        let mut walker = WalkBuilder::new(&root);
        walker
            .hidden(false)
            .ignore(false)
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .require_git(false)
            .follow_links(shared_root)
            .max_depth(Some(config.max_depth));
        let filter = Arc::clone(&policy);
        let settings = config.clone();
        let active = Arc::clone(&generation.0);
        let scan_id = generation.1;
        walker.filter_entry(move |entry| {
            active.load(Ordering::Relaxed) == scan_id
                && (entry.depth() == 0 || filter.traverse(entry.path(), &settings))
        });
        for entry in walker.build().flatten() {
            if generation.0.load(Ordering::Relaxed) != generation.1 {
                break;
            }
            if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                continue;
            }
            // Keep the Shared spelling of symlinked directories in search results.
            let path = project_path(entry.path());
            if policy.visible(entry.path(), config)
                && (entry.depth() > 0 || path.join(".git").exists() || include_roots)
            {
                projects
                    .entry(path.clone())
                    .or_insert_with(|| Project::from_path(path));
            }
        }
    }
    for favorite in favorites {
        let path = project_path(favorite);
        if path.is_dir() {
            projects.insert(path.clone(), Project::from_path(path));
        }
    }
    Ok(projects.into_values().collect())
}

pub fn basename(path: &Path) -> String {
    path.file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
