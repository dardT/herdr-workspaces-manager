use crate::config::{Config, normalize};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
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
    excluded_directory: Option<&Path>,
    include_roots: bool,
    config: &Config,
) -> Vec<Project> {
    let mut projects = HashMap::new();
    let mut patterns = GlobSetBuilder::new();
    for pattern in &config.excluded_names {
        if let Ok(glob) = Glob::new(pattern) {
            patterns.add(glob);
        }
    }
    let excluded = patterns.build().ok();
    let excluded_directory = excluded_directory.map(normalize);

    for root in roots {
        let root = normalize(root);
        if !root.is_dir() {
            continue;
        }
        let mut walker = WalkBuilder::new(&root);
        walker
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .require_git(false)
            .follow_links(false)
            .max_depth(Some(config.max_depth));
        let excluded = excluded.clone();
        let excluded_directory = excluded_directory.clone();
        walker.filter_entry(move |entry| {
            if entry.depth() == 0 {
                return true;
            }
            if excluded_directory
                .as_ref()
                .is_some_and(|path| entry.path() == path)
            {
                return false;
            }
            !excluded
                .as_ref()
                .is_some_and(|set| set.is_match(entry.file_name()))
        });
        for entry in walker.build().flatten() {
            if !entry.file_type().is_some_and(|kind| kind.is_dir()) {
                continue;
            }
            let path = normalize(entry.path());
            if entry.depth() > 0 || path.join(".git").exists() || include_roots {
                projects
                    .entry(path.clone())
                    .or_insert_with(|| Project::from_path(path));
            }
        }
    }
    for favorite in favorites {
        let path = normalize(favorite);
        if path.is_dir() {
            projects.insert(path.clone(), Project::from_path(path));
        }
    }
    projects.into_values().collect()
}

pub fn basename(path: &Path) -> String {
    path.file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
