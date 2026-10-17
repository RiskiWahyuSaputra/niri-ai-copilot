//! Builder panel kecil yang dipakai berulang di widget.

use gtk4 as gtk;
use gtk::prelude::*;

/// Panel dengan judul kecil + konten.
pub fn panel(title: &str, content: &impl IsA<gtk::Widget>) -> gtk::Box {
    let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 4);
    wrapper.add_css_class("panel");
    let label = gtk::Label::new(Some(title));
    label.add_css_class("panel-title");
    label.set_xalign(0.0);
    wrapper.append(&label);
    wrapper.append(content);
    wrapper
}

/// Label isi panel dengan wrapping.
pub fn body_label(placeholder: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(placeholder));
    label.add_css_class("panel-body");
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_selectable(true);
    label
}

/// Baris chip kata kunci (membungkus otomatis).
pub fn keyword_row() -> gtk::FlowBox {
    let row = gtk::FlowBox::new();
    row.set_selection_mode(gtk::SelectionMode::None);
    row.set_column_spacing(6);
    row.set_row_spacing(4);
    row.set_halign(gtk::Align::Start);
    row
}

/// Tombol ber-class "tool".
pub fn tool_button(label: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class("tool");
    b
}

/// Tombol ber-class "primary".
pub fn primary_button(label: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class("primary");
    b
}

/// Tombol ber-class "danger".
pub fn danger_button(label: &str) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class("danger");
    b
}
