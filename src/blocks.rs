use std::sync::Arc;
use std::sync::Mutex;

use crate::gtk;
use crate::misc;
use crate::{AppState, Picker};

use gtk::prelude::*;
use gtk::{gdk, glib};

pub(crate) fn generic_picker_box() -> Picker {
    let mainbox = gtk::Box::builder()
        .name("main-box")
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    let box_input = gtk::Box::builder()
        .name("search-box")
        .spacing(5)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(5)
        .build();

    let search_input = gtk::Entry::builder()
        .name("search-input")
        .placeholder_text("Type to search apps...")
        .build();
    search_input.set_hexpand(true);

    let toggle_btn = gtk::Button::builder().name("toggle-button").build();

    let scroll_apps = gtk::ScrolledWindow::builder()
        .name("apps-scroll")
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .overflow(gtk::Overflow::Hidden)
        .margin_start(10)
        .margin_end(10)
        .margin_top(10)
        .margin_bottom(10)
        .build();
    scroll_apps.set_vexpand(true);

    let listbox = gtk::ListBox::builder()
        .name("apps-list")
        .selection_mode(gtk::SelectionMode::Single)
        .vexpand_set(true)
        .build();

    box_input.append(&search_input);
    box_input.append(&toggle_btn);

    scroll_apps.set_child(Some(&listbox));

    mainbox.append(&box_input);
    mainbox.append(&scroll_apps);

    Picker {
        mainbox,
        search_input,
        toggle_btn,
        listbox,
    }
}

pub(crate) fn apps_populate_list(listbox: &gtk::ListBox, state: &Arc<Mutex<AppState>>, pattern: &str) {
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
                misc::fuzzy(&e.genc.clone().unwrap_or("".to_owned()), pattern)
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

pub(crate) fn create_app_row(app: &crate::AppEntry) -> gtk::ListBoxRow {
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
        if let Some(icon) = create_icon_widget(icon_str, 48) {
            hbox.append(&icon);
        }
    }

    let text_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .build();

    let name_markup = match &app.genc {
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

fn create_icon_widget(icon_str: &str, size: i32) -> Option<gtk::Image> {
    let path = misc::expand_tilde(icon_str).expect("could not expand path");
    if path.is_file() {
        if let Ok(texture) = gdk::Texture::from_file(&gtk::gio::File::for_path(&path)) {
            let image = gtk::Image::from_paintable(Some(&texture));
            image.set_pixel_size(size);
            return Some(image);
        }
    } // bad

    let display = gdk::Display::default()?;
    let icon_theme = gtk::IconTheme::for_display(&display);

    let icon_paintable = icon_theme.lookup_icon(
        icon_str,
        &["application-x-executable"], // bad?
        size,
        1,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::PRELOAD,
    );

    let image = gtk::Image::builder()
        .overflow(gtk::Overflow::Hidden)
        .name("app-row-image")
        .pixel_size(size)
        .paintable(&icon_paintable)
        .build();

    Some(image)
}

pub(crate) fn create_error_window(app: &gtk::Application, error: misc::CommandError) {
    let error_window = gtk::Window::builder()
        .title("Gall - Command Error")
        .default_width(600)
        .default_height(400)
        .resizable(true)
        .build();

    // Attach the window to the application
    error_window.set_application(Some(app));

    let vbox = gtk::Box::builder()
        .name("error-box")
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_start(15)
        .margin_end(15)
        .margin_top(15)
        .margin_bottom(15)
        .build();

    let reason_label = gtk::Label::builder()
        .wrap(true)
        .name("error-reason")
        .halign(gtk::Align::Start)
        .build();

    reason_label.set_markup(&format!(
        "<span size=\"16000\"><b>{}</b></span>",
        error.reason
    ));
    vbox.append(&reason_label);

    let button_box = gtk::Box::builder()
        .name("error-btn-box")
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .halign(gtk::Align::End)
        .margin_top(10)
        .build();

    let copy_btn_class = ["error-copy-btn"];

    // STDOUT
    if let Some(stdout_text) = error.stdout {
        let stdout_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .name("error-label-stdout")
            .build();
        stdout_label.set_markup("<b>STDOUT:</b>");
        vbox.append(&stdout_label);

        let stdout_scrolled = gtk::ScrolledWindow::builder()
            .name("error-stdout-scroll")
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .height_request(120)
            .build();

        let stdout_textview = gtk::TextView::new();
        stdout_textview.set_editable(false);
        stdout_textview.set_monospace(true);

        stdout_textview.buffer().set_text(&stdout_text);
        stdout_scrolled.set_child(Some(&stdout_textview));
        vbox.append(&stdout_scrolled);

        // Copy button
        let copy_stdout_btn = gtk::Button::with_label("Copy STDOUT");
        copy_stdout_btn.set_css_classes(&copy_btn_class);
        let stdout_buffer = stdout_textview.buffer();
        copy_stdout_btn.connect_clicked(move |_| {
            let clipboard = gtk::gdk::Display::default().and_then(|display| Some(display.clipboard()));
            if let Some(clipboard) = clipboard {
                let text = stdout_buffer.text(&stdout_buffer.start_iter(), &stdout_buffer.end_iter(), false);
                clipboard.set_text(&text);
            }
        });
        button_box.append(&copy_stdout_btn);
    }

    // STDERR
    if let Some(stderr_text) = error.stderr {
        let stderr_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .name("error-label-stderr")
            .build();
        stderr_label.set_markup("<b>STDERR:</b>");
        vbox.append(&stderr_label);

        let stderr_scrolled = gtk::ScrolledWindow::builder()
            .name("error-stderr-scroll")
            .hscrollbar_policy(gtk::PolicyType::Automatic)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .height_request(120)
            .build();

        let stderr_textview = gtk::TextView::new();
        stderr_textview.set_editable(false);
        stderr_textview.set_monospace(true);

        stderr_textview.buffer().set_text(&stderr_text);
        stderr_scrolled.set_child(Some(&stderr_textview));
        vbox.append(&stderr_scrolled);

        // Copy button
        let copy_stderr_btn = gtk::Button::with_label("Copy STDERR");
        copy_stderr_btn.set_css_classes(&copy_btn_class);
        let stderr_buffer = stderr_textview.buffer();
        copy_stderr_btn.connect_clicked(move |_| {
            let clipboard = gtk::gdk::Display::default().and_then(|display| Some(display.clipboard()));

            if let Some(clipboard) = clipboard {
                let text = stderr_buffer.text(&stderr_buffer.start_iter(), &stderr_buffer.end_iter(), false);
                clipboard.set_text(&text);
            }
        });
        button_box.append(&copy_stderr_btn);
    }

    // Close button
    let close_btn = gtk::Button::builder()
        .name("error-close-btn")
        .label("Close")
        .build();
    button_box.append(&close_btn);

    vbox.append(&button_box);
    error_window.set_child(Some(&vbox));
    error_window.present();

    let window = error_window.clone();
    close_btn.connect_clicked(move |_| {
        window.close();
    });
    close_btn.grab_focus();

    let key_controller = gtk::EventControllerKey::new();
    let window = error_window.clone();

    key_controller.connect_key_pressed(move |_controller, keyval, _keycode, _state| {
        // Escape + Ctrl+Return (search_input takes Return)
        if keyval == gdk::Key::Return || keyval == gdk::Key::Escape {
            window.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    error_window.add_controller(key_controller);
}
