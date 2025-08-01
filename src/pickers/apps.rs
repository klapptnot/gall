use crate::{
    config::{AppEntry, ConfigLoad},
    gtk::{self, gdk, glib},
    misc,
    pickers::{self, Picker},
    GallApp,
};
use gtk::prelude::*;
use std::sync::{Arc, Mutex};

pub struct AppPickerState {
    name_fuzz: bool,
    selected: u32,
    fil_apps: u32,
    all_apps: Vec<AppEntry>,
    callback: Arc<Option<Box<dyn Fn()>>>,
}

pub struct AppPicker {
    parent: Arc<GallApp>,
    state: Arc<Mutex<AppPickerState>>,
    mainbox: gtk::Box,
    search_input: gtk::Entry,
    toggle_btn: gtk::Button,
    listbox: gtk::ListBox,
}

impl AppPickerState {
    fn new() -> Self {
        Self {
            name_fuzz: true,
            selected: 0,
            fil_apps: 0,
            all_apps: Vec::new(),
            callback: Arc::new(None),
        }
    }
}

impl AppPicker {
    pub fn new(parent: Arc<GallApp>) -> Self {
        let (mainbox, search_input, toggle_btn, listbox) = pickers::create_picker_components();
        let state = Arc::new(Mutex::new(AppPickerState::new()));

        let _ = toggle_btn.set_icon_name("edit-find-symbolic");
        let _ = toggle_btn.set_tooltip_text(Some("Search by name"));

        Self {
            parent,
            state,
            mainbox,
            search_input,
            toggle_btn,
            listbox,
        }
    }
}

impl Picker for AppPicker {
    fn load(&self, config: &ConfigLoad) -> bool {
        self.reload(config);
        app_picker_control(&self);
        populate_app_list(&self.listbox, &self.state, "");

        true
    }

    fn show(&self, current: super::PickerKind) -> bool {
        let had_to_load = current != self.kind();

        if had_to_load {
            self.parent.window.set_child(Some(&self.mainbox));
        }
        self.listbox.select_row(self.listbox.row_at_index(0).as_ref());

        let name_fuzz = {
            let mut locked = self.state.lock().unwrap();
            locked.selected = 0;
            locked.name_fuzz
        };

        if !name_fuzz {
            toggle_fuzzy_search_mode(&self.state, &self.toggle_btn);
        }

        self.search_input.grab_focus();
        self.search_input.set_text(""); // calls populate_app_list if needed

        had_to_load
    }

    fn kind(&self) -> super::PickerKind {
        super::PickerKind::Apps
    }

    fn if_done(&self, callback: Box<dyn Fn()>) -> () {
        let mut state = self.state.lock().unwrap();
        state.callback = Arc::new(Some(callback));
    }

    fn reload(&self, config: &ConfigLoad) {
        let mut state = self.state.lock().unwrap();

        state.fil_apps = config.apps.len() as u32;
        // TODO: cbwqbfq[bf[oqbq[bfqboe[bfoe]]]] use a slice
        state.all_apps = config.apps.clone();
        state.name_fuzz = true;
    }
}

fn populate_app_list(listbox: &gtk::ListBox, state: &Arc<Mutex<AppPickerState>>, pattern: &str) {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }

    let mut locked = state.lock().unwrap();

    locked
        .all_apps
        .iter()
        .filter(|e| {
            if locked.name_fuzz {
                misc::fuzzy(&e.name, pattern)
            } else {
                misc::fuzzy(&e.gend.clone().unwrap_or("".to_owned()), pattern)
                    || misc::fuzzy(&e.desc.clone().unwrap_or("".to_owned()), pattern)
            }
        })
        .for_each(|e| {
            let app_row = create_app_row(e);
            listbox.append(&app_row);
        });

    locked.fil_apps = listbox.observe_children().n_items();

    if locked.selected > locked.fil_apps {
        locked.selected = 0
    }

    listbox.select_row(listbox.row_at_index(locked.selected as i32).as_ref());

    listbox.show();
}

fn create_app_row(app: &AppEntry) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name("app-row");

    unsafe { row.set_data("exec", app.exec.clone()) };

    let hbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .margin_start(10)
        .margin_end(10)
        .margin_top(5)
        .margin_bottom(5)
        .build();

    if let Some(icon_str) = &app.icon {
        if let Some(icon) = crate::blocks::create_icon_widget(icon_str, 48) {
            hbox.append(&icon);
        }
    }

    let text_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .build();

    let name_markup = match &app.gend {
        Some(g) if g != &app.name => format!(
            "<b>{}</b> - <i>{}</i>",
            glib::markup_escape_text(&app.name),
            glib::markup_escape_text(g),
        ),
        _ => format!("<b>{}</b>", glib::markup_escape_text(&app.name)),
    };

    let name_label = gtk::Label::new(None);
    name_label.set_markup(&name_markup);
    name_label.set_halign(gtk::Align::Start);
    text_box.append(&name_label);

    if let Some(desc) = &app.desc {
        let short_desc = if desc.len() > 60 {
            format!("{}...", &desc[..60])
        } else {
            desc.clone()
        };
        let desc_label = gtk::Label::new(Some(&short_desc));
        desc_label.set_halign(gtk::Align::Start);
        desc_label.style_context().add_class("dim-label");
        text_box.append(&desc_label);
    }

    hbox.append(&text_box);
    row.set_child(Some(&hbox));

    row
}

