use crate::config::normalize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Path, PathBuf},
    process::Command,
};

pub struct Herdr {
    binary: String,
}

impl Herdr {
    pub fn new() -> Self {
        Self {
            binary: std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into()),
        }
    }

    fn call(&self, args: &[&str]) -> io::Result<Value> {
        let output = Command::new(&self.binary).args(args).output()?;
        if !output.status.success() {
            let message = String::from_utf8_lossy(&output.stderr).into_owned();
            return Err(io::Error::other(message));
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
        value
            .get("result")
            .cloned()
            .ok_or_else(|| io::Error::other("Missing Herdr result"))
    }

    fn workspace_paths(&self) -> io::Result<Vec<(String, PathBuf)>> {
        let response = self.call(&["workspace", "list"])?;
        let mut paths = Vec::new();
        for workspace in response["workspaces"].as_array().into_iter().flatten() {
            let Some(id) = workspace["workspace_id"].as_str() else {
                continue;
            };
            let path = if let Some(checkout) = workspace["worktree"]["checkout_path"].as_str() {
                Some(checkout.to_owned())
            } else {
                let response = self.call(&["pane", "list", "--workspace", id])?;
                response["panes"].as_array().and_then(|panes| {
                    panes
                        .iter()
                        .find_map(|pane| pane["cwd"].as_str().map(str::to_owned))
                })
            };
            if let Some(path) = path {
                paths.push((id.to_owned(), normalize(Path::new(&path))));
            }
        }
        Ok(paths)
    }

    pub fn create(&self, projects: &[PathBuf], focus: bool) -> io::Result<()> {
        let previous = self.workspace_paths()?;
        let previous_names: HashMap<_, _> = labels(&previous).into_iter().collect();
        let mut created = Vec::new();
        for path in projects {
            let path = path.to_string_lossy();
            let result = self.call(&["workspace", "create", "--cwd", &path, "--no-focus"])?;
            let id = result["workspace"]["workspace_id"]
                .as_str()
                .ok_or_else(|| io::Error::other("Herdr did not return a workspace ID"))?;
            created.push(id.to_owned());
        }
        let current = self.workspace_paths()?;
        let created_set: HashSet<_> = created.iter().collect();
        for (id, label) in labels(&current) {
            if created_set.contains(&id) || previous_names.get(&id) != Some(&label) {
                self.call(&["workspace", "rename", &id, &label])?;
            }
        }
        if focus && let Some(last) = created.last() {
            self.call(&["workspace", "focus", last])?;
        }
        Ok(())
    }
}

pub fn labels(paths: &[(String, PathBuf)]) -> Vec<(String, String)> {
    let unique: HashSet<&PathBuf> = paths.iter().map(|(_, path)| path).collect();
    let mut suffixes = HashMap::new();
    for path in &unique {
        let parts: Vec<_> = path.components().collect();
        for length in 1..=parts.len() {
            let suffix: PathBuf = parts[parts.len() - length..].iter().collect();
            let matches = unique
                .iter()
                .filter(|other| other.ends_with(&suffix))
                .count();
            if matches == 1 {
                suffixes.insert(
                    *path,
                    suffix.to_string_lossy().trim_start_matches('/').to_string(),
                );
                break;
            }
        }
    }
    let mut totals = HashMap::new();
    for (_, path) in paths {
        *totals.entry(path).or_insert(0usize) += 1;
    }
    let mut positions = HashMap::new();
    paths
        .iter()
        .map(|(id, path)| {
            let position = positions.entry(path).or_insert(0usize);
            *position += 1;
            let name = suffixes
                .get(path)
                .cloned()
                .unwrap_or_else(|| path.display().to_string());
            let label = if totals[path] > 1 {
                format!("{name} #{position}")
            } else {
                name
            };
            (id.clone(), label)
        })
        .collect()
}
