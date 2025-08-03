use crate::gtk;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use gtk::gdk;
use gtk::prelude::{Cast, DisplayExt, ListModelExt, MonitorExt};

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

#[inline]
pub(crate) fn daemonize() {
    unsafe {
        if libc::daemon(0, 0) != 0 {
            eprintln!("Error: \"Failed to daemonize process\"");
            std::process::exit(1);
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn unix_sched_yield() {
  unsafe {
    std::arch::asm!(
      "syscall",
      in("rax") 24,
      options(nostack, nomem)
    );
  }
}

