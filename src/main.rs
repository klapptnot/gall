// All files `use crate::gtk`
pub(crate) use gtk4 as gtk;

use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gtk::gio::ApplicationFlags;
use gtk::prelude::*;
use gtk::{gdk, glib, Application, ApplicationWindow};

use crate::args::Commands;
mod args;
mod blocks;
mod misc;

const GTK_APP_ID: &str = "net.domain.AppLauncher";
const PID_FILE_PATH: &str = "/tmp/gtk4-daemon.pid";
const LOCAL_PATH: &str = ".config/gall";
const DESKTOP_PATHS: [&str; 3] = [
    "/usr/share/applications/",
    "/usr/local/share/applications/",
    "~/.local/share/applications/",
];

#[derive(Debug, Deserialize)]
struct ConfigLoad {
    css_reload: bool,
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

struct AppState {
    config: PathBuf,
    styles: PathBuf,
    name_fuzz: bool,
    selected: u32,
    #[allow(dead_code)]
    css_reload: bool,
    all_apps: Vec<AppEntry>,
    fil_apps: u32,
    spawn_err: Option<misc::CommandError>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: PathBuf::new(),
            styles: PathBuf::new(),
            name_fuzz: true,
            selected: 0,
            css_reload: false,
            all_apps: Vec::new(),
            fil_apps: 0,
            spawn_err: None,
        }
    }
}

impl AppState {
    fn new(config: PathBuf, styles: PathBuf) -> Self {
        Self {
            config,
            styles,
            ..Default::default()
        }
    }
}

struct AppWindow {
    state: Arc<Mutex<AppState>>,
    window: ApplicationWindow,
    search_input: gtk::Entry,
    toggle_btn: gtk::Button,
    listbox: gtk::ListBox,
}

impl AppWindow {
    pub fn new(app: &Application, state: Arc<Mutex<AppState>>) -> Self {
        let (window, listbox, search_input, toggle_btn) = blocks::main_launch_window(app);

        Self {
            state,
            window,
            listbox,
            search_input,
            toggle_btn,
        }
    }

    pub fn load(&self) -> &Self {
        let state = self.state.clone();
        {
            let mut locked = state.lock().unwrap();
            misc::apply_styles(&locked.styles);
            let config = misc::load_config(&locked.config);

            locked.all_apps = config.apps;
            locked.css_reload = config.css_reload;
        } // drop mutex lock

        let listbox = self.listbox.clone();
        blocks::populate_list(&listbox, &state, "");
        setup_controllers(&self);

        self
    }

    pub fn present(&self) {
        self.window.present();
    }
}

