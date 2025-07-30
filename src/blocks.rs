use crate::gtk;
use crate::misc;

use gtk::prelude::*;
use gtk::{gdk, glib};

pub fn create_icon_widget(icon_str: &str, size: i32) -> Option<gtk::Image> {
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

    reason_label.set_markup(&format!("<span size=\"16000\"><b>{}</b></span>", error.reason));
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
