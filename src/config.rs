use crate::error::AppResult;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tags: TagsConfig,
    #[serde(default)]
    pub projects: ProjectsConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub keys: HashMap<String, String>,
    #[serde(default)]
    pub pomodoro: PomodoroConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsConfig {
    #[serde(default = "default_tags")]
    pub default: Vec<String>,
}

impl Default for TagsConfig {
    fn default() -> Self {
        Self {
            default: default_tags(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default = "default_projects")]
    pub default: Vec<String>,
}

impl Default for ProjectsConfig {
    fn default() -> Self {
        Self {
            default: default_projects(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_show_done")]
    pub show_done_in_all: bool,
    #[serde(default = "default_week_days")]
    pub week_days: i64,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_done_in_all: default_show_done(),
            week_days: default_week_days(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PomodoroConfig {
    #[serde(default = "default_pomo_minutes")]
    pub minutes: u64,
}

impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            minutes: default_pomo_minutes(),
        }
    }
}

fn default_tags() -> Vec<String> {
    vec![
        "work".into(),
        "personal".into(),
        "urgent".into(),
        "idea".into(),
        "learning".into(),
        "followup".into(),
    ]
}

fn default_projects() -> Vec<String> {
    vec![
        "Inbox".into(),
        "Work".into(),
        "Personal".into(),
        "Learning".into(),
    ]
}

fn default_show_done() -> bool {
    false
}
fn default_week_days() -> i64 {
    7
}
fn default_pomo_minutes() -> u64 {
    25
}

/// Resolved paths for app data and config.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub db_file: PathBuf,
}

impl Paths {
    pub fn resolve() -> AppResult<Self> {
        // Prefer XDG-like paths regardless of OS: ~/.config/todotui and
        // ~/.local/share/todotui. Fall back to `directories` only if HOME is
        // missing.
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let (config_dir, data_dir) = match home {
            Some(h) => (
                h.join(".config").join("todotui"),
                h.join(".local").join("share").join("todotui"),
            ),
            None => {
                let pd = ProjectDirs::from("", "", "todotui").ok_or_else(|| {
                    crate::error::AppError::Other("could not resolve project dirs".into())
                })?;
                (pd.config_dir().to_path_buf(), pd.data_dir().to_path_buf())
            }
        };

        let config_file = config_dir.join("config.toml");
        let db_file = data_dir.join("tasks.db");

        Ok(Self {
            config_dir,
            data_dir,
            config_file,
            db_file,
        })
    }

    pub fn ensure_dirs(&self) -> AppResult<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.data_dir)?;
        Ok(())
    }
}

/// Load the config from disk; if it does not exist, write defaults and return them.
pub fn load_or_init(paths: &Paths) -> AppResult<Config> {
    paths.ensure_dirs()?;
    if !paths.config_file.exists() {
        let cfg = Config::default();
        write_config(&paths.config_file, &cfg)?;
        return Ok(cfg);
    }
    let body = fs::read_to_string(&paths.config_file)?;
    let cfg: Config = toml::from_str(&body)?;
    Ok(cfg)
}

fn write_config(path: &Path, cfg: &Config) -> AppResult<()> {
    let body = toml::to_string_pretty(cfg)?;
    let header = "# todotui configuration\n\
                  # Tags are predefined here; the app will not create tags on the fly.\n\n";
    fs::write(path, format!("{header}{body}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_seed_tags() {
        let c = Config::default();
        assert!(c.tags.default.contains(&"work".to_string()));
        assert!(c.projects.default.contains(&"Inbox".to_string()));
        assert_eq!(c.pomodoro.minutes, 25);
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let c = Config::default();
        let s = toml::to_string(&c).unwrap();
        let parsed: Config = toml::from_str(&s).unwrap();
        assert_eq!(parsed.tags.default, c.tags.default);
    }
}