fn setup_controllers(app: &AppWindow) {
    let key_controller = gtk::EventControllerKey::new();

    // Clone references for the closure
    let search_input = app.search_input.clone();
    let listbox = app.listbox.clone();
    let window = app.window.clone();
    let toggle_btn = app.toggle_btn.clone();
    let app_state = app.state.clone();

    key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
        match keyval {
            // Ctrl+Esc: Clear input or switch to description search
            gdk::Key::Escape if state.contains(gdk::ModifierType::CONTROL_MASK) => {
                if search_input.text().is_empty() {
                    let state = app_state.clone();
                    toggle_search_mode(state, &toggle_btn);
                }
                search_input.set_text("");
                glib::Propagation::Stop
            }

            // Escape + Ctrl+Return (search_input takes Return)
            gdk::Key::Return | gdk::Key::Escape => {
                let mut locked = app_state.lock().unwrap();
                locked.selected = 0;
                search_input.set_text("");
                window.hide();
                glib::Propagation::Stop
            }

            // Up arrow: Move up in list
            gdk::Key::Up => {
                let mut locked = app_state.lock().unwrap();
                if locked.fil_apps > 0 {
                    listbox.grab_focus();

                    if locked.selected > 0 {
                        locked.selected -= 1;
                    } else {
                        locked.selected = locked.fil_apps - 1;
                    }

                    if let Some(row) = listbox.row_at_index(locked.selected as i32) {
                        listbox.select_row(Some(&row));
                        row.grab_focus();
                        search_input.grab_focus();
                    }
                }
                glib::Propagation::Stop
            }

            // Down arrow: Move down in list
            gdk::Key::Down => {
                let mut locked = app_state.lock().unwrap();
                if locked.fil_apps > 0 {
                    listbox.grab_focus();

                    let max_index = locked.fil_apps - 1;
                    if locked.selected < max_index {
                        locked.selected += 1;
                    } else {
                        locked.selected = 0;
                    }

                    if let Some(row) = listbox.row_at_index(locked.selected as i32) {
                        listbox.select_row(Some(&row));
                        row.grab_focus();
                        search_input.grab_focus();
                    }
                }
                glib::Propagation::Stop
            }

            _ => glib::Propagation::Proceed,
        }
    });

    app.window.add_controller(key_controller);

    let listbox = app.listbox.clone();
    let state = app.state.clone();

    app.search_input.connect_changed(move |entry| {
        let text = entry.text();
        blocks::populate_list(&listbox, &state, text.as_str());
    });

    let state = app.state.clone();
    let listbox = app.listbox.clone();
    let window = app.window.clone();

    app.search_input.connect_activate(move |_| {
        let row: gtk::ListBoxRow;
        {
            let locked = state.lock().unwrap();
            row = listbox
                .row_at_index(locked.selected as i32)
                .expect("Invalid row");
        }

        unsafe {
            if let Some(exec) = row.data::<String>("exec") {
                let exec = exec.as_ref().clone();
                let state_clone = state.clone();

                std::thread::spawn(move || {
                    match misc::launch_detached(&exec) {
                        Ok(()) => {
                            // Clear any previous errors on success
                            if let Ok(mut locked) = state_clone.lock() {
                                locked.spawn_err = None;
                            }
                        }
                        Err(e) => {
                            // Set the error in state
                            if let Ok(mut locked) = state_clone.lock() {
                                locked.spawn_err = Some(e);
                            }
                        }
                    }
                });
            }
        };

        window.hide();

        // check for spawn errors every 250ms
        let state_timer = state.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            if let Ok(mut locked) = state_timer.lock() {
                if let Some(error) = locked.spawn_err.take() {
                    blocks::create_error_window(error);
                    return glib::ControlFlow::Break;
                }
            }
            glib::ControlFlow::Continue
        });
    });

    let state = app.state.clone();
    app.toggle_btn.connect("clicked", true, move |val| {
        let btn = val
            .get(0)
            .and_then(|v| v.get::<gtk::Button>().ok())
            .expect("First argument is not a gtk::Button");

        let state = state.clone();
        toggle_search_mode(state, &btn);

        None
    });

    let window = app.window.clone();
    let search_input = app.search_input.clone();
    let state = app.state.clone();

    glib::source::unix_signal_add_local(libc::SIGUSR1, move || {
        search_input.set_text("");
        let mut locked = state.lock().unwrap();
        if window.is_visible() {
            window.hide();
            locked.selected = 0;
        } else {
            window.show();
            if locked.css_reload {
                misc::apply_styles(&locked.styles);
            }
        }
        glib::ControlFlow::Continue
    });

    // Hmm
    let search_input = app.search_input.clone();
    app.window.connect_close_request(move |window| {
        window.hide();
        search_input.set_text("");
        glib::Propagation::Stop
    });

    let state = app.state.clone();
    let listbox = app.listbox.clone();
    glib::source::unix_signal_add_local(libc::SIGUSR2, move || {
        {
            let mut locked = state.lock().unwrap();
            misc::apply_styles(&locked.styles);
            let config = misc::load_config(&locked.config);

            locked.all_apps = config.apps;
            locked.css_reload = config.css_reload;
        }
        blocks::populate_list(&listbox, &state, "");
        glib::ControlFlow::Continue
    });
}

fn toggle_search_mode(state: Arc<Mutex<AppState>>, toggle_btn: &gtk::Button) {
    let mut locked = state.lock().unwrap();
    locked.name_fuzz = !locked.name_fuzz;

    if locked.name_fuzz {
        toggle_btn.set_icon_name("edit-find-symbolic");
        toggle_btn.set_tooltip_text(Some("Search by name"));
    } else {
        toggle_btn.set_icon_name("dialog-information-symbolic");
        toggle_btn.set_tooltip_text(Some("Search by generic + description"));
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
        0
    });

    app.connect_activate(move |app| {
        let state = Arc::new(Mutex::new(AppState::new(
            config.to_path_buf(),
            styles.to_path_buf(),
        )));
        let app_win = AppWindow::new(app, state);
        app_win.load();

        if open_on_load {
            app_win.present();
        }
    });

    if !open_on_load {
        misc::daemonize();
    }

    app.run()
}

fn main() {
    let cli = args::Cli::parse();

    match cli.command {
        Commands::Start(args) => {
            let config = args.config.map_or(misc::get_local_path("config.toml"), |p| p);
            let styles = args.styles.map_or(misc::get_local_path("styles.css"), |p| p);

            println!("  Styles path: {}", styles.display());
            println!("  Config path: {}", config.display());
            println!(
                "  Daemonize: {}",
                if !args.open { "enabled" } else { "disabled" }
            );

            gtk_main(config, styles, args.open);
        }
        Commands::Stop(args) => {
            if args.force {
                println!("🛑 Force shutdown requested...");
                misc::send_signal(libc::SIGKILL);
            } else {
                println!("🛑 Stopping daemon...");
                misc::send_signal(libc::SIGINT);
            }
        }
        Commands::Apps => {
            println!("🔄 Toggling launcher visibility...");
            misc::send_signal(libc::SIGUSR1);
        }
        Commands::Reload => {
            println!("♻️  Reloading daemon configuration...");
            misc::send_signal(libc::SIGUSR2);
        }
    }
}
