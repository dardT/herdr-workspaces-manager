use crate::config::{Config, normalize};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Project {
    pub path: PathBuf,
    pub git: bool,
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
            let git = path.join(".git").exists();
            if entry.depth() > 0 || git || include_roots {
                projects
                    .entry(path.clone())
                    .or_insert(Project { path, git });
            }
        }
    }
    for favorite in favorites {
        let path = normalize(favorite);
        if path.is_dir() {
            projects.insert(
                path.clone(),
                Project {
                    git: path.join(".git").exists(),
                    path,
                },
            );
        }
    }
    projects.into_values().collect()
}

pub fn basename(path: &Path) -> String {
    path.file_name()
        .map(|part| part.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