fn toggle_fuzzy_search_mode(state: &Arc<Mutex<AppPickerState>>, toggle_btn: &gtk::Button) {
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

fn launch_command_helper(exec: String, app: &gtk::Application) -> () {
    let cmde = std::thread::spawn(move || misc::launch_detached(&exec));
    let app = app.clone();

    // just to ensure it's used once
    let mut cmde = Some(cmde);

    glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
        if let Some(ref handle) = cmde {
            if !handle.is_finished() {
                return glib::ControlFlow::Continue;
            }
        } else {
            return glib::ControlFlow::Break;
        }

        let handle = cmde.take();

        if let Some(handle) = handle {
            let jhres = handle.join();

            if jhres.is_err() {
                return glib::ControlFlow::Break;
            }

            let jhres = jhres.unwrap();

            if let Err(error) = jhres {
                crate::blocks::create_error_window(&app, error);
                return glib::ControlFlow::Break;
            }
        }

        glib::ControlFlow::Continue
    });
}

fn app_picker_control(picker: &AppPicker) {
    {
        let listbox = picker.listbox.clone();
        let state = picker.state.clone();

        picker.search_input.connect_changed(move |entry| {
            let text = entry.text();
            populate_app_list(&listbox, &state, text.as_str());
        });
    }

    {
        let key_controller = gtk::EventControllerKey::new();

        // Clone references for the closure
        let search_input = picker.search_input.clone();
        let listbox = picker.listbox.clone();
        let toggle_btn = picker.toggle_btn.clone();
        let picker_state = picker.state.clone();

        key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
            match keyval {
                // Ctrl+Esc: Clear input or switch to description search
                gdk::Key::Escape if state.contains(gdk::ModifierType::CONTROL_MASK) => {
                    if search_input.text().is_empty() {
                        {
                            let pstate = picker_state.clone();
                            toggle_fuzzy_search_mode(&pstate, &toggle_btn);
                        }
                    }
                    search_input.set_text("");
                    glib::Propagation::Stop
                }

                // Escape + Ctrl+Return (search_input takes Return)
                gdk::Key::Return | gdk::Key::Escape => {
                    search_input.set_text("");
                    listbox.select_row(listbox.row_at_index(0).as_ref());
                    {
                        let mut locked = picker_state.lock().unwrap();
                        locked.selected = 0;
                        if let Some(ref callback) = *locked.callback {
                            callback();
                        }
                    }
                    glib::Propagation::Stop
                }

                // Up arrow: Move up in list
                gdk::Key::Up => {
                    let mut locked = picker_state.lock().unwrap();
                    if locked.fil_apps > 0 {
                        if locked.selected > 0 {
                            locked.selected -= 1;
                        } else {
                            locked.selected = locked.fil_apps - 1;
                        }

                        let row = listbox.row_at_index(locked.selected as i32);
                        listbox.select_row(row.as_ref());
                        row.map(|r| {
                            r.grab_focus();
                            search_input.grab_focus();
                        });
                    }
                    glib::Propagation::Stop
                }

                // Down arrow: Move down in list
                gdk::Key::Down => {
                    let mut locked = picker_state.lock().unwrap();
                    if locked.fil_apps > 0 {
                        let max_index = locked.fil_apps - 1;
                        if locked.selected < max_index {
                            locked.selected += 1;
                        } else {
                            locked.selected = 0;
                        }

                        let row = listbox.row_at_index(locked.selected as i32);
                        listbox.select_row(row.as_ref());
                        row.map(|r| {
                            r.grab_focus();
                            search_input.grab_focus();
                        });
                    }
                    glib::Propagation::Stop
                }

                _ => glib::Propagation::Proceed,
            }
        });

        let _ = &picker.parent.window.add_controller(key_controller);
    }

    {
        let state = picker.state.clone();
        let listbox = picker.listbox.clone();
        let gapp = picker.parent.app.clone();

        picker.search_input.connect_activate(move |_| {
            let row: gtk::ListBoxRow;
            {
                let locked = state.lock().unwrap();
                row = listbox
                    .row_at_index(locked.selected as i32)
                    .expect("Invalid row");

                if let Some(ref callback) = *locked.callback {
                    callback();
                }
            }

            let exec = unsafe { row.data::<String>("exec").map(|v| v.as_ref().clone()) };
            if let Some(exec) = exec {
                launch_command_helper(exec, &gapp);
            }
        });
    }

    {
        let listbox = picker.listbox.clone();
        let gapp = picker.parent.app.clone();
        let state = picker.state.clone();

        listbox.connect_row_activated(move |_, row| {
            {
                let locked = state.lock().unwrap();
                if let Some(ref callback) = *locked.callback {
                    callback();
                }
            }

            let exec = unsafe { row.data::<String>("exec").map(|v| v.as_ref().clone()) };
            if let Some(exec) = exec {
                launch_command_helper(exec, &gapp);
            }
        });
    }

    {
        let state = picker.state.clone();
        picker.toggle_btn.connect_clicked(move |btn| {
            let state = state.clone();
            toggle_fuzzy_search_mode(&state, btn);
        });
    }
}
