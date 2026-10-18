use crate::config::{self, Config, DiscoveryProfile, RuleAction};
use globset::{Glob, GlobMatcher};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

struct CompiledRule {
    priority: i32,
    index: usize,
    action: RuleAction,
    matcher: GlobMatcher,
    // Literal paths allow an included descendant to be reached through an excluded parent.
    exception: Option<PathBuf>,
}

pub struct Policy {
    rules: Vec<CompiledRule>,
    macos_allowed: Vec<PathBuf>,
    names: Vec<GlobMatcher>,
    global: Gitignore,
    git_files: Mutex<HashMap<PathBuf, Gitignore>>,
}

impl Policy {
    pub fn new(config: &Config) -> Result<Self, String> {
        let profile = match config.discovery_profile {
            DiscoveryProfile::Auto if cfg!(target_os = "macos") => DiscoveryProfile::Macos,
            DiscoveryProfile::Auto => DiscoveryProfile::Unix,
            other => other,
        };
        let mut rules = Vec::new();
        let home = config::home();
        let macos_allowed = if matches!(profile, DiscoveryProfile::Macos) {
            vec![config::project_path(&home), PathBuf::from("/Users/Shared")]
        } else {
            Vec::new()
        };
        let defaults: &[&str] = match profile {
            DiscoveryProfile::Macos => &[
                "/System",
                "/Library",
                "/Applications",
                "/dev",
                "/private",
                "/etc",
                "/tmp",
                "/var",
                "/Volumes",
                "/cores",
                "/Network",
                "/usr",
                "/bin",
                "/sbin",
                "/opt",
                "/Users/*/Library",
                "~/Library",
            ],
            DiscoveryProfile::Unix | DiscoveryProfile::Auto => &[
                "/proc", "/sys", "/dev", "/run", "/tmp", "/var", "/etc", "/usr", "/bin", "/sbin",
                "/lib", "/lib64", "/opt", "/mnt", "/media", "/snap",
            ],
        };
        for (index, pattern) in defaults.iter().enumerate() {
            rules.push(Self::compile(
                pattern,
                0,
                index,
                RuleAction::Exclude,
                &home,
            )?);
        }
        for (index, rule) in config.rules.iter().enumerate() {
            rules.push(Self::compile(
                &rule.path,
                rule.priority,
                defaults.len() + index,
                rule.action,
                &home,
            )?);
        }
        let names = config
            .excluded_names
            .iter()
            .filter(|name| !config.include_hidden || name.as_str() != ".*")
            .map(|name| {
                Glob::new(name)
                    .map(|glob| glob.compile_matcher())
                    .map_err(|err| err.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (global, _) = GitignoreBuilder::new("/").build_global();
        Ok(Self {
            rules,
            macos_allowed,
            names,
            global,
            git_files: Mutex::new(HashMap::new()),
        })
    }

    fn compile(
        pattern: &str,
        priority: i32,
        index: usize,
        action: RuleAction,
        home: &Path,
    ) -> Result<CompiledRule, String> {
        let path = if pattern == "~" {
            home.display().to_string()
        } else if let Some(suffix) = pattern.strip_prefix("~/") {
            home.join(suffix).display().to_string()
        } else {
            pattern.to_owned()
        };
        if !path.starts_with('/') {
            return Err(format!(
                "Discovery rule must use an absolute path or ~/ prefix: {pattern}"
            ));
        }
        let matcher = Glob::new(&path)
            .map_err(|err| format!("Invalid discovery rule {pattern}: {err}"))?
            .compile_matcher();
        let exception = (action == RuleAction::Include && !path.contains(['*', '?', '[', '{']))
            .then(|| PathBuf::from(&path));
        Ok(CompiledRule {
            priority,
            index,
            action,
            matcher,
            exception,
        })
    }

    fn git_decision(&self, path: &Path) -> Option<bool> {
        let global = self.global.matched_path_or_any_parents(path, true);
        let mut result = if global.is_none() {
            None
        } else {
            Some(global.is_ignore())
        };
        let mut parents: Vec<_> = path.ancestors().skip(1).collect();
        parents.reverse();
        let mut cache = self.git_files.lock().expect("gitignore cache poisoned");
        for parent in parents {
            let matcher = cache.entry(parent.to_path_buf()).or_insert_with(|| {
                let mut builder = GitignoreBuilder::new(parent);
                let git = parent.join(".git");
                if git.is_dir() && git.join("info/exclude").is_file() {
                    builder.add(git.join("info/exclude"));
                }
                if parent.join(".gitignore").is_file() {
                    builder.add(parent.join(".gitignore"));
                }
                if parent.join(".ignore").is_file() {
                    builder.add(parent.join(".ignore"));
                }
                builder.build().unwrap_or_else(|_| Gitignore::empty())
            });
            let matched = matcher.matched_path_or_any_parents(path, true);
            if !matched.is_none() {
                result = Some(matched.is_ignore());
            }
        }
        result
    }

    fn in_scope(&self, path: &Path) -> bool {
        self.macos_allowed.is_empty()
            || self
                .macos_allowed
                .iter()
                .any(|allowed| path.starts_with(allowed))
            || path.ancestors().any(|ancestor| {
                self.rules.iter().any(|rule| {
                    rule.action == RuleAction::Include && rule.matcher.is_match(ancestor)
                })
            })
    }

    pub fn visible(&self, path: &Path, config: &Config) -> bool {
        if !self.in_scope(path)
            || path
                .components()
                .any(|component| component.as_os_str() == ".git")
        {
            return false;
        }
        let mut winner: Option<(i32, usize, bool)> = None;
        for (index, ancestor) in path
            .ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .enumerate()
        {
            if ancestor == Path::new("/") {
                continue;
            }
            let name = ancestor.file_name().unwrap_or_default();
            if (!config.include_hidden && name.to_string_lossy().starts_with('.'))
                || self.names.iter().any(|matcher| matcher.is_match(name))
            {
                let candidate = (0, index, false);
                if winner
                    .is_none_or(|previous| (candidate.0, candidate.1) >= (previous.0, previous.1))
                {
                    winner = Some(candidate);
                }
            }
            for rule in &self.rules {
                if rule.matcher.is_match(ancestor) {
                    let candidate = (
                        rule.priority,
                        index * (self.rules.len() + 1) + rule.index,
                        rule.action == RuleAction::Include,
                    );
                    if winner.is_none_or(|previous| {
                        (candidate.0, candidate.1) >= (previous.0, previous.1)
                    }) {
                        winner = Some(candidate);
                    }
                }
            }
        }
        // Git ignore rules sit between defaults and user rules at priority 10.
        if let Some(included) = self.git_decision(path)
            && winner.is_none_or(|previous| previous.0 < 10)
        {
            winner = Some((10, 0, !included));
        }
        winner.is_none_or(|(_, _, included)| included)
    }

    pub fn traverse(&self, path: &Path, config: &Config) -> bool {
        if path
            .components()
            .any(|component| component.as_os_str() == ".git")
        {
            return false;
        }
        self.visible(path, config)
            || self
                .macos_allowed
                .iter()
                .any(|allowed| allowed.starts_with(path))
            || self.rules.iter().any(|rule| {
                rule.exception
                    .as_ref()
                    .is_some_and(|exception| exception.starts_with(path) && exception != path)
            })
    }
}
