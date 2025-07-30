pub(crate) mod apps;

use crate::{
    gtk, Arc, GallApp,
    config::ConfigLoad
};
use gtk::prelude::{BoxExt, WidgetExt};

#[derive(PartialEq, Clone, Copy, Debug)]
pub(crate) enum PickerKind {
    Apps,
    None,
}

pub trait Picker {
    fn load(&self, config: &ConfigLoad) -> bool;
    fn show(&self, current: PickerKind) -> bool;
    fn kind(&self) -> PickerKind;
    fn reload(&self, config: &ConfigLoad);
    fn if_done(&self, callback: Box<dyn Fn()>);
}

impl PickerKind {
    pub fn variants() -> [Self; 1] {
        [PickerKind::Apps]
    }

    pub fn from_kind(&self, app: Arc<GallApp>) -> Arc<dyn Picker> {
        let picker = match self {
            PickerKind::Apps => apps::AppPicker::new(app),
            PickerKind::None => unreachable!(), // used only in one place
        };

        Arc::new(picker)
    }
}

pub(crate) fn create_picker_components() -> (gtk::Box, gtk::Entry, gtk::Button, gtk::ListBox) {
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

    // Assemble the UI
    box_input.append(&search_input);
    box_input.append(&toggle_btn);
    scroll_apps.set_child(Some(&listbox));
    mainbox.append(&box_input);
    mainbox.append(&scroll_apps);

    (mainbox, search_input, toggle_btn, listbox)
}
