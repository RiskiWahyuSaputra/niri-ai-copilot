//! Dialog: konfirmasi izin perekaman + pilihan mode/sumber audio, pesan error,
//! dan konfirmasi destruktif.
//!
//! Catatan: `ComboBoxText` ditandai deprecated di binding GObject sejak GTK
//! 4.10 (penggantinya `DropDown`), tetapi masih didukung penuh pada runtime
//! GTK 4.x. Migrasi ke `DropDown` dicatat sebagai pekerjaan lanjutan di
//! docs/TESTING.md.
#![allow(deprecated)]

use gtk4 as gtk;
use gtk::prelude::*;
use std::rc::Rc;

use crate::audio::AudioDevice;
use crate::config::Settings;
use crate::privacy::{external_ai_disclosure, recording_consent_text};
use crate::storage::session::SessionKind;
use crate::ui::panels;
use crate::ui::state::StartOptions;

/// Tampilkan dialog izin perekaman. `on_start` dipanggil hanya jika pengguna
/// menekan "Mulai Sesi" dengan izin terkonfirmasi.
pub fn show_consent_dialog<F>(
    parent: &gtk::Window,
    settings: &Settings,
    devices: &[AudioDevice],
    on_start: F,
) where
    F: Fn(StartOptions) + 'static,
{
    let win = gtk::Window::builder()
        .title("Mulai Sesi — Niri AI Copilot")
        .transient_for(parent)
        .modal(true)
        .resizable(false)
        .build();

    let card = gtk::Box::new(gtk::Orientation::Vertical, 10);
    card.add_css_class("card");
    card.set_margin_top(8);
    card.set_margin_bottom(8);
    card.set_margin_start(8);
    card.set_margin_end(8);
    win.set_child(Some(&card));

    let heading = gtk::Label::new(Some("Konfirmasi Sesi"));
    heading.add_css_class("header-title");
    heading.set_xalign(0.0);
    card.append(&heading);

    let sub = gtk::Label::new(Some(
        "Perekaman dan pemrosesan audio berjalan lokal di perangkat Anda. \
         Saran jawaban AI (jika diaktifkan) mengirim teks ke layanan eksternal.",
    ));
    sub.add_css_class("dim");
    sub.set_wrap(true);
    sub.set_xalign(0.0);
    card.append(&sub);

    // ── Mode sesi ────────────────────────────────────────────────────────
    let mode_label = gtk::Label::new(Some("Mode sesi"));
    mode_label.add_css_class("panel-title");
    mode_label.set_xalign(0.0);
    card.append(&mode_label);
    let mode_combo = gtk::ComboBoxText::new();
    mode_combo.append(Some("practice"), "Practice Mode (AI interviewer, default)");
    mode_combo.append(Some("live"), "Interview Langsung (deteksi pertanyaan + saran)");
    mode_combo.append(Some("transcription"), "Transkripsi saja (tanpa AI)");
    mode_combo.set_active_id(Some(if settings.practice.default_mode {
        "practice"
    } else {
        "live"
    }));
    card.append(&mode_combo);

    // ── Sumber audio ─────────────────────────────────────────────────────
    let mics: Vec<&AudioDevice> = devices.iter().filter(|d| !d.is_monitor).collect();
    let monitors: Vec<&AudioDevice> = devices.iter().filter(|d| d.is_monitor).collect();

    let mic_check = gtk::CheckButton::with_label("Rekam microphone");
    mic_check.set_active(!mics.is_empty());
    let mic_combo = gtk::ComboBoxText::new();
    for d in &mics {
        mic_combo.append(Some(&d.name), &format!("{} ({})", d.description, d.name));
    }
    if let Some(default) = mics.iter().find(|d| d.is_default) {
        mic_combo.set_active_id(Some(&default.name));
    } else if !mics.is_empty() {
        mic_combo.set_active(Some(0));
    }
    mic_combo.set_sensitive(!mics.is_empty());
    card.append(&mic_check);
    card.append(&mic_combo);

    let meeting_check =
        gtk::CheckButton::with_label("Rekam audio meeting (monitor sink, jika diizinkan)");
    meeting_check.set_active(false);
    let meeting_combo = gtk::ComboBoxText::new();
    for d in &monitors {
        meeting_combo.append(Some(&d.name), &format!("{} ({})", d.description, d.name));
    }
    if !monitors.is_empty() {
        meeting_combo.set_active(Some(0));
    }
    meeting_combo.set_sensitive(!monitors.is_empty());
    meeting_check.set_sensitive(!monitors.is_empty());
    card.append(&meeting_check);
    card.append(&meeting_combo);

    if mics.is_empty() && monitors.is_empty() {
        let warn = gtk::Label::new(Some(
            "Tidak ada sumber audio terdeteksi. Pastikan PipeWire berjalan \
             (`pactl info`) lalu buka dialog ini lagi.",
        ));
        warn.add_css_class("hint");
        warn.set_wrap(true);
        warn.set_xalign(0.0);
        card.append(&warn);
    }

    // ── Izin & disclosure ────────────────────────────────────────────────
    let consent_check = wrap_check(recording_consent_text());
    let consent_required = settings.privacy.confirm_recording;
    consent_check.set_active(!consent_required);
    consent_check.set_sensitive(consent_required);
    card.append(&consent_check);

    let ai_check = wrap_check(external_ai_disclosure());
    let disclosure_required = settings.privacy.disclose_external_ai;
    ai_check.set_active(!disclosure_required);
    ai_check.set_sensitive(disclosure_required);
    card.append(&ai_check);

    // ── Tombol ───────────────────────────────────────────────────────────
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = panels::tool_button("Batal");
    let start = panels::primary_button("Mulai Sesi");
    start.set_sensitive(!devices.is_empty() && consent_check.is_active());
    buttons.append(&cancel);
    buttons.append(&start);
    card.append(&buttons);

    let start_for_toggle = start.clone();
    let devices_available = !devices.is_empty();
    consent_check.connect_toggled(move |c| {
        start_for_toggle.set_sensitive(c.is_active() && devices_available);
    });

    {
        let win = win.clone();
        cancel.connect_clicked(move |_| win.close());
    }

    let on_start = Rc::new(on_start);
    {
        let win = win.clone();
        let mode_combo = mode_combo.clone();
        let mic_check = mic_check.clone();
        let mic_combo = mic_combo.clone();
        let meeting_check = meeting_check.clone();
        let meeting_combo = meeting_combo.clone();
        let consent_check = consent_check.clone();
        let ai_check = ai_check.clone();
        start.connect_clicked(move |_| {
            let kind = match mode_combo.active_id().as_deref() {
                Some("live") => SessionKind::Live,
                Some("transcription") => SessionKind::Transcription,
                _ => SessionKind::Practice,
            };
            let mic = if mic_check.is_active() {
                mic_combo.active_id().map(|s| s.to_string())
            } else {
                None
            };
            let meeting = if meeting_check.is_active() {
                meeting_combo.active_id().map(|s| s.to_string())
            } else {
                None
            };
            let opts = StartOptions {
                kind,
                mic_device: mic,
                meeting_device: meeting,
                consent_confirmed: consent_check.is_active(),
                external_ai_ok: ai_check.is_active(),
            };
            on_start(opts);
            win.close();
        });
    }

    win.present();
}

/// CheckBox dengan label yang membungkus (CheckButton::with_label tidak wrap).
fn wrap_check(text: &str) -> gtk::CheckButton {
    let check = gtk::CheckButton::new();
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    check.set_child(Some(&label));
    check
}

/// Dialog error sederhana.
pub fn show_error(parent: &gtk::Window, title: &str, message: &str) {
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(message)
        .buttons(["Mengerti"])
        .build();
    dialog.choose(Some(parent), gtk::gio::Cancellable::NONE, |_| {});
}

/// Dialog konfirmasi destruktif; `on_confirm` hanya dipanggil jika dikonfirmasi.
pub fn show_confirm<F>(parent: &gtk::Window, title: &str, message: &str, on_confirm: F)
where
    F: Fn() + 'static,
{
    let dialog = gtk::AlertDialog::builder()
        .modal(true)
        .message(title)
        .detail(message)
        .buttons(["Batal", "Lanjutkan"])
        .cancel_button(0)
        .default_button(0)
        .build();
    let on_confirm = Rc::new(on_confirm);
    dialog.choose(Some(parent), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(1) = res {
            on_confirm();
        }
    });
}
