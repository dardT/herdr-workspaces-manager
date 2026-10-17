use crate::{
    config::{self, Config},
    discovery::{self, Project},
    workspaces::Herdr,
};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, KeyboardEnhancementFlags, MouseButton, MouseEvent, MouseEventKind,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
};
use nucleo_matcher::{
    Config as MatchConfig, Matcher, Utf32String,
    pattern::{Atom, AtomKind, CaseMatching, Normalization},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Picker,
    BatchReview,
    RootConfirm,
    RootInput,
    Help,
}

#[derive(Clone, PartialEq, Eq)]
enum Scope {
    All,
    Favorites,
    Root(usize),
    Directory(PathBuf),
}

struct App {
    config: Config,
    config_path: PathBuf,
    scope: Scope,
    mode: Mode,
    query: String,
    query_caret: usize,
    root_input: String,
    projects: Vec<Project>,
    results: Vec<usize>,
    pending: BTreeSet<PathBuf>,
    review_highlighted: usize,
    review_state: ListState,
    review_list_area: Rect,
    dialog_buttons: Vec<(Rect, &'static str)>,
    root_to_remove: Option<PathBuf>,
    favorite_undo_path: Option<PathBuf>,
    highlighted: usize,
    list_state: ListState,
    list_area: Rect,
    scope_buttons: Vec<(Rect, Scope)>,
    action_buttons: Vec<(Rect, &'static str)>,
    help_button: Rect,
    status: String,
    root_added_notice: Option<String>,
    generation: u64,
    sender: Sender<(u64, Vec<Project>)>,
    receiver: Receiver<(u64, Vec<Project>)>,
    running: bool,
}

impl App {
    fn new(config: Config, config_path: PathBuf) -> Self {
        let (sender, receiver) = mpsc::channel();
        let mut app = Self {
            config,
            config_path,
            scope: Scope::All,
            mode: Mode::Picker,
            query: String::new(),
            query_caret: 0,
            root_input: String::new(),
            projects: Vec::new(),
            results: Vec::new(),
            pending: BTreeSet::new(),
            review_highlighted: 0,
            review_state: ListState::default(),
            review_list_area: Rect::default(),
            dialog_buttons: Vec::new(),
            root_to_remove: None,
            favorite_undo_path: None,
            highlighted: 0,
            list_state: ListState::default(),
            list_area: Rect::default(),
            scope_buttons: Vec::new(),
            action_buttons: Vec::new(),
            help_button: Rect::default(),
            status: String::new(),
            root_added_notice: None,
            generation: 0,
            sender,
            receiver,
            running: true,
        };
        app.scan();
        app
    }

    fn scan(&mut self) {
        self.root_added_notice = None;
        self.favorite_undo_path = None;
        self.generation += 1;
        let generation = self.generation;
        let (roots, favorites, scoped_directory) = match &self.scope {
            Scope::All => (vec![config::home()], Vec::new(), None),
            Scope::Favorites => (Vec::new(), self.config.favorites.clone(), None),
            Scope::Root(index) => (
                self.config.roots.get(*index).cloned().into_iter().collect(),
                Vec::new(),
                None,
            ),
            Scope::Directory(path) => (vec![path.clone()], Vec::new(), Some(path.clone())),
        };
        let home = config::normalize(&config::home());
        let excluded_directory = (self.scope == Scope::All
            || scoped_directory.as_ref() == Some(&home))
        .then(|| home.join("Library"));
        let include_roots = matches!(self.scope, Scope::Root(_));
        let config = self.config.clone();
        let sender = self.sender.clone();
        self.projects.clear();
        self.results.clear();
        self.highlighted = 0;
        self.list_state = ListState::default();
        self.status = format!("Scanning {}…", self.scope_name());
        thread::spawn(move || {
            let mut results = discovery::discover(
                &roots,
                &favorites,
                excluded_directory.as_deref(),
                include_roots,
                &config,
            );
            if let Some(directory) = scoped_directory {
                results.retain(|project| project.path != directory);
            }
            let _ = sender.send((generation, results));
        });
    }

    fn receive(&mut self) -> bool {
        let mut updated = false;
        while let Ok((generation, projects)) = self.receiver.try_recv() {
            if generation != self.generation {
                continue;
            }
            self.projects = projects;
            self.filter();
            updated = true;
        }
        updated
    }

    fn filter(&mut self) {
        let mut matcher = Matcher::new(MatchConfig::DEFAULT);
        // Atom normalizes the query as required by the matcher (including mixed-case input).
        let needle = Atom::new(
            &self.query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
            false,
        );
        let mut matched = Vec::new();
        for (index, project) in self.projects.iter().enumerate() {
            let path = project.path.to_string_lossy();
            let candidate = Utf32String::from(path.as_ref());
            if self.query.is_empty() || needle.score(candidate.slice(..), &mut matcher).is_some() {
                matched.push(index);
            }
        }
        matched.sort_unstable_by(|left, right| {
            let left = &self.projects[*left].path;
            let right = &self.projects[*right].path;
            left.components()
                .count()
                .cmp(&right.components().count())
                .then_with(|| left.cmp(right))
        });
        let matches = matched.len();
        self.results = matched.into_iter().take(1000).collect();
        self.highlighted = self.highlighted.min(self.results.len().saturating_sub(1));
        self.list_state
            .select((!self.results.is_empty()).then_some(self.highlighted));
        self.status = format!(
            "{matches} matches · {} selected · {} indexed · {}",
            self.pending.len(),
            self.projects.len(),
            self.scope_name()
        );
        if let Some(notice) = &self.root_added_notice {
            self.status = format!("{notice} · {matches} matches");
        }
    }

    fn query_changed(&mut self) {
        self.dismiss_favorite_undo();
        self.highlighted = 0;
        self.filter();
    }

    fn edit_search_key(&mut self, key: KeyEvent) -> bool {
        let previous = || {
            self.query[..self.query_caret]
                .grapheme_indices(true)
                .next_back()
                .map(|(index, _)| index)
                .unwrap_or(0)
        };
        let next = || {
            self.query[self.query_caret..]
                .graphemes(true)
                .next()
                .map(|grapheme| self.query_caret + grapheme.len())
                .unwrap_or(self.query_caret)
        };
        match (key.code, key.modifiers) {
            (KeyCode::Left, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.query_caret = previous();
            }
            (KeyCode::Right, KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.query_caret = next();
            }
            (KeyCode::Home, _) => self.query_caret = 0,
            (KeyCode::End, _) => self.query_caret = self.query.len(),
            (KeyCode::Char(character), KeyModifiers::CONTROL) => {
                match character.to_ascii_lowercase() {
                    'a' => self.query_caret = 0,
                    'e' => self.query_caret = self.query.len(),
                    'b' => self.query_caret = previous(),
                    'f' => self.query_caret = next(),
                    'u' => {
                        if self.query_caret > 0 {
                            self.query.drain(..self.query_caret);
                            self.query_caret = 0;
                            self.query_changed();
                        }
                    }
                    'k' => {
                        if self.query_caret < self.query.len() {
                            self.query.truncate(self.query_caret);
                            self.query_changed();
                        }
                    }
                    'w' => {
                        let mut start = self.query_caret;
                        for (index, grapheme) in self.query[..start].grapheme_indices(true).rev() {
                            if grapheme.chars().all(char::is_whitespace) {
                                start = index;
                            } else {
                                break;
                            }
                        }
                        for (index, grapheme) in self.query[..start].grapheme_indices(true).rev() {
                            if grapheme.chars().any(char::is_whitespace) {
                                break;
                            }
                            start = index;
                        }
                        if start < self.query_caret {
                            self.query.replace_range(start..self.query_caret, "");
                            self.query_caret = start;
                            self.query_changed();
                        }
                    }
                    _ => return false,
                }
            }
            (KeyCode::Backspace, _) => {
                let start = previous();
                if start < self.query_caret {
                    self.query.replace_range(start..self.query_caret, "");
                    self.query_caret = start;
                    self.query_changed();
                }
            }
            (KeyCode::Delete, _) => {
                let end = next();
                if end > self.query_caret {
                    self.query.replace_range(self.query_caret..end, "");
                    self.query_changed();
                }
            }
            (KeyCode::Char(character), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.query.insert(self.query_caret, character);
                self.query_caret += character.len_utf8();
                self.query_changed();
            }
            _ => return false,
        }
        true
    }

    fn search_line(&self, width: u16) -> (String, u16) {
        let capacity = usize::from(width.saturating_sub(9));
        let before = self.query[..self.query_caret]
            .graphemes(true)
            .rev()
            .scan(0, |used, grapheme| {
                let columns = UnicodeWidthStr::width(grapheme);
                (*used + columns <= capacity).then(|| {
                    *used += columns;
                    grapheme
                })
            })
            .collect::<Vec<_>>();
        let used = before
            .iter()
            .map(|part| UnicodeWidthStr::width(*part))
            .sum::<usize>();
        let prefix = before.into_iter().rev().collect::<String>();
        let suffix = self.query[self.query_caret..]
            .graphemes(true)
            .scan(used, |used, grapheme| {
                let columns = UnicodeWidthStr::width(grapheme);
                (*used + columns <= capacity).then(|| {
                    *used += columns;
                    grapheme
                })
            })
            .collect::<String>();
        (
            format!("Search: {prefix}{suffix}"),
            8_u16.saturating_add(u16::try_from(used).unwrap_or(u16::MAX)),
        )
    }

    fn scope_name(&self) -> String {
        match &self.scope {
            Scope::All => "All home projects".into(),
            Scope::Favorites => "Favorites".into(),
            Scope::Root(index) => self
                .config
                .roots
                .get(*index)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "All home projects".into()),
            Scope::Directory(path) => path.display().to_string(),
        }
    }

    fn selected_path(&self) -> Option<PathBuf> {
        self.results
            .get(self.highlighted)
            .map(|index| self.projects[*index].path.clone())
    }

    fn next_scope(&mut self) {
        self.scope = match &self.scope {
            Scope::All => Scope::Favorites,
            Scope::Favorites if self.config.roots.is_empty() => Scope::All,
            Scope::Favorites => Scope::Root(0),
            Scope::Root(index) if index + 1 < self.config.roots.len() => Scope::Root(index + 1),
            Scope::Root(_) | Scope::Directory(_) => Scope::All,
        };
        self.scan();
    }

    fn previous_scope(&mut self) {
        self.scope = match &self.scope {
            Scope::All if self.config.roots.is_empty() => Scope::Favorites,
            Scope::All => Scope::Root(self.config.roots.len() - 1),
            Scope::Favorites => Scope::All,
            Scope::Root(0) => Scope::Favorites,
            Scope::Root(index) => Scope::Root(index - 1),
            Scope::Directory(_) => Scope::All,
        };
        self.scan();
    }

    fn enter_directory(&mut self) {
        if let Some(path) = self.selected_path() {
            self.scope = Scope::Directory(path);
            self.query.clear();
            self.query_caret = 0;
            self.scan();
        } else {
            self.status = "Highlight a directory before pressing Tab".into();
        }
    }

    fn parent_directory(&mut self) {
        let directory = match &self.scope {
            Scope::Directory(path) => Some(path.clone()),
            Scope::Root(index) => self
                .config
                .roots
                .get(*index)
                .map(|path| config::normalize(path)),
            Scope::All | Scope::Favorites => None,
        };
        let Some(directory) = directory else {
            self.status = "No directory scope to move up from".into();
            return;
        };
        let home = config::normalize(&config::home());
        if directory == home {
            self.status = "Already at your home directory".into();
            return;
        }
        if let Some(parent) = directory.parent() {
            self.scope = Scope::Directory(parent.to_path_buf());
            self.query.clear();
            self.query_caret = 0;
            self.scan();
        } else {
            self.status = "Already at the filesystem root".into();
        }
    }

    fn toggle_pending(&mut self) {
        if let Some(path) = self.selected_path() {
            if !self.pending.insert(path.clone()) {
                self.pending.remove(&path);
            }
            self.status = format!("{} projects selected · Enter to review", self.pending.len());
        }
    }

    fn remove_review_selection(&mut self) {
        if let Some(path) = self.pending.iter().nth(self.review_highlighted).cloned() {
            self.pending.remove(&path);
            if self.pending.is_empty() {
                self.mode = Mode::Picker;
                self.status = "Batch is empty · select projects to start again".into();
            } else {
                self.review_highlighted = self.review_highlighted.min(self.pending.len() - 1);
                self.review_state.select(Some(self.review_highlighted));
            }
        }
    }

    fn dismiss_favorite_undo(&mut self) {
        if let Some(path) = self.favorite_undo_path.take() {
            self.projects.retain(|project| project.path != path);
        }
    }

    fn favorite(&mut self) {
        if let Some(path) = self.selected_path() {
            if self.scope == Scope::Favorites
                && self
                    .favorite_undo_path
                    .as_ref()
                    .is_some_and(|previous| previous != &path)
            {
                self.dismiss_favorite_undo();
                self.filter();
            }
            let mut updated = self.config.clone();
            let removed = if let Some(index) = updated
                .favorites
                .iter()
                .position(|item| config::normalize(item) == path)
            {
                updated.favorites.remove(index);
                true
            } else {
                updated.favorites.push(path.clone());
                false
            };
            if let Err(error) = config::save(&self.config_path, &updated) {
                self.status = format!("Favorite could not be saved: {error}");
                return;
            }
            self.config = updated;
            if self.scope == Scope::Favorites {
                if removed {
                    self.favorite_undo_path = Some(path);
                    self.status = "Favorite removed · Ctrl+F again to restore".into();
                } else {
                    self.favorite_undo_path = None;
                    self.status = "Favorite restored".into();
                }
            } else {
                self.status = if removed {
                    "Favorite removed · Ctrl+F again to restore"
                } else {
                    "Favorite added · Ctrl+F again to remove"
                }
                .into();
            }
        }
    }

    fn confirm_root_removal(&mut self) {
        self.mode = Mode::Picker;
        if let Some(path) = self.root_to_remove.take()
            && let Some(index) = self
                .config
                .roots
                .iter()
                .position(|root| config::normalize(root) == path)
        {
            self.remove_root_index(index);
        }
    }

    fn remove_root_index(&mut self, index: usize) {
        let mut updated = self.config.clone();
        if index >= updated.roots.len() {
            self.status = "Root no longer available".into();
            return;
        }
        let path = config::normalize(&updated.roots.remove(index));
        if let Err(error) = config::save(&self.config_path, &updated) {
            self.status = format!("Root could not be removed: {error}");
            return;
        }
        self.config = updated;
        match self.scope {
            Scope::Root(active) if active == index => {
                self.scope = Scope::All;
                self.scan();
            }
            Scope::Root(active) if active > index => self.scope = Scope::Root(active - 1),
            _ => {}
        }
        self.status = format!("Root removed: {}", path.display());
    }

    fn remove_root(&mut self) {
        if let Some(index) = self.selected_path().and_then(|path| {
            self.config
                .roots
                .iter()
                .position(|root| config::normalize(root) == path)
        }) {
            self.remove_root_index(index);
        } else if let Scope::Root(index) = self.scope {
            self.root_to_remove = self
                .config
                .roots
                .get(index)
                .map(|path| config::normalize(path));
            if self.root_to_remove.is_some() {
                self.mode = Mode::RootConfirm;
            }
        } else {
            self.status = "Highlight a configured root or select its root scope".into();
        }
    }

    fn add_root_path(&mut self, path: &Path) {
        let path = config::normalize(path);
        if !path.is_dir() {
            self.status = format!(
                "Not a directory: {} · use a full path with + Root",
                path.display()
            );
            return;
        }
        if self
            .config
            .roots
            .iter()
            .any(|root| config::normalize(root) == path)
        {
            self.status = format!("Root already available: {}", path.display());
            return;
        }
        let mut updated = self.config.clone();
        updated.roots.push(path.clone());
        if let Err(error) = config::save(&self.config_path, &updated) {
            self.status = format!("Root could not be saved: {error}");
            return;
        }
        self.config = updated;
        self.root_added_notice = Some(format!("Root added: {}", path.display()));
        self.status = format!(
            "Root added: {} · scope: {}",
            path.display(),
            self.scope_name()
        );
    }

    fn add_highlighted_root(&mut self) {
        if let Some(path) = self.selected_path() {
            self.add_root_path(&path);
        } else {
            self.status =
                "Highlight a project before pressing Ctrl+R; use + Root for a path".into();
        }
    }

    fn add_root_from_input(&mut self) {
        let input = self.root_input.trim().to_owned();
        self.mode = Mode::Picker;
        self.root_input.clear();
        if input.is_empty() {
            self.status =
                "Enter a full directory path, or highlight a project and press Ctrl+R".into();
        } else {
            self.add_root_path(Path::new(&input));
        }
    }

    fn create(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        match Herdr::new().create(&paths, self.config.focus_created) {
            Ok(()) => self.running = false,
            Err(error) => {
                self.status = format!("Workspace creation failed: {error}");
                self.mode = Mode::Picker;
            }
        }
    }

    fn activate(&mut self) {
        if !self.pending.is_empty() {
            self.review_highlighted = 0;
            self.review_state.select(Some(0));
            self.mode = Mode::BatchReview;
        } else if let Some(path) = self.selected_path() {
            self.create(vec![path]);
        }
    }

    fn key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.mode == Mode::Help {
            if key.code == KeyCode::Esc || bound(&self.config.keybindings.help, key) {
                self.mode = Mode::Picker;
            }
            return;
        }
        if self.mode == Mode::BatchReview {
            if bound(&self.config.keybindings.select, key) {
                self.remove_review_selection();
                return;
            }
            match key.code {
                KeyCode::Up => {
                    self.review_highlighted = self.review_highlighted.saturating_sub(1);
                    self.review_state.select(Some(self.review_highlighted));
                }
                KeyCode::Down => {
                    self.review_highlighted =
                        (self.review_highlighted + 1).min(self.pending.len().saturating_sub(1));
                    self.review_state.select(Some(self.review_highlighted));
                }
                KeyCode::Backspace | KeyCode::Delete => self.remove_review_selection(),
                KeyCode::Enter => self.create(self.pending.iter().cloned().collect()),
                KeyCode::Esc => self.mode = Mode::Picker,
                _ => {}
            }
            return;
        }
        if self.mode == Mode::RootConfirm {
            match key.code {
                KeyCode::Enter => self.confirm_root_removal(),
                KeyCode::Esc => {
                    self.root_to_remove = None;
                    self.mode = Mode::Picker;
                }
                _ => {}
            }
            return;
        }
        if self.mode == Mode::RootInput {
            match key.code {
                KeyCode::Enter => self.add_root_from_input(),
                KeyCode::Esc => {
                    self.mode = Mode::Picker;
                    self.root_input.clear();
                }
                KeyCode::Backspace => {
                    self.root_input.pop();
                }
                KeyCode::Char(character)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.root_input.push(character);
                }
                _ => {}
            }
            return;
        }
        let bindings = &self.config.keybindings;
        if bound(&bindings.help, key) {
            self.mode = Mode::Help;
            return;
        }
        if bound(&bindings.select, key) {
            self.toggle_pending();
            return;
        }
        if bound(&bindings.scope, key) {
            self.next_scope();
            return;
        }
        if bound(&bindings.scope_backward, key) {
            self.previous_scope();
            return;
        }
        if bound(&bindings.enter_directory, key) {
            self.enter_directory();
            return;
        }
        if bound(&bindings.parent_directory, key) {
            self.parent_directory();
            return;
        }
        if bound(&bindings.favorite, key) {
            self.favorite();
            return;
        }
        if bound(&bindings.add_root, key) {
            self.add_highlighted_root();
            return;
        }
        if bound(&bindings.remove_root, key) {
            self.remove_root();
            return;
        }
        if self.edit_search_key(key) {
            return;
        }
        match key.code {
            KeyCode::Esc => self.running = false,
            KeyCode::Enter => self.activate(),
            KeyCode::Up => {
                self.highlighted = self.highlighted.saturating_sub(1);
                self.list_state
                    .select((!self.results.is_empty()).then_some(self.highlighted));
            }
            KeyCode::Down => {
                self.highlighted = (self.highlighted + 1).min(self.results.len().saturating_sub(1));
                self.list_state
                    .select((!self.results.is_empty()).then_some(self.highlighted));
            }
            _ => {}
        }
    }

    fn mouse(&mut self, mouse: MouseEvent) {
        if self.mode == Mode::BatchReview {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    self.review_highlighted =
                        (self.review_highlighted + 3).min(self.pending.len().saturating_sub(1));
                }
                MouseEventKind::ScrollUp => {
                    self.review_highlighted = self.review_highlighted.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some((_, action)) = self
                        .dialog_buttons
                        .iter()
                        .find(|(rect, _)| contains(*rect, mouse.column, mouse.row))
                    {
                        match *action {
                            "create" => self.create(self.pending.iter().cloned().collect()),
                            "drop" => self.remove_review_selection(),
                            "back" => self.mode = Mode::Picker,
                            _ => {}
                        }
                        return;
                    }
                    if contains(self.review_list_area, mouse.column, mouse.row) {
                        let row = self.review_state.offset()
                            + usize::from(mouse.row - self.review_list_area.y);
                        if row < self.pending.len() {
                            self.review_highlighted = row;
                        }
                    }
                }
                _ => return,
            }
            self.review_state.select(Some(self.review_highlighted));
            return;
        }
        if self.mode == Mode::RootConfirm {
            if mouse.kind == MouseEventKind::Down(MouseButton::Left)
                && let Some((_, action)) = self
                    .dialog_buttons
                    .iter()
                    .find(|(rect, _)| contains(*rect, mouse.column, mouse.row))
            {
                match *action {
                    "remove_root" => self.confirm_root_removal(),
                    "back" => {
                        self.root_to_remove = None;
                        self.mode = Mode::Picker;
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.mode != Mode::Picker {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                self.highlighted = (self.highlighted + 3).min(self.results.len().saturating_sub(1))
            }
            MouseEventKind::ScrollUp => self.highlighted = self.highlighted.saturating_sub(3),
            MouseEventKind::Down(MouseButton::Left) => {
                if contains(self.help_button, mouse.column, mouse.row) {
                    self.mode = Mode::Help;
                    return;
                }
                if let Some((_, scope)) = self
                    .scope_buttons
                    .iter()
                    .find(|(rect, _)| contains(*rect, mouse.column, mouse.row))
                {
                    self.scope = scope.clone();
                    self.scan();
                    return;
                }
                if let Some((_, action)) = self
                    .action_buttons
                    .iter()
                    .find(|(rect, _)| contains(*rect, mouse.column, mouse.row))
                {
                    match *action {
                        "root" => self.mode = Mode::RootInput,
                        "remove" => self.remove_root(),
                        "favorite" => self.favorite(),
                        _ => {}
                    }
                    return;
                }
                if contains(self.list_area, mouse.column, mouse.row) {
                    let row = self.list_state.offset() + usize::from(mouse.row - self.list_area.y);
                    if row < self.results.len() {
                        self.highlighted = row;
                    }
                }
            }
            _ => return,
        }
        self.list_state
            .select((!self.results.is_empty()).then_some(self.highlighted));
    }

    fn draw(&mut self, frame: &mut Frame) {
        let sections = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(4),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(frame.area());
        frame.render_widget(
            Paragraph::new(" WORKSPACES MANAGER").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            sections[0],
        );
        self.scope_buttons.clear();
        self.action_buttons.clear();
        self.dialog_buttons.clear();
        let mut x = sections[1].x;
        for (text, scope) in [("All", Scope::All), ("★ Favorites", Scope::Favorites)] {
            let width = u16::try_from(text.chars().count() + 2).unwrap_or(u16::MAX);
            if x.saturating_add(width) > sections[1].right() {
                break;
            }
            let rect = Rect::new(x, sections[1].y, width, 1);
            let style = if self.scope == scope {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Cyan)
            };
            frame.render_widget(Paragraph::new(format!(" {text} ")).style(style), rect);
            self.scope_buttons.push((rect, scope));
            x += width + 1;
        }
        if let Scope::Directory(path) = &self.scope {
            let available = sections[1].right().saturating_sub(x).saturating_sub(3);
            let name: String = discovery::basename(path)
                .chars()
                .take(available as usize)
                .collect();
            if !name.is_empty() {
                let width = u16::try_from(name.chars().count() + 3).unwrap_or(u16::MAX);
                frame.render_widget(
                    Paragraph::new(format!(" ↳{name} "))
                        .style(Style::default().fg(Color::Black).bg(Color::Cyan)),
                    Rect::new(x, sections[1].y, width, 1),
                );
            }
        }
        let mut root_x = sections[1].x;
        let mut root_y = sections[1].y + 1;
        for (index, path) in self.config.roots.iter().enumerate() {
            let name = discovery::basename(path);
            let text: String = name
                .chars()
                .take(sections[1].width.saturating_sub(2) as usize)
                .collect();
            let width = u16::try_from(text.chars().count() + 2).unwrap_or(u16::MAX);
            if root_x.saturating_add(width) > sections[1].right() {
                root_x = sections[1].x;
                root_y += 1;
            }
            if root_y >= sections[1].y + 3 {
                break;
            }
            let rect = Rect::new(root_x, root_y, width, 1);
            let scope = Scope::Root(index);
            let style = if self.scope == scope {
                Style::default().fg(Color::Black).bg(Color::Cyan)
            } else {
                Style::default().fg(Color::Cyan)
            };
            frame.render_widget(Paragraph::new(format!(" {text} ")).style(style), rect);
            self.scope_buttons.push((rect, scope));
            root_x += width + 1;
        }
        x = sections[1].x;
        for (text, action) in [
            ("+ Root", "root"),
            ("− Root", "remove"),
            ("★ Favorite", "favorite"),
        ] {
            let width = u16::try_from(text.chars().count() + 2).unwrap_or(u16::MAX);
            if x.saturating_add(width) > sections[1].right() {
                break;
            }
            let rect = Rect::new(x, sections[1].y + 3, width, 1);
            frame.render_widget(
                Paragraph::new(format!(" {text} ")).style(Style::default().fg(Color::Yellow)),
                rect,
            );
            self.action_buttons.push((rect, action));
            x += width + 1;
        }
        let (search, caret) = self.search_line(sections[2].width.saturating_sub(2));
        frame.render_widget(
            Paragraph::new(search).block(Block::default().borders(Borders::ALL)),
            sections[2],
        );
        if self.mode == Mode::Picker && sections[2].width > 10 && sections[2].height > 2 {
            frame.set_cursor_position((
                sections[2].x.saturating_add(1).saturating_add(caret),
                sections[2].y.saturating_add(1),
            ));
        }
        let favorite_paths: BTreeSet<_> = self
            .config
            .favorites
            .iter()
            .map(|path| config::normalize(path))
            .collect();
        let root_paths: BTreeSet<_> = self
            .config
            .roots
            .iter()
            .map(|path| config::normalize(path))
            .collect();
        let rows = self
            .results
            .iter()
            .enumerate()
            .map(|(position, index)| {
                let project = &self.projects[*index];
                let marker = if self.pending.contains(&project.path) {
                    "✓"
                } else {
                    " "
                };
                let kind = if project.git { "git" } else { "dir" };
                let favorite = if favorite_paths.contains(&project.path) {
                    Span::styled(
                        "★",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(" ")
                };
                let root = if root_paths.contains(&project.path) {
                    Span::styled(
                        "⌂",
                        Style::default()
                            .fg(Color::LightBlue)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::raw(" ")
                };
                let line = Line::from(vec![
                    Span::raw(format!(" {marker} {kind:<3} ")),
                    favorite,
                    root,
                    Span::raw(format!(
                        " {:<22}  {}",
                        discovery::basename(&project.path),
                        project.path.display()
                    )),
                ]);
                let style = if position == self.highlighted {
                    Style::default().fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(line).style(style)
            })
            .collect::<Vec<_>>();
        let list = List::new(rows)
            .block(Block::default().title("Projects").borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        self.list_area = Rect::new(
            sections[3].x.saturating_add(1),
            sections[3].y.saturating_add(1),
            sections[3].width.saturating_sub(2),
            sections[3].height.saturating_sub(2),
        );
        frame.render_stateful_widget(list, sections[3], &mut self.list_state);
        frame.render_widget(
            Paragraph::new(self.status.as_str()).wrap(Wrap { trim: true }),
            sections[4],
        );
        let help_hint = format!(" {} Shortcuts ", self.config.keybindings.help);
        self.help_button = Rect::new(
            sections[5].x,
            sections[5].y,
            u16::try_from(help_hint.chars().count())
                .unwrap_or(u16::MAX)
                .min(sections[5].width),
            1,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{help_hint}· Enter create/review · {} select · Esc close",
                self.config.keybindings.select,
            ))
            .style(Style::default().fg(Color::DarkGray)),
            sections[5],
        );
        match self.mode {
            Mode::Picker => {}
            Mode::Help => {
                let area = centered(frame.area(), 85, 90);
                frame.render_widget(Clear, area);
                let keys = &self.config.keybindings;
                let content = format!(
                    "Enter                 Create highlighted project; with a batch, review it\n\
                     Enter (batch review)  Create selected workspaces\n\
                     Up / Down (review)    Choose a selection; Delete removes it\n\
                     {}        Toggle pending selection\n\
                     Type / Backspace      Search / edit at caret\n\
                     Left / Right          Move search caret (Home / End also work)\n\
                     Ctrl+A / Ctrl+E       Start / end of search\n\
                     Ctrl+B / Ctrl+W       Caret left / delete previous word\n\
                     Ctrl+U / Ctrl+K       Delete before / after caret\n\
                     Delete                Delete next character\n\
                     Up / Down             Navigate results\n\
                     {}        Browse highlighted directory's subfolders\n\
                     {}        Browse the current directory's parent\n\
                     {}        Next search scope\n\
                     {}        Previous search scope\n\
                     {}        Toggle favorite\n\
                     {}        Add highlighted project as a root\n\
                     + Root (mouse)        Enter a full directory path\n\
                     {}        Remove highlighted root; confirm active root otherwise\n\
                     {}        Show / hide shortcuts\n\
                     Esc                   Back from overlay or review; close picker\n\n\
                     Click a row to highlight it; click scopes and action labels above.\n\
                     Edit config.json to change these picker shortcuts.",
                    keys.select,
                    keys.enter_directory,
                    keys.parent_directory,
                    keys.scope,
                    keys.scope_backward,
                    keys.favorite,
                    keys.add_root,
                    keys.remove_root,
                    keys.help,
                );
                frame.render_widget(
                    Paragraph::new(content)
                        .wrap(Wrap { trim: false })
                        .block(Block::default().title("Shortcuts").borders(Borders::ALL)),
                    area,
                );
            }
            Mode::BatchReview => {
                let height = u16::try_from(self.pending.len())
                    .unwrap_or(u16::MAX)
                    .saturating_add(4)
                    .clamp(7, 12);
                let area = centered_cells(frame.area(), 82, height);
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Block::default()
                        .title(" Review batch ")
                        .borders(Borders::ALL),
                    area,
                );
                let inner = Rect::new(
                    area.x.saturating_add(1),
                    area.y.saturating_add(1),
                    area.width.saturating_sub(2),
                    area.height.saturating_sub(2),
                );
                let parts = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Min(1),
                        Constraint::Length(1),
                    ])
                    .split(inner);
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} selected · remove mistakes before creating",
                        self.pending.len()
                    ))
                    .style(Style::default().fg(Color::Cyan)),
                    parts[0],
                );
                self.review_list_area = parts[1];
                let rows = self
                    .pending
                    .iter()
                    .map(|path| ListItem::new(compact_path(path, parts[1].width.saturating_sub(2))))
                    .collect::<Vec<_>>();
                frame.render_stateful_widget(
                    List::new(rows)
                        .highlight_symbol("› ")
                        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan)),
                    parts[1],
                    &mut self.review_state,
                );
                let mut x = parts[2].x;
                for (label, action) in [
                    (" Enter create ", "create"),
                    (" Del remove ", "drop"),
                    (" Esc back ", "back"),
                ] {
                    let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
                    let rect = Rect::new(x, parts[2].y, width, 1);
                    frame.render_widget(
                        Paragraph::new(label).style(Style::default().fg(Color::Yellow)),
                        rect,
                    );
                    self.dialog_buttons.push((rect, action));
                    x = x.saturating_add(width);
                }
            }
            Mode::RootConfirm => {
                let area = centered_cells(frame.area(), 72, 6);
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Block::default()
                        .title(" Remove active root? ")
                        .borders(Borders::ALL),
                    area,
                );
                let path = self.root_to_remove.as_deref().unwrap_or(Path::new(""));
                frame.render_widget(
                    Paragraph::new(compact_path(path, area.width.saturating_sub(4))),
                    Rect::new(
                        area.x.saturating_add(2),
                        area.y.saturating_add(2),
                        area.width.saturating_sub(4),
                        1,
                    ),
                );
                let mut x = area.x.saturating_add(2);
                for (label, action) in [(" Enter remove ", "remove_root"), (" Esc back ", "back")] {
                    let width = u16::try_from(label.chars().count()).unwrap_or(u16::MAX);
                    let rect = Rect::new(x, area.bottom().saturating_sub(2), width, 1);
                    frame.render_widget(
                        Paragraph::new(label).style(Style::default().fg(Color::Yellow)),
                        rect,
                    );
                    self.dialog_buttons.push((rect, action));
                    x = x.saturating_add(width);
                }
            }
            Mode::RootInput => {
                let area = centered(frame.area(), 75, 25);
                frame.render_widget(Clear, area);
                frame.render_widget(
                    Paragraph::new(format!(
                        "Add root by full directory path (e.g. ~/ZedProjects/pi-sandbox)\n\n{}▏\n\nEnter: add   Esc: back",
                        self.root_input
                    ))
                    .block(Block::default().borders(Borders::ALL)),
                    area,
                );
            }
        }
    }
}

