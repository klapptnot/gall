mod args;
mod blocks;
mod config;
mod misc;
mod pickers;
mod socket;

use gtk4 as gtk;

use clap::Parser;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow};

use config::ConfigLoad;
use pickers::{Picker, PickerKind};
use socket::AppMessage;

type PickerCurr = Arc<Mutex<Option<Arc<dyn Picker>>>>;
type PickerList = Arc<Mutex<Vec<Arc<dyn Picker>>>>;

const GTK_APP_ID: &str = "xyz.gall.pickers";
const LOCAL_PATH: &str = ".config/gall";
const DESKTOP_PATHS: [&str; 3] = [
    "/usr/share/applications/",
    "/usr/local/share/applications/",
    "~/.local/share/applications/",
];

struct AppState {
    config_path: PathBuf,
    styles_path: PathBuf,
    msg_queue: socket::MessageQueue,
    config: Arc<ConfigLoad>,
}

impl AppState {
    fn new(config_path: PathBuf, styles_path: PathBuf, msg_queue: socket::MessageQueue, config: Arc<ConfigLoad>) -> Self {
        Self {
            config_path,
            styles_path,
            msg_queue,
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

    pub fn load(&self, app: Arc<GallApp>) -> &Self {
        {
            let state = self.state.clone();
            let locked = state.lock().unwrap();
            misc::apply_styles(&locked.styles_path);

            let mut pickers_lock = self.pickers.lock().unwrap();

            for kind in PickerKind::variants() {
                let win = self.window.clone();
                let cpick = PickerKind::from_kind(&kind, app.clone());

                cpick.load(&locked.config);
                cpick.if_done(Box::new(move || win.hide()));
                pickers_lock.push(cpick);
            }

            let write_queue = locked.msg_queue.clone();
            std::thread::spawn(move || socket::start_socket_listener(write_queue));
            println!("🔌Starting socket listener on {}", socket::get_socket_path().to_str().expect("path to be valid string"));
        }

        {
            let locked = self.state.lock().unwrap();
            let queue_for_idle = Arc::clone(&locked.msg_queue);
            drop(locked);

            let state = self.state.clone();
            let window = self.window.clone();
            let picker = self.picker.clone();
            let pickers = self.pickers.clone();
            let gtk_app = self.app.clone();

            glib::idle_add_local(move || {
                let Ok(mut queue) = queue_for_idle.lock() else {
                    return glib::ControlFlow::Continue;
                };

                let Some(message) = queue.pop_front() else {
                    return glib::ControlFlow::Continue;
                };

                let message = AppMessage::from(message);
                println!("📨Got Message: {message:?}");
                match message {
                    AppMessage::TogglePicker(kind) => {
                        let locked = state.lock().unwrap();

                        if locked.config.css_reload {
                            misc::apply_styles(&locked.styles_path);
                        }

                        if window.is_visible() {
                            window.hide();
                            return glib::ControlFlow::Continue;
                        }

                        picker_switch(&pickers, &picker, kind);
                        window.show();
                    }
                    AppMessage::AppReload => {
                        let mut locked = state.lock().unwrap();
                        misc::apply_styles(&locked.styles_path);
                        locked.config = config::load_config(&locked.config_path);

                        let pickers_lock = pickers.lock().unwrap();
                        for it in &*pickers_lock {
                            it.reload(&locked.config);
                        }
                    }
                    AppMessage::AppClose => {
                        gtk_app.quit();
                    }
                    AppMessage::AppPing => {
                        let _ = socket::send_message(AppMessage::AppPing);
                    }
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

fn gtk_main(config: PathBuf, styles: PathBuf, stay_here: bool) -> glib::ExitCode {
    if !stay_here {
        misc::daemonize();
    }

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
        let message_queue: socket::MessageQueue = Arc::new(Mutex::new(std::collections::VecDeque::new()));
        let state = Arc::new(Mutex::new(AppState::new(
            config.to_path_buf(),
            styles.to_path_buf(),
            message_queue,
            config::load_config(&config),
        )));
        let app_win = Arc::new(GallApp::new(app, state));
        app_win.load(app_win.clone());
    });

    app.connect_shutdown(move |app_ref| {
        let _ = std::fs::remove_file(socket::get_socket_path());
        app_ref.quit();
    });

    glib::source::unix_signal_add(libc::SIGINT, || {
        let _ = std::fs::remove_file(socket::get_socket_path());
        glib::ControlFlow::Break
    });

    app.run()
}

fn main() {
    let cli = args::Cli::parse();

    match cli.command {
        args::Commands::Start(args) => {
            if socket::process_is_running() {
                eprintln!("Process is already running!");
                std::process::exit(0)
            }

            if socket::get_socket_path().exists() {
                std::fs::remove_file(socket::get_socket_path()).expect("Unable to unlink socket!");
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
        args::Commands::Stop => {
            if socket::process_is_running() {
                match socket::send_message(AppMessage::AppClose) {
                    Err(e) => eprintln!("Failed to send: {e}"),
                    _ => (),
                }

                std::thread::sleep(std::time::Duration::from_millis(500));
                if socket::process_is_running() {
                    eprintln!("Process hasn't stopped after 500ms, try `--force`");
                    std::process::exit(1);
                }
            } else {
                eprintln!("Process is already dead!");
            }

            if socket::get_socket_path().exists() {
                std::fs::remove_file(socket::get_socket_path()).expect("Unable to unlink socket!");
            }
        }
        args::Commands::Apps => match socket::send_message(AppMessage::TogglePicker(PickerKind::Apps)) {
            Err(e) => eprintln!("Failed to send: {e}"),
            _ => (),
        },
        args::Commands::Reload => match socket::send_message(AppMessage::AppReload) {
            Err(e) => eprintln!("Failed to send: {e}"),
            _ => (),
        },
    }
}
