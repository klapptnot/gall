use crate::{gtk, Arc};

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt};

use crate::{AppEntry, ConfigLoad};

pub(crate) struct CommandError {
    pub(crate) reason: String,
    pub(crate) stderr: Option<String>,
    pub(crate) stdout: Option<String>,
}

pub(crate) fn get_full_display_size() -> (i32, i32) {
    let display = gdk::Display::default().expect("Failed to get default display");
    let model = display.monitors();

    let n_monitors = model.n_items();
    if n_monitors == 0 {
        panic!("No monitors found");
    }

    let mut x0 = i32::MAX;
    let mut y0 = i32::MAX;
    let mut x1 = i32::MIN;
    let mut y1 = i32::MIN;

    for i in 0..n_monitors {
        let obj = model
            .item(i)
            .expect(&format!("Failed to get monitor at index {}", i));
        let monitor = obj
            .downcast::<gdk::Monitor>()
            .expect("ListModel item is not a Monitor");

        let geom = monitor.geometry();
        x0 = x0.min(geom.x());
        y0 = y0.min(geom.y());
        x1 = x1.max(geom.x() + geom.width());
        y1 = y1.max(geom.y() + geom.height());
    }

    let width = x1 - x0;
    let height = y1 - y0;
    (width, height)
}

pub(crate) fn apply_styles(filepath: &PathBuf) {
    let provider = gtk::CssProvider::new();
    provider.load_from_path(filepath);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    } else {
        eprintln!("No display found for applying CSS.");
    }
}

#[inline]
pub(crate) fn get_local_path(name: &str) -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .expect("HOME env var is not set")
        .join(crate::LOCAL_PATH)
        .join(name)
}

pub(crate) fn launch_detached(exec_command: &str) -> Result<(), CommandError> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(exec_command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| CommandError {
            reason: format!("Failed to spawn process: {}", e),
            stderr: None,
            stdout: None,
        })?;

    let start_time = Instant::now();
    let timeout = Duration::from_secs(3);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }

                let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
                child.stdout.take().map(|mut v| v.read_to_end(&mut stdout));
                child.stderr.take().map(|mut v| v.read_to_end(&mut stderr));

                return Err(CommandError {
                    reason: format!("Command failed with exit code: {}", status.code().unwrap_or(-1)),
                    stderr: Some(String::from_utf8_lossy(&stderr).into_owned()),
                    stdout: Some(String::from_utf8_lossy(&stdout).into_owned()),
                });
            }
            Ok(None) => {
                if start_time.elapsed() >= timeout {
                    return Ok(());
                }

                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                return Err(CommandError {
                    reason: format!("Error waiting for process: {}", e),
                    stderr: None,
                    stdout: None,
                });
            }
        }
    }
}

pub(crate) fn expand_tilde<P: AsRef<Path>>(path: P) -> Option<std::path::PathBuf> {
    let path = path.as_ref();

    if !path.starts_with("~") {
        return Some(path.to_path_buf());
    }

    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .map(|mut p| {
            if path.as_os_str().len() > 1 {
                p.push(&path.as_os_str().to_string_lossy()[2..]);
            }
            p
        })
}

pub(crate) fn load_config(filepath: &PathBuf) -> Arc<ConfigLoad> {
    let desktop_paths = crate::DESKTOP_PATHS.map(expand_tilde).map(Option::unwrap);

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

#[inline]
pub(crate) fn fuzzy(s: &str, pattern: &str) -> bool {
    let mut s_bytes = s.bytes();

    for pattern_byte in pattern.bytes() {
        let pattern_lower = pattern_byte.to_ascii_lowercase();
        let mut found = false;

        while let Some(s_byte) = s_bytes.next() {
            let s_lower = s_byte.to_ascii_lowercase();
            if s_lower == pattern_lower {
                found = true;
                break;
            }
        }

        if !found {
            return false;
        }
    }

    true
}

pub(crate) fn send_signal(sig: i32) {
    let Ok(pid) = read_pid_file() else {
        println!("PID file missing or bad formatted!");
        std::process::exit(1);
    };

    if !process_is_running(pid) {
        eprintln!("Process with PID {} is already dead!", pid);
        std::process::exit(1);
    }

    unsafe {
        if libc::kill(pid, sig) != 0 {
            eprintln!("Failed to send signal to daemon (PID: {})", pid);
            std::process::exit(1);
        }
    }
}

pub(crate) fn read_pid_file() -> Result<i32, Box<dyn std::error::Error>> {
    let pidfile = pid_file_path();

    if pidfile.exists() && pidfile.is_file() {
        let pid_str = std::fs::read_to_string(pidfile)?;
        Ok(pid_str.trim().parse()?)
    } else {
        Err("".into())
    }
}

pub(crate) fn daemon_is_running() -> bool {
    let pidfile = pid_file_path();

    if !pidfile.exists() || !pidfile.is_file() {
        return false;
    }

    let Ok(pid) = read_pid_file() else {
        println!("PID file missing or bad formatted!");
        std::process::exit(1);
    };

    process_is_running(pid)
}

#[inline]
pub(crate) fn process_is_running(pid: i32) -> bool {
    std::fs::metadata(format!("/proc/{}", pid)).is_ok()
}

#[inline]
pub(crate) fn pid_file_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .map(|p| p.join(crate::PID_FILE_PATH))
        .expect("Could not get XDG_RUNTIME_DIR")
}

#[inline]
pub(crate) fn daemonize() {
    unsafe {
        if libc::daemon(0, 0) != 0 {
            eprintln!("Error: \"Failed to daemonize process\"");
            std::process::exit(1);
        }
    }

    let pidfile = pid_file_path();

    let pid = std::process::id();
    std::fs::write(pidfile, pid.to_string()).expect("Failed to write PID file");
}
