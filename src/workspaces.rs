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

#[derive(Clone, Copy)]
struct Rect {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

impl Rect {
    fn right(self) -> u64 {
        self.x + self.width
    }
    fn bottom(self) -> u64 {
        self.y + self.height
    }
}

struct Pane {
    id: String,
    rect: Rect,
}

struct Layout {
    panes: Vec<Pane>,
    area: Rect,
}

fn rect(value: &Value) -> io::Result<Rect> {
    let field = |name| {
        value[name]
            .as_u64()
            .ok_or_else(|| io::Error::other(format!("Missing pane rectangle {name}")))
    };
    Ok(Rect {
        x: field("x")?,
        y: field("y")?,
        width: field("width")?,
        height: field("height")?,
    })
}

impl Layout {
    fn from_value(value: &Value) -> io::Result<Self> {
        let layout = &value["layout"];
        let area = rect(&layout["area"])?;
        let panes = layout["panes"]
            .as_array()
            .ok_or_else(|| io::Error::other("Missing pane layout"))?
            .iter()
            .map(|pane| {
                Ok(Pane {
                    id: pane["pane_id"]
                        .as_str()
                        .ok_or_else(|| io::Error::other("Missing pane ID"))?
                        .into(),
                    rect: rect(&pane["rect"])?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self { panes, area })
    }

    // A valid extension introduces at most one new grid line per axis. Never
    // rearrange existing panes or exceed the configured rectangular grid.
    fn placement(&self, columns: usize, rows: usize) -> Option<(usize, &'static str)> {
        let boundaries = |horizontal: bool| {
            let mut edges = Vec::new();
            for pane in &self.panes {
                let r = pane.rect;
                edges.extend(if horizontal {
                    [r.x, r.right()]
                } else {
                    [r.y, r.bottom()]
                });
            }
            edges.sort_unstable();
            edges.dedup();
            edges
        };
        let xs = boundaries(true);
        let ys = boundaries(false);
        if xs.len().saturating_sub(1) > columns || ys.len().saturating_sub(1) > rows {
            return None;
        }
        let mut candidates = Vec::new();
        for (index, pane) in self.panes.iter().enumerate() {
            let r = pane.rect;
            for (direction, allowed) in [
                (
                    "right",
                    r.width >= 4
                        && (xs.contains(&(r.x + r.width / 2))
                            || xs.len().saturating_sub(1) < columns),
                ),
                (
                    "down",
                    r.height >= 4
                        && (ys.contains(&(r.y + r.height / 2))
                            || ys.len().saturating_sub(1) < rows),
                ),
            ] {
                if allowed {
                    let area = r.width * r.height;
                    // Prefer columns on wide panes, rows on tall panes.
                    let shape = if direction == "right" {
                        r.width
                    } else {
                        r.height * 2
                    };
                    candidates.push((area, shape, index, direction));
                }
            }
        }
        candidates
            .into_iter()
            .max_by_key(|(area, shape, _, _)| (*area, *shape))
            .map(|(_, _, index, direction)| (index, direction))
    }

    fn insert(&mut self, index: usize, direction: &str) {
        let original = self.panes[index].rect;
        let mut added = original;
        if direction == "right" {
            let left = original.width / 2;
            self.panes[index].rect.width = left;
            added.x += left;
            added.width -= left;
        } else {
            let top = original.height / 2;
            self.panes[index].rect.height = top;
            added.y += top;
            added.height -= top;
        }
        self.panes.push(Pane {
            id: "__planned".into(),
            rect: added,
        });
    }
}

pub struct PanePlan {
    pub placements: Vec<String>,
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

    fn tab_context(&self) -> io::Result<(String, Layout)> {
        // Plugin popups receive the invoking location in context JSON, not in
        // the HERDR_WORKSPACE_ID / HERDR_TAB_ID variables given to normal panes.
        let plugin_context = std::env::var("HERDR_PLUGIN_CONTEXT_JSON")
            .ok()
            .map(|text| serde_json::from_str::<Value>(&text).map_err(io::Error::other))
            .transpose()?;
        let context_id = |name| {
            plugin_context
                .as_ref()?
                .get(name)?
                .as_str()
                .map(str::to_owned)
        };
        let workspace = context_id("workspace_id")
            .or_else(|| std::env::var("HERDR_WORKSPACE_ID").ok())
            .ok_or_else(|| io::Error::other("No invoking Herdr workspace available"))?;
        let response = self.call(&["pane", "list", "--workspace", &workspace])?;
        let tab = context_id("tab_id")
            .or_else(|| std::env::var("HERDR_TAB_ID").ok())
            .or_else(|| {
                response["panes"]
                    .as_array()?
                    .iter()
                    .find(|pane| pane["focused"].as_bool() == Some(true))?["tab_id"]
                    .as_str()
                    .map(str::to_owned)
            })
            .ok_or_else(|| io::Error::other("No invoking Herdr tab available"))?;
        let pane_id = response["panes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|pane| pane["tab_id"].as_str() == Some(&tab))
            .and_then(|pane| pane["pane_id"].as_str())
            .ok_or_else(|| io::Error::other("Invoking tab no longer has a pane"))?;
        let layout = self.layout(pane_id)?;
        Ok((workspace, layout))
    }

    fn layout(&self, pane_id: &str) -> io::Result<Layout> {
        Layout::from_value(&self.call(&["pane", "layout", "--pane", pane_id])?)
    }

    pub fn pane_plan(&self, count: usize, columns: usize, rows: usize) -> io::Result<PanePlan> {
        if columns == 0 || rows == 0 {
            return Err(io::Error::other(
                "Pane grid limits must be greater than zero",
            ));
        }
        let (_, mut layout) = self.tab_context()?;
        let mut tab = 0;
        let mut placements = Vec::new();
        for _ in 0..count {
            if let Some((index, direction)) = layout.placement(columns, rows) {
                layout.insert(index, direction);
                placements.push(format!("tab {} · split {direction}", tab + 1));
            } else {
                tab += 1;
                placements.push(format!("new tab {} · first pane", tab + 1));
                layout.panes = vec![Pane {
                    id: "__planned".into(),
                    rect: layout.area,
                }];
            }
        }
        Ok(PanePlan { placements })
    }

    pub fn open_in_workspace(
        &self,
        projects: &[PathBuf],
        columns: usize,
        rows: usize,
        mut on_created: impl FnMut(&Path),
    ) -> io::Result<()> {
        if columns == 0 || rows == 0 {
            return Err(io::Error::other(
                "Pane grid limits must be greater than zero",
            ));
        }
        let (workspace, mut layout) = self.tab_context()?;
        for (index, path) in projects.iter().enumerate() {
            let cwd = path.to_string_lossy();
            let focus = if index + 1 == projects.len() {
                "--focus"
            } else {
                "--no-focus"
            };
            if let Some((target, direction)) = layout.placement(columns, rows) {
                let result = self.call(&[
                    "pane",
                    "split",
                    &layout.panes[target].id,
                    "--direction",
                    direction,
                    "--cwd",
                    &cwd,
                    focus,
                ])?;
                let id = result["pane"]["pane_id"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("Herdr did not return a pane ID"))?;
                layout = self.layout(id)?;
            } else {
                let result = self.call(&[
                    "tab",
                    "create",
                    "--workspace",
                    &workspace,
                    "--cwd",
                    &cwd,
                    focus,
                ])?;
                let id = result["root_pane"]["pane_id"]
                    .as_str()
                    .ok_or_else(|| io::Error::other("Herdr did not return a tab root pane ID"))?;
                layout = self.layout(id)?;
            }
            on_created(path);
        }
        Ok(())
    }

    pub fn workspace_paths(&self) -> io::Result<Vec<(String, PathBuf)>> {
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

    pub fn create(
        &self,
        projects: &[PathBuf],
        focus: bool,
        mut on_created: impl FnMut(&Path),
    ) -> io::Result<()> {
        let previous = self.workspace_paths()?;
        let previous_names: HashMap<_, _> = labels(&previous).into_iter().collect();
        let mut created = Vec::new();
        for path in projects {
            let cwd = path.to_string_lossy();
            let result = self.call(&["workspace", "create", "--cwd", &cwd, "--no-focus"])?;
            let id = result["workspace"]["workspace_id"]
                .as_str()
                .ok_or_else(|| io::Error::other("Herdr did not return a workspace ID"))?;
            created.push(id.to_owned());
            on_created(path);
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

pub fn planned_labels(existing: &[(String, PathBuf)], projects: &[PathBuf]) -> Vec<String> {
    let mut paths = existing.to_vec();
    let mut used_ids: HashSet<String> = existing.iter().map(|(id, _)| id.clone()).collect();
    for (index, path) in projects.iter().enumerate() {
        let mut id = format!("__preview_{index}");
        while used_ids.contains(&id) {
            id.push('_');
        }
        used_ids.insert(id.clone());
        paths.push((id, normalize(path)));
    }
    labels(&paths)
        .into_iter()
        .skip(existing.len())
        .map(|(_, name)| name)
        .collect()
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
