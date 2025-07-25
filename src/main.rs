use gtk4 as gtk;

use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};

mod action;
mod args;
mod blocks;
mod misc;

const SIGNAL_APPS: i32 = libc::SIGUSR1;
#[allow(dead_code)]
const SIGNAL_CLIPBOARD: i32 = libc::SIGUSR2;
const SIGNAL_RELOAD: i32 = libc::SIGWINCH;
const GTK_APP_ID: &str = "xyz.gall.pickers";
const PID_FILE_PATH: &str = "gall-daemon.lock";
const LOCAL_PATH: &str = ".config/gall";
const DESKTOP_PATHS: [&str; 3] = [
    "/usr/share/applications/",
    "/usr/local/share/applications/",
    "~/.local/share/applications/",
];

#[derive(Debug, Deserialize)]
struct ConfigLoad {
    css_reload: bool,
    terminal: Option<String>,
    apps: Vec<AppEntry>,
}

#[derive(Debug, Deserialize)]
struct AppEntry {
    name: String,
    #[serde(rename = "generic")]
    genc: Option<String>,
    #[serde(rename = "description")]
    desc: Option<String>,
    icon: Option<String>,
    exec: String,
}

#[derive(PartialEq)]
enum PickerKind {
    Apps,
    #[allow(dead_code)]
    Clipboard,
    None,
}

struct AppState {
    config: PathBuf,
    styles: PathBuf,
    visible: bool,
    pick_kind: PickerKind,
    name_fuzz: bool,
    selected: u32,
    css_reload: bool,
    all_apps: Vec<AppEntry>,
    fil_apps: u32,
}

impl AppState {
    fn new(config: PathBuf, styles: PathBuf) -> Self {
        Self {
            config,
            styles,
            visible: false,
            pick_kind: PickerKind::None,
            name_fuzz: true,
            selected: 0,
            css_reload: false,
            all_apps: Vec::new(),
            fil_apps: 0,
        }
    }
}

struct Picker {
    mainbox: gtk::Box,
    search_input: gtk::Entry,
    toggle_btn: gtk::Button,
    listbox: gtk::ListBox,
}

struct GallApp {
    app: Application,
    state: Arc<Mutex<AppState>>,
    window: ApplicationWindow,
    pickers: [Arc<Picker>; 1],
}

impl GallApp {
    pub fn new(app: &Application, state: Arc<Mutex<AppState>>) -> Self {
        let (w, h) = misc::get_full_display_size();
        let (w, h) = (w as f32 * 0.3, h as f32 * 0.4);

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Gall")
            .default_width(w as i32)
            .default_height(h as i32)
            .decorated(false)
            .build();

        let picker = Arc::new(blocks::generic_picker_box());

        Self {
            app: app.clone(),
            state,
            window,
            pickers: [picker.clone()],
        }
    }

    pub fn load(&self, app: &Arc<GallApp>) -> &Self {
        let state = self.state.clone();
        let mut locked = state.lock().unwrap();
        misc::apply_styles(&locked.styles);
        let config = misc::load_config(&locked.config);

        locked.all_apps = config.apps;
        locked.css_reload = config.css_reload;

        {
            let selfr = app.clone();
            let picker = selfr.pickers[0].clone();
            let window = selfr.window.clone();
            let search_input = picker.search_input.clone();
            let state = selfr.state.clone();
            let listbox = picker.listbox.clone();

            glib::source::unix_signal_add_local(SIGNAL_APPS, move || {
                search_input.set_text("");
                let mut locked = state.lock().unwrap();
                locked.selected = 0;
                listbox.select_row(listbox.row_at_index(0).as_ref());

                locked.visible = !locked.visible;

                if locked.css_reload {
                    misc::apply_styles(&locked.styles);
                }

                if locked.pick_kind == PickerKind::Apps {
                    if !locked.visible {
                        window.hide();
                    } else {
                        window.show();
                        search_input.grab_focus();
                    }

                    return glib::ControlFlow::Continue;
                }

                // !!visible
                if locked.visible {
                    window.show();
                }

                locked.pick_kind = PickerKind::Apps;
                search_input.grab_focus();

                drop(locked); // load_app_picker locks mutex

                selfr.load_app_picker();

                glib::ControlFlow::Continue
            });
        }

        {
            // let selfr = app.clone();
            let picker = self.pickers[0].clone();
            let window = self.window.clone();
            let search_input = picker.search_input.clone();
            let listbox = picker.listbox.clone();
            let state = self.state.clone();

            window.connect_close_request(move |window| {
                let mut locked = state.lock().unwrap();
                locked.visible = false;
                window.hide();

                search_input.set_text("");
                listbox.select_row(listbox.row_at_index(0).as_ref());
                glib::Propagation::Stop
            });
        }

        action::set_picker_control(&self, &self.pickers[0]);
        action::app_picker_control(&self, &self.pickers[0]);

        self
    }

