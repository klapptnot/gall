use crate::gtk;

use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::blocks;
use crate::misc;
use crate::{AppState, GallApp, Picker};
use crate::{Arc, Mutex};

pub(crate) fn set_picker_control(app: &GallApp, picker: &Arc<Picker>) {
    let mwindow = &app.window;
    let state = &app.state;

    {
        let key_controller = gtk::EventControllerKey::new();

        // Clone references for the closure
        let search_input = picker.search_input.clone();
        let listbox = picker.listbox.clone();
        let window = mwindow.clone();
        let toggle_btn = picker.toggle_btn.clone();
        let app_state = state.clone();

        key_controller.connect_key_pressed(move |_controller, keyval, _keycode, state| {
            match keyval {
                // Ctrl+Esc: Clear input or switch to description search
                gdk::Key::Escape if state.contains(gdk::ModifierType::CONTROL_MASK) => {
                    if search_input.text().is_empty() {
                        {
                            let state = app_state.clone();
                            apps_toggle_search_mode(state, &toggle_btn);
                        }
                    }
                    search_input.set_text("");
                    glib::Propagation::Stop
                }

                // Escape + Ctrl+Return (search_input takes Return)
                gdk::Key::Return | gdk::Key::Escape => {
                    {
                        let mut locked = app_state.lock().unwrap();
                        locked.selected = 0;
                        locked.visible = false;
                    }
                    search_input.set_text("");
                    listbox.select_row(listbox.row_at_index(0).as_ref());
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

        mwindow.add_controller(key_controller);
    }
}

pub(crate) fn app_picker_control(app: &GallApp, picker: &Arc<Picker>) {
    let mwindow = &app.window;
    let state = &app.state;
    {
        let listbox = picker.listbox.clone();
        let state = state.clone();

        picker.search_input.connect_changed(move |entry| {
            let text = entry.text();
            blocks::apps_populate_list(&listbox, &state, text.as_str());
        });
    }

    {
        let state = state.clone();
        let listbox = picker.listbox.clone();
        let window = mwindow.clone();
        let gapp = app.app.clone();

        picker.search_input.connect_activate(move |_| {
            let row: gtk::ListBoxRow;
            {
                let mut locked = state.lock().unwrap();
                row = listbox
                    .row_at_index(locked.selected as i32)
                    .expect("Invalid row");
                locked.visible = false;
            }

            window.hide();

            let exec = unsafe { row.data::<String>("exec").map(|v| v.as_ref().clone()) };
            if let Some(exec) = exec {
                launch_command_helper(exec, &gapp);
            }
        });
    }

    {
        let listbox = picker.listbox.clone();
        let window = mwindow.clone();
        let gapp = app.app.clone();

        listbox.connect_row_activated(move |_, row| {
            window.hide();

            let exec = unsafe { row.data::<String>("exec").map(|v| v.as_ref().clone()) };
            if let Some(exec) = exec {
                launch_command_helper(exec, &gapp);
            }
        });
    }

    {
        let state = state.clone();
        picker.toggle_btn.connect("clicked", true, move |val| {
            let btn = val
                .get(0)
                .and_then(|v| v.get::<gtk::Button>().ok())
                .expect("First argument is not a gtk::Button");

            let state = state.clone();
            apps_toggle_search_mode(state, &btn);

            None
        });
    }

    {
        let state = state.clone();
        let listbox = picker.listbox.clone();
        glib::source::unix_signal_add_local(crate::SIGNAL_RELOAD, move || {
            {
                let mut locked = state.lock().unwrap();
                misc::apply_styles(&locked.styles);
                let config = misc::load_config(&locked.config);

                locked.all_apps = config.apps;
                locked.css_reload = config.css_reload;
            }
            blocks::apps_populate_list(&listbox, &state, "");
            glib::ControlFlow::Continue
        });
    }
}

fn apps_toggle_search_mode(state: Arc<Mutex<AppState>>, toggle_btn: &gtk::Button) {
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
                blocks::create_error_window(&app, error);
                return glib::ControlFlow::Break;
            }
        }

        glib::ControlFlow::Continue
    });
}
