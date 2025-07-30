use crate::misc;

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct AppEntry {
    pub name: String,
    #[serde(rename = "generic")]
    pub genc: Option<String>,
    #[serde(rename = "description")]
    pub desc: Option<String>,
    pub icon: Option<String>,
    pub exec: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigLoad {
    pub css_reload: bool,
    pub terminal: Option<String>,
    pub apps: Vec<AppEntry>,
}

pub(crate) fn load_config(filepath: &PathBuf) -> Arc<ConfigLoad> {
    let desktop_paths = crate::DESKTOP_PATHS.map(misc::expand_tilde).map(Option::unwrap);

    let cfg = std::fs::read_to_string(&filepath)
        .map_err(|e| {
            eprintln!(
                "Error reading config file {}: {}",
                filepath.to_str().unwrap_or("<none>"),
                e
            );
        })
        .and_then(|data| {
            toml::from_str::<ConfigLoad>(&data).map_err(|e| {
                eprintln!(
                    "Error loading config file {}: {}",
                    filepath.to_str().unwrap_or("<none>"),
                    e.message()
                );
            })
        });

    let mut cfg = match cfg {
        Ok(cfg) => cfg,
        Err(_) => ConfigLoad {
            css_reload: false,
            terminal: None,
            apps: Vec::new(),
        },
    };

    let apps = load_apps(&desktop_paths, &cfg.terminal);
    cfg.apps.extend(apps);

    Arc::new(cfg)
}

fn parse_desktop_file<P: AsRef<Path>>(filepath: P, term: &Option<String>) -> Option<AppEntry> {
    let content = std::fs::read_to_string(filepath).ok()?;

    let start_idx = content.find("[Desktop Entry]")?;
    let section_start = start_idx + 15;

    let section = if let Some(next_section) = content[section_start..].find("\n[") {
        &content[section_start..section_start + next_section]
    } else {
        &content[section_start..]
    };

    let mut fields = HashMap::new();
    let needed_fields = [
        "Name",
        "GenericName",
        "Comment",
        "Icon",
        "Exec",
        "Type",
        "NoDisplay",
        "Terminal",
    ];

    for line in section.lines() {
        let line = line.trim();
        if line.contains('=') && !line.starts_with('#') {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                if needed_fields.contains(&key) {
                    fields.insert(key, value.trim());
                }
            }
        }
    }

    if fields
        .get("NoDisplay")
        .map_or("", |v| v)
        .to_string()
        .to_lowercase()
        == "true"
    {
        return None;
    }

    let term_app = fields
        .get("Terminal")
        .map_or(false, |v| v.eq_ignore_ascii_case("true"));

    if term_app && term.is_none() {
        return None;
    }

    if fields.get("Type").map_or("", |v| v).to_string() != "Application" {
        return None;
    }

    let name = fields.get("Name")?;
    let exec_cmd = fields.get("Exec")?;

    if name.is_empty() || exec_cmd.is_empty() {
        return None;
    }

    let cleaned_exec = exec_cmd
        .replace(" %F", "")
        .replace(" %f", "")
        .replace(" %U", "")
        .replace(" %u", "")
        .replace("=%F", "")
        .replace("=%f", "")
        .replace("=%U", "")
        .replace("=%u", "");

    let cleaned_exec = if term_app {
        format!("{} {cleaned_exec}", term.as_ref().unwrap())
    } else {
        cleaned_exec
    };

    Some(AppEntry {
        name: name.to_string(),
        genc: fields.get("GenericName").map_or(None, |v| Some(v.to_string())),
        desc: fields.get("Comment").map_or(None, |v| Some(v.to_string())),
        icon: fields.get("Icon").map_or(None, |v| Some(v.to_string())),
        exec: cleaned_exec.to_string(),
    })
}

fn load_apps(desktop_paths: &[std::path::PathBuf], term: &Option<String>) -> Vec<AppEntry> {
    let mut apps = Vec::new();
    for path in desktop_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(filename) = entry.file_name().to_str() {
                    if filename.ends_with(".desktop") {
                        if let Some(desktop_app) = parse_desktop_file(entry.path(), &term) {
                            apps.push(desktop_app);
                        }
                    }
                }
            }
        }
    }

    let mut seen_names = HashSet::new();
    let mut unique_apps = Vec::new();

    for app in apps {
        if seen_names.insert(app.name.clone()) {
            unique_apps.push(app);
        }
    }

    unique_apps
}