fn compact_path(path: &Path, width: u16) -> String {
    let home = config::normalize(&config::home());
    let text = path
        .strip_prefix(&home)
        .map(|relative| format!("~/{}", relative.display()))
        .unwrap_or_else(|_| path.display().to_string());
    let width = usize::from(width);
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        text
    } else {
        format!(
            "…{}",
            chars[chars.len() - width.saturating_sub(1)..]
                .iter()
                .collect::<String>()
        )
    }
}

fn centered_cells(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = area.width.saturating_mul(width) / 100;
    let h = area.height.saturating_mul(height) / 100;
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h.max(5),
    )
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
}

fn bound(bindings: &str, key: KeyEvent) -> bool {
    bindings.split(',').any(|binding| {
        let mut modifiers = KeyModifiers::NONE;
        let mut code = None;
        for part in binding.trim().to_ascii_lowercase().split('+') {
            match part {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                "alt" => modifiers |= KeyModifiers::ALT,
                "enter" => code = Some(KeyCode::Enter),
                "esc" | "escape" => code = Some(KeyCode::Esc),
                "tab" => code = Some(KeyCode::Tab),
                "up" => code = Some(KeyCode::Up),
                "down" => code = Some(KeyCode::Down),
                character if character.chars().count() == 1 => {
                    code = character.chars().next().map(KeyCode::Char)
                }
                _ => return false,
            }
        }
        let shifted_question = binding.trim() == "?"
            && key.modifiers == KeyModifiers::SHIFT
            && matches!(key.code, KeyCode::Char('?') | KeyCode::Char('/'));
        let same_code = match (code, key.code) {
            (Some(KeyCode::Char(expected)), KeyCode::Char(actual)) => {
                expected.eq_ignore_ascii_case(&actual)
            }
            (Some(KeyCode::Tab), KeyCode::BackTab) if modifiers == KeyModifiers::SHIFT => {
                return key.modifiers == KeyModifiers::SHIFT || key.modifiers.is_empty();
            }
            (expected, actual) => expected == Some(actual),
        };
        (same_code && modifiers == key.modifiers) || shifted_question
    })
}

pub fn run() -> io::Result<()> {
    let path = config::config_path();
    let config = config::load(&path)?;
    let mut app = App::new(config, path);
    let mut terminal = ratatui::init();
    let keyboard = execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .is_ok();
    let result = (|| {
        execute!(io::stdout(), EnableMouseCapture)?;
        let mut dirty = true;
        while app.running {
            dirty |= app.receive();
            if dirty {
                terminal.draw(|frame| app.draw(frame))?;
                dirty = false;
            }
            if event::poll(Duration::from_millis(40))? {
                match event::read()? {
                    Event::Key(key) => app.key(key),
                    Event::Mouse(mouse) => app.mouse(mouse),
                    _ => {}
                }
                dirty = true;
            }
        }
        Ok(())
    })();
    let _ = execute!(io::stdout(), DisableMouseCapture);
    if keyboard {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}
