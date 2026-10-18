use serde::{Deserialize, Serialize};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub roots: Vec<PathBuf>,
    pub favorites: Vec<PathBuf>,
    pub max_depth: usize,
    pub include_hidden: bool,
    pub focus_created: bool,
    pub max_pane_columns: usize,
    pub max_pane_rows: usize,
    pub keybindings: Keybindings,
    pub excluded_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Keybindings {
    pub select: String,
    pub open_in_workspace: String,
    pub scope: String,
    pub scope_backward: String,
    pub enter_directory: String,
    pub parent_directory: String,
    pub favorite: String,
    pub add_root: String,
    pub remove_root: String,
    pub help: String,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            select: "ctrl+space".into(),
            open_in_workspace: "ctrl+t".into(),
            scope: "ctrl+g".into(),
            scope_backward: "ctrl+shift+g".into(),
            enter_directory: "tab".into(),
            parent_directory: "shift+tab".into(),
            favorite: "ctrl+f".into(),
            add_root: "ctrl+r".into(),
            remove_root: "ctrl+shift+r".into(),
            help: "?".into(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let home = home();
        let roots = ["ZedProjects", "PycharmProjects"]
            .iter()
            .map(|name| home.join(name))
            .filter(|path| path.is_dir())
            .collect();
        Self {
            roots,
            favorites: Vec::new(),
            max_depth: 20,
            include_hidden: false,
            focus_created: false,
            max_pane_columns: 2,
            max_pane_rows: 2,
            keybindings: Keybindings::default(),
            excluded_names: [
                ".*",
                "node_modules",
                "vendor",
                "target",
                "dist",
                "build",
                ".venv",
                "venv",
                "__pycache__",
                ".next",
                "coverage",
                "site-packages",
            ]
            .iter()
            .map(|value| (*value).into())
            .collect(),
        }
    }
}

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn all_roots() -> Vec<PathBuf> {
    let mut roots = vec![home()];
    #[cfg(target_os = "macos")]
    {
        let shared = PathBuf::from("/Users/Shared");
        if shared.is_dir() {
            roots.push(shared);
        }
    }
    roots
}

pub fn normalize(path: &Path) -> PathBuf {
    let expanded = if path == Path::new("~") {
        home()
    } else if let Ok(rest) = path.strip_prefix("~") {
        home().join(rest)
    } else {
        path.to_path_buf()
    };
    fs::canonicalize(&expanded).unwrap_or(expanded)
}

// Keep the user-visible Shared alias for discovery, favorites and recent history.
pub fn project_path(path: &Path) -> PathBuf {
    if cfg!(target_os = "macos") && path.starts_with("/Users/Shared") {
        path.to_path_buf()
    } else {
        normalize(path)
    }
}

pub fn config_path() -> PathBuf {
    env::var_os("HERDR_PLUGIN_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config/herdr/workspaces-manager"))
        .join("config.json")
}

pub fn load(path: &Path) -> io::Result<Config> {
    if path.exists() {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
        let missing_bindings = [
            "scope_backward",
            "enter_directory",
            "parent_directory",
            "open_in_workspace",
        ]
        .iter()
        .any(|name| value.pointer(&format!("/keybindings/{name}")).is_none());
        let mut config: Config = serde_json::from_value(value.clone()).map_err(io::Error::other)?;
        if value
            .pointer("/keybindings/select")
            .is_some_and(|binding| binding == "shift+enter,ctrl+t")
        {
            config.keybindings.select = "ctrl+space".into();
        }
        // Migrate generated defaults while preserving other custom bindings.
        let old_remove = config.keybindings.remove_root == "ctrl+d";
        if old_remove {
            config.keybindings.remove_root = "ctrl+shift+r".into();
        }
        if old_remove
            || missing_bindings
            || value.get("max_pane_columns").is_none()
            || value.get("max_pane_rows").is_none()
            || value
                .pointer("/keybindings/select")
                .is_some_and(|binding| binding == "shift+enter,ctrl+t")
        {
            save(path, &config)?;
        }
        Ok(config)
    } else {
        let config = Config::default();
        save(path, &config)?;
        Ok(config)
    }
}

pub fn save(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(config).map_err(io::Error::other)?,
    )?;
    fs::rename(temporary, path)
}