    pub fn load_app_picker(&self) -> &Self {
        let state = self.state.clone();
        let picker = &self.pickers[0];

        self.window.set_child(Some(&picker.mainbox));
        blocks::apps_populate_list(&picker.listbox, &state, "");
        picker.search_input.grab_focus();

        let _ = &picker.toggle_btn.set_icon_name("edit-find-symbolic");
        let _ = &picker.toggle_btn.set_tooltip_text(Some("Search by name"));

        self
    }
}

fn gtk_main(config: PathBuf, styles: PathBuf, open_on_load: bool) -> glib::ExitCode {
    let app = Application::builder()
        .application_id(GTK_APP_ID)
        .flags(ApplicationFlags::FLAGS_NONE | ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();

    app.connect_command_line(|app, _command_line| {
        // <<<< DON'T. TRY. TO. PARSE. ARGS. GTK. PERIOD.
        app.activate();

        glib::ExitCode::SUCCESS
    });

    app.connect_activate(move |app| {
        let state = Arc::new(Mutex::new(AppState::new(
            config.to_path_buf(),
            styles.to_path_buf(),
        )));
        let app_win = Arc::new(GallApp::new(app, state));
        app_win.load(&app_win);

        println!("  init () ➜ 🩵!");
    });

    if !open_on_load {
        misc::daemonize();
    }

    app.run()
}

fn main() {
    let cli = args::Cli::parse();

    match cli.command {
        args::Commands::Start(args) => {
            if misc::daemon_is_running() {
                eprintln!("Process is already running!");
                std::process::exit(0)
            }

            let config = args.config.map_or(misc::get_local_path("pickers.toml"), |p| p);
            let styles = args.styles.map_or(misc::get_local_path("pickers.css"), |p| p);

            println!("  Styles path: {}", styles.display());
            println!("  Config path: {}", config.display());
            println!(
                "  Daemonize: {}",
                if !args.here { "enabled" } else { "disabled" }
            );

            gtk_main(config, styles, args.here);
        }
        args::Commands::Stop(args) => {
            if !misc::daemon_is_running() {
                eprintln!("Process is already dead!");
                std::process::exit(0)
            }

            if args.force {
                println!("🛑 Force shutdown requested...");
                misc::send_signal(libc::SIGKILL);
            } else {
                println!("🛑 Stopping daemon...");
                misc::send_signal(libc::SIGINT);

                let pid = misc::read_pid_file().expect("PID file missing or bad formatted!");

                std::thread::sleep(std::time::Duration::from_millis(500));
                if misc::process_is_running(pid) {
                    eprintln!("Process hasn't stopped after 500ms, try `--force`");
                    std::process::exit(1);
                }
            }

            std::fs::remove_file(misc::pid_file_path()).expect("Unable to remove PID file!");
        }
        args::Commands::Apps => {
            println!("🔄 Toggling launcher visibility...");
            misc::send_signal(SIGNAL_APPS);
        }
        args::Commands::Reload => {
            println!("♻️  Reloading daemon configuration...");
            misc::send_signal(SIGNAL_RELOAD);
        }
    }
}
