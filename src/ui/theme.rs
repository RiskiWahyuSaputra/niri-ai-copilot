//! Theme & CSS.
//!
//! Dark theme premium minimalis: sudut membulat, aksen indigo/ungu, kontras
//! teks tinggi. Opacity dan ukuran font dari settings diterapkan lewat
//! provider CSS dinamis.

use gtk4 as gtk;
use std::cell::RefCell;

use crate::config::Settings;

const BASE_CSS: &str = include_str!("../../assets/style.css");

thread_local! {
    static DYNAMIC: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Muat CSS dasar (sekali) + CSS dinamis sesuai settings.
pub fn apply(settings: &Settings) {
    let display = gtk::gdk::Display::default().expect("display GTK tidak tersedia");
    let base = gtk::CssProvider::new();
    base.load_from_data(BASE_CSS);
    gtk::style_context_add_provider_for_display(
        &display,
        &base,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    apply_dynamic(settings);
}

/// Perbarui CSS dinamis (opacity & font size) tanpa reload dasar.
pub fn apply_dynamic(settings: &Settings) {
    let display = gtk::gdk::Display::default().expect("display GTK tidak tersedia");
    let css = format!(
        ".card {{ opacity: {opacity}; }}\n.card {{ font-size: {font}px; }}\n",
        opacity = settings.appearance.opacity.clamp(0.55, 1.0),
        font = settings.appearance.font_size.clamp(11.0, 22.0),
    );
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&css);
    DYNAMIC.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(old) = slot.take() {
            gtk::style_context_remove_provider_for_display(&display, &old);
        }
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        *slot = Some(provider);
    });
}
