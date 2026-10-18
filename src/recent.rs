use crate::config::project_path;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::HashSet,
    env, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const LIMIT: usize = 30;

#[derive(Deserialize, Serialize)]
struct Entry {
    path: PathBuf,
    last_opened: u64,
}

pub fn state_path(config_path: &Path) -> PathBuf {
    env::var_os("HERDR_PLUGIN_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| config_path.parent().unwrap_or(Path::new(".")).to_path_buf())
        .join("recent.json")
}

fn load(path: &Path) -> io::Result<Vec<Entry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<Entry> =
        serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
    entries.sort_by_key(|entry| Reverse(entry.last_opened));
    let mut seen = HashSet::new();
    entries.retain(|entry| seen.insert(project_path(&entry.path)));
    entries.truncate(LIMIT);
    Ok(entries)
}

pub fn paths(path: &Path) -> io::Result<Vec<PathBuf>> {
    Ok(load(path)?
        .into_iter()
        .map(|entry| project_path(&entry.path))
        .collect())
}

pub fn record(path: &Path, created: &[PathBuf]) -> io::Result<()> {
    if created.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Different Herdr sessions may create workspaces at the same time.
    let lock = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.with_extension("lock"))?;
    lock.lock()?;
    let mut entries = load(path)?;
    for project in created {
        let project = project_path(project);
        entries.retain(|entry| project_path(&entry.path) != project);
        let last_opened = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_millis()
            .try_into()
            .map_err(io::Error::other)?;
        entries.insert(
            0,
            Entry {
                path: project,
                last_opened,
            },
        );
    }
    entries.truncate(LIMIT);
    let temporary = path.with_extension("tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&entries).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}
