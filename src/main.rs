mod args;
mod blocks;
mod config;
mod misc;
mod pickers;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Parser;
use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};
use gtk4 as gtk;

use config::ConfigLoad;
use pickers::{Picker, PickerKind};

type PickerCurr = Arc<Mutex<Option<Arc<dyn Picker>>>>;
type PickerList = Arc<Mutex<Vec<Arc<dyn Picker>>>>;

const SIGNAL_APPS: i32 = libc::SIGUSR1;
const SIGNAL_RELOAD: i32 = libc::SIGWINCH;
const GTK_APP_ID: &str = "xyz.gall.pickers";
const PID_FILE_PATH: &str = "gall-daemon.lock";
const LOCAL_PATH: &str = ".config/gall";
const DESKTOP_PATHS: [&str; 3] = [
    "/usr/share/applications/",
    "/usr/local/share/applications/",
    "~/.local/share/applications/",
];

struct AppState {
    config_path: PathBuf,
    styles_path: PathBuf,
    config: Arc<ConfigLoad>,
}

impl AppState {
    fn new(config_path: PathBuf, styles_path: PathBuf, config: Arc<ConfigLoad>) -> Self {
        Self {
            config_path,
            styles_path,
            config,
        }
    }
}

struct GallApp {
    app: Application,
    state: Arc<Mutex<AppState>>,
    window: Arc<ApplicationWindow>,
    pickers: PickerList,
    picker: PickerCurr,
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

        Self {
            app: app.clone(),
            state,
            window: Arc::new(window),
            pickers: Arc::new(Mutex::new(Vec::with_capacity(PickerKind::None as usize))),
            picker: Arc::new(Mutex::new(None)),
        }
    }

    pub fn load(&self, app: &Arc<GallApp>) -> &Self {
        let state = self.state.clone();
        let locked = state.lock().unwrap();
        misc::apply_styles(&locked.styles_path);

        {
            let mut pickers_lock = self.pickers.lock().unwrap();

            for kind in PickerKind::variants() {
                let win = self.window.clone();
                let cpick = PickerKind::from_kind(&kind, app.clone());

                cpick.load(&locked.config);
                cpick.if_done(Box::new(move || win.hide()));
                pickers_lock.push(cpick);
            }
        }
        drop(locked);

        {
            let picker = self.picker.clone();
            let pickers = self.pickers.clone();
            let window = self.window.clone();

            glib::source::unix_signal_add_local(SIGNAL_APPS, move || {
                let locked = state.lock().unwrap();

                if locked.config.css_reload {
                    misc::apply_styles(&locked.styles_path);
                }

                if window.is_visible() {
                    window.hide();
                } else {
                    picker_switch(&pickers, &picker, PickerKind::Apps);
                    window.show();
                }

                glib::ControlFlow::Continue
            });
        }

        {
            let window = self.window.clone();

            window.connect_close_request(move |window| {
                window.hide();

                glib::Propagation::Stop
            });
        }

        self
    }
}

fn picker_switch(pickers: &PickerList, picker: &PickerCurr, kind: PickerKind) {
    let mut picker_lock = picker.lock().unwrap();
    let pickers_lock = pickers.lock().unwrap();
    let cur_kind = picker_lock.as_ref().map_or(PickerKind::None, |ref p| p.kind());

    let picker = &pickers_lock[kind as usize];
    if picker.show(cur_kind) {
        *picker_lock = Some(picker.clone());
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
            config::load_config(&config),
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
