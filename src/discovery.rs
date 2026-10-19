use crate::config::{Config, project_path};
use crate::policy::Policy;
use ignore::{WalkBuilder, WalkState};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    browse_root_repo: bool,
    config: &Config,
    generation: (Arc<AtomicU64>, u64),
    mut emit: impl FnMut(Vec<Project>) -> bool + Send,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    let mut batch = Vec::new();
    let mut last_emit = std::time::Instant::now();
    let policy = Arc::new(Policy::new(config)?);
    let connected = AtomicBool::new(true);

    for root in roots {
        if generation.0.load(Ordering::Relaxed) != generation.1 {
            return Ok(());
        }
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
            .max_depth(Some(config.max_depth))
            .threads(4);
        let filter = Arc::clone(&policy);
        let settings = config.clone();
        let stop_at_repos = config.ignore_git_repo_contents && !browse_root_repo;
        let active = Arc::clone(&generation.0);
        let scan_id = generation.1;
        walker.filter_entry(move |entry| {
            active.load(Ordering::Relaxed) == scan_id
                && (entry.depth() == 0
                    || (filter.traverse(entry.path(), &settings)
                        && (!stop_at_repos
                            || !entry
                                .path()
                                .parent()
                                .is_some_and(|parent| parent.join(".git").exists()))))
        });
        let state = Mutex::new((&mut seen, &mut batch, &mut last_emit, &mut emit));
        walker.build_parallel().run(|| {
            let policy = Arc::clone(&policy);
            let state = &state;
            let connected = &connected;
            let active = &generation.0;
            Box::new(move |entry| {
                if active.load(Ordering::Relaxed) != scan_id || !connected.load(Ordering::Relaxed) {
                    return WalkState::Quit;
                }
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                    return WalkState::Continue;
                }
                // Keep the Shared spelling of symlinked directories in search results.
                let path = project_path(entry.path());
                if policy.visible(entry.path(), config)
                    && (entry.depth() > 0 || path.join(".git").exists() || include_roots)
                {
                    let mut state = state.lock().expect("discovery state poisoned");
                    let (seen, batch, last_emit, emit) = &mut *state;
                    if seen.insert(path.clone()) {
                        batch.push(Project::from_path(path));
                        if batch.len() >= 64
                            || last_emit.elapsed() >= std::time::Duration::from_millis(50)
                        {
                            if !emit(std::mem::take(batch)) {
                                connected.store(false, Ordering::Relaxed);
                                return WalkState::Quit;
                            }
                            **last_emit = std::time::Instant::now();
                        }
                    }
                }
                WalkState::Continue
            })
        });
        if !connected.load(Ordering::Relaxed) {
            return Ok(());
        }
    }
    if generation.0.load(Ordering::Relaxed) != generation.1 {
        return Ok(());
    }
    for favorite in favorites {
        let path = project_path(favorite);
        if path.is_dir() && seen.insert(path.clone()) {
            batch.push(Project::from_path(path));
            if batch.len() >= 64 && !emit(std::mem::take(&mut batch)) {
                return Ok(());
            }
        }
    }
    if !batch.is_empty() {
        emit(batch);
    }
    Ok(())
}

pub fn basename(path: &Path) -> String {
    path.file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
