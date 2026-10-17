//! Halaman Pengaturan (Appearance / Audio / AI / Privacy / Performance).
//!
//! Catatan: `ComboBoxText` deprecated di binding sejak GTK 4.10 (`DropDown`
//! adalah penggantinya) tetapi masih didukung runtime GTK 4.x; lihat catatan
//! migrasi di docs/TESTING.md.
#![allow(deprecated)]

use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::audio::devices;
use crate::config::{AnswerLength, AnswerStyle, Corner, Settings};
use crate::storage::Db;
use crate::ui::dialog;
use crate::ui::panels;

pub fn show<F>(parent: &gtk::Window, settings: Rc<RefCell<Settings>>, db: Rc<Db>, on_saved: F)
where
    F: Fn() + 'static,
{
    let current = settings.borrow().clone();

    let win = gtk::Window::builder()
        .title("Pengaturan — Niri AI Copilot")
        .transient_for(parent)
        .default_width(580)
        .default_height(660)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_top(10);
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);
    win.set_child(Some(&root));

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let form = gtk::Box::new(gtk::Orientation::Vertical, 14);
    scrolled.set_child(Some(&form));
    root.append(&scrolled);

    // ── Appearance ──────────────────────────────────────────────────────
    let appearance = section(&form, "Appearance");
    let theme_combo = combo(&["dark"], &["Dark (satu-satunya theme saat ini)"]);
    theme_combo.set_active_id(Some("dark"));
    add_row(&appearance, "Theme", &theme_combo);

    let opacity = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.55, 1.0, 0.01);
    opacity.set_value(current.appearance.opacity);
    opacity.set_digits(2);
    add_row(&appearance, "Opacity widget", &opacity);

    let font_size = spin(11.0, 22.0, 0.5, current.appearance.font_size);
    add_row(&appearance, "Ukuran font (px)", &font_size);

    let width = spin(260.0, 520.0, 10.0, current.appearance.compact_width as f64);
    add_row(&appearance, "Lebar widget compact (px)", &width);

    let corner = combo(
        &["top_left", "top_right", "bottom_left", "bottom_right"],
        &["Kiri atas", "Kanan atas", "Kiri bawah", "Kanan bawah"],
    );
    corner.set_active_id(Some(match current.appearance.corner {
        Corner::TopLeft => "top_left",
        Corner::TopRight => "top_right",
        Corner::BottomLeft => "bottom_left",
        Corner::BottomRight => "bottom_right",
    }));
    add_row(&appearance, "Posisi (overlay)", &corner);

    let margin = spin(0.0, 200.0, 2.0, current.appearance.margin as f64);
    add_row(&appearance, "Margin dari tepi (px)", &margin);

    // ── Audio ───────────────────────────────────────────────────────────
    let audio = section(&form, "Audio");
    let mic_combo = combo(&["<default>"], &["Default sistem"]);
    let monitor_combo = combo(&["<none>"], &["(tidak digunakan)"]);
    match devices::list_sources() {
        Ok(list) => {
            for d in list.iter().filter(|d| !d.is_monitor) {
                mic_combo.append(Some(&d.name), &d.description);
            }
            for d in list.iter().filter(|d| d.is_monitor) {
                monitor_combo.append(Some(&d.name), &d.description);
            }
        }
        Err(e) => {
            let warn = gtk::Label::new(Some(&format!("Tidak dapat membaca daftar perangkat: {e}")));
            warn.add_css_class("hint");
            warn.set_wrap(true);
            appearance_child(&audio, &warn);
        }
    }
    mic_combo.set_active_id(Some(
        current.audio.input_device.as_deref().unwrap_or("<default>"),
    ));
    monitor_combo.set_active_id(Some(
        current.audio.meeting_device.as_deref().unwrap_or("<none>"),
    ));
    add_row(&audio, "Input device", &mic_combo);
    add_row(&audio, "Meeting audio source", &monitor_combo);

    let capture_both = check("Rekam mic + audio meeting bersamaan", current.audio.capture_both);
    add_row(&audio, "", &capture_both);

    let level_meter = check("Tampilkan indikator level input", current.audio.show_level_meter);
    add_row(&audio, "", &level_meter);

    let language = combo(&["auto", "id", "en"], &["Auto-detect", "Indonesia", "Inggris"]);
    language.set_active_id(Some(&current.audio.language));
    add_row(&audio, "Bahasa STT", &language);

    let chunk = spin(2.0, 30.0, 1.0, current.audio.chunk_seconds as f64);
    add_row(&audio, "Durasi potongan STT (detik)", &chunk);
    let chunk_hint = dim("Potongan lebih kecil = transkrip lebih cepat muncul, CPU sedikit lebih sibuk.");
    appearance_child(&audio, &chunk_hint);

    let stt_binary = entry(&current.stt.binary, "whisper-cli");
    add_row(&audio, "Binary whisper.cpp", &stt_binary);
    let stt_model = entry(&current.stt.model, "/path/ke/ggml-base.bin");
    let model_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    model_row.append(&stt_model);
    let pick_model = panels::tool_button("Pilih…");
    model_row.append(&pick_model);
    add_row(&audio, "Model STT (whisper.cpp)", &model_row);
    {
        let stt_model = stt_model.clone();
        let win_for_pick = win.clone();
        pick_model.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Pilih model whisper.cpp (ggml/gguf)")
                .build();
            let entry_clone = stt_model.clone();
            dialog.open(Some(&win_for_pick), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        entry_clone.set_text(&path.display().to_string());
                    }
                }
            });
        });
    }
    let threads = spin(0.0, 32.0, 1.0, current.stt.threads as f64);
    add_row(&audio, "Thread STT (0 = otomatis)", &threads);
    let translate = check("Terjemahkan transkrip ke Inggris", current.stt.translate);
    add_row(&audio, "", &translate);

    // ── AI Provider ─────────────────────────────────────────────────────
    let ai = section(&form, "AI Provider");
    let provider = combo(&["groq"], &["Groq"]);
    provider.set_active_id(Some("groq"));
    add_row(&ai, "Provider", &provider);
    let model = entry(&current.ai.model, "llama-3.3-70b-versatile");
    add_row(&ai, "Model", &model);

    let key_status = gtk::Label::new(Some(&api_key_status(&current)));
    key_status.add_css_class("dim");
    key_status.set_wrap(true);
    key_status.set_xalign(0.0);
    appearance_child(&ai, &key_status);

    let api_key = gtk::PasswordEntry::builder()
        .placeholder_text("Tempel API key baru…")
        .show_peek_icon(true)
        .build();
    let key_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    key_row.append(&api_key);
    let save_key = panels::tool_button("Simpan key");
    let clear_key = panels::tool_button("Hapus key");
    key_row.append(&save_key);
    key_row.append(&clear_key);
    add_row(&ai, "API key", &key_row);

    let model_combo_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let response_lang = combo(
        &["auto", "id", "en"],
        &["Ikuti pertanyaan", "Indonesia", "Inggris"],
    );
    response_lang.set_active_id(Some(&current.ai.response_language));
    model_combo_row.append(&response_lang);
    let length_combo = combo(
        &["short", "medium", "detailed"],
        &["Short", "Medium", "Detailed"],
    );
    length_combo.set_active_id(Some(match current.ai.answer_length {
        AnswerLength::Short => "short",
        AnswerLength::Medium => "medium",
        AnswerLength::Detailed => "detailed",
    }));
    model_combo_row.append(&length_combo);
    add_row(&ai, "Bahasa / panjang jawaban", &model_combo_row);

    let style_combo = combo(
        &["concise", "professional", "technical", "star", "beginner_friendly", "english_interview"],
        &[
            "Concise",
            "Professional",
            "Technical",
            "STAR Method",
            "Beginner Friendly",
            "English Interview",
        ],
    );
    style_combo.set_active_id(Some(match current.ai.answer_style {
        AnswerStyle::Concise => "concise",
        AnswerStyle::Professional => "professional",
        AnswerStyle::Technical => "technical",
        AnswerStyle::Star => "star",
        AnswerStyle::BeginnerFriendly => "beginner_friendly",
        AnswerStyle::EnglishInterview => "english_interview",
    }));
    add_row(&ai, "Gaya jawaban", &style_combo);

    let timeout = spin(5.0, 180.0, 5.0, current.ai.timeout_secs as f64);
    add_row(&ai, "Timeout API (detik)", &timeout);
    let retries = spin(0.0, 5.0, 1.0, current.ai.max_retries as f64);
    add_row(&ai, "Maksimum retry (429/5xx)", &retries);

    let test_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let test_btn = panels::tool_button("Tes koneksi");
    let test_result = gtk::Label::new(None);
    test_result.add_css_class("dim");
    test_result.set_xalign(0.0);
    test_result.set_wrap(true);
    test_row.append(&test_btn);
    test_row.append(&test_result);
    add_row(&ai, "", &test_row);
    {
        let settings = Rc::clone(&settings);
        let test_result = test_result.clone();
        let api_key_entry = api_key.clone();
        test_btn.connect_clicked(move |_| {
            // Simpan dulu API key dari entry (bila diisi), lalu tes di thread.
            let key = api_key_entry.text().to_string();
            {
                let s = settings.borrow();
                if !key.trim().is_empty() {
                    if let Err(e) = s.store_api_key(&key) {
                        test_result.set_label(&format!("Gagal menyimpan key: {e}"));
                        return;
                    }
                }
            }
            test_result.set_label("Menguji koneksi…");
            let s = settings.borrow().clone();
            let result_label = test_result.clone();
            let (tx, rx) = std::sync::mpsc::channel::<String>();
            std::thread::spawn(move || {
                let msg = match crate::ai::build_provider(&s) {
                    Ok(provider) => crate::ai::test_connection(provider.as_ref())
                        .map(|reply| format!("Koneksi OK — respons: {reply}"))
                        .map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                let text = match msg {
                    Ok(m) => m,
                    Err(err) => format!("Gagal: {err}"),
                };
                let _ = tx.send(text);
            });
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                match rx.try_recv() {
                    Ok(msg) => {
                        result_label.set_label(&msg);
                        gtk::glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        gtk::glib::ControlFlow::Break
                    }
                }
            });
        });
    }
    {
        let settings = Rc::clone(&settings);
        let key_status = key_status.clone();
        let api_key_entry = api_key.clone();
        save_key.connect_clicked(move |_| {
            let key = api_key_entry.text().to_string();
            if key.trim().is_empty() {
                key_status.set_label("Entry kosong — tidak ada yang disimpan.");
                return;
            }
            match settings.borrow().store_api_key(&key) {
                Ok(()) => {
                    api_key_entry.set_text("");
                    key_status.set_label("API key disimpan di file kredensial (0600) — tidak pernah masuk database.");
                }
                Err(e) => key_status.set_label(&format!("Gagal menyimpan: {e}")),
            }
        });
    }
    {
        let settings = Rc::clone(&settings);
        let key_status = key_status.clone();
        clear_key.connect_clicked(move |_| {
            match settings.borrow().clear_api_key() {
                Ok(()) => key_status.set_label("API key tersimpan dihapus. (env var tetap dipakai bila ada)"),
                Err(e) => key_status.set_label(&format!("Gagal menghapus: {e}")),
            }
        });
    }

    // ── Privacy ─────────────────────────────────────────────────────────
    let privacy = section(&form, "Privacy");
    let confirm_rec = check(
        "Wajibkan dialog konfirmasi izin sebelum merekam",
        current.privacy.confirm_recording,
    );
    add_row(&privacy, "", &confirm_rec);
    let show_timer = check(
        "Tampilkan timer pada indikator REC",
        current.privacy.show_recording_timer,
    );
    add_row(&privacy, "", &show_timer);
    let keep_timer_note = dim("Indikator REC selalu tampil selama perekaman aktif dan tidak bisa dimatikan.");
    appearance_child(&privacy, &keep_timer_note);
    let save_audio = check(
        "Simpan file audio ke disk (default: tidak)",
        current.privacy.save_audio,
    );
    add_row(&privacy, "", &save_audio);
    let retention = spin(0.0, 365.0, 1.0, current.privacy.audio_retention_days as f64);
    add_row(&privacy, "Retensi audio (hari, 0 = tanpa batas)", &retention);
    let disclose = check(
        "Tampilkan disclosure sebelum mengirim teks ke AI eksternal",
        current.privacy.disclose_external_ai,
    );
    add_row(&privacy, "", &disclose);
    let delete_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let delete_btn = panels::danger_button("Hapus semua data sesi");
    let delete_note = dim("Menghapus seluruh riwayat sesi, transkrip, dan rekaman tersimpan. Tidak bisa dibatalkan.");
    delete_row.append(&delete_btn);
    delete_row.append(&delete_note);
    add_row(&privacy, "", &delete_row);
    {
        let db = Rc::clone(&db);
        let parent = win.clone();
        let note = delete_note.clone();
        delete_btn.connect_clicked(move |_| {
            let db = Rc::clone(&db);
            let note = note.clone();
            dialog::show_confirm(
                &parent,
                "Hapus semua data sesi?",
                "Seluruh riwayat sesi, transkrip, pertanyaan, dan saran akan dihapus permanen.",
                move || {
                    let result = crate::storage::session::delete_all_sessions(&db).map(|n| {
                        n
                    });
                    match result {
                        Ok(n) => {
                            // Hapus juga audio tersimpan bila ada.
                            let dir = crate::config::paths::recordings_dir();
                            let _ = std::fs::remove_dir_all(&dir);
                            note.set_label(&format!("{n} sesi dihapus beserta data terkait."));
                        }
                        Err(e) => note.set_label(&format!("Gagal menghapus: {e}")),
                    }
                },
            );
        });
    }

    // ── Performance ─────────────────────────────────────────────────────
    let perf = section(&form, "Performance");
    let cpu_friendly = check("CPU-friendly mode (thread & chunk konservatif)", current.performance.cpu_friendly);
    add_row(&perf, "", &cpu_friendly);
    let max_context = spin(50.0, 5000.0, 50.0, current.performance.max_transcript_context as f64);
    add_row(&perf, "Maks. segmen transkrip di memori", &max_context);
    let debounce = spin(500.0, 15000.0, 250.0, current.performance.ai_debounce_ms as f64);
    add_row(&perf, "Debounce request AI (ms)", &debounce);
    let reduce_bg = check("Kurangi pemrosesan latar belakang saat idle", current.performance.reduce_background);
    add_row(&perf, "", &reduce_bg);

    // ── Practice ────────────────────────────────────────────────────────
    let practice = section(&form, "Practice Mode");
    let practice_default = check("Jadikan Practice Mode sebagai mode default", current.practice.default_mode);
    add_row(&practice, "", &practice_default);
    let practice_cat = combo(
        &["mixed", "hr", "behavioral", "technical", "english"],
        &["Mixed", "HR", "Behavioral", "Technical", "English Interview"],
    );
    practice_cat.set_active_id(Some(&current.practice.category));
    add_row(&practice, "Kategori latihan", &practice_cat);

    // ── Footer ──────────────────────────────────────────────────────────
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save_status = gtk::Label::new(None);
    save_status.add_css_class("dim");
    save_status.set_xalign(0.0);
    save_status.set_hexpand(true);
    let btn_save = panels::primary_button("Simpan Pengaturan");
    let btn_close = panels::tool_button("Tutup");
    footer.append(&save_status);
    footer.append(&btn_save);
    footer.append(&btn_close);
    root.append(&footer);

    {
        let win = win.clone();
        btn_close.connect_clicked(move |_| win.close());
    }

    let on_saved = Rc::new(on_saved);
    {
        let settings = Rc::clone(&settings);
        let win = win.clone();
        btn_save.connect_clicked(move |_| {
            let mut s = settings.borrow_mut();

            s.appearance.theme = theme_combo
                .active_id()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "dark".into());
            s.appearance.opacity = opacity.value();
            s.appearance.font_size = font_size.value();
            s.appearance.compact_width = width.value() as i32;
            s.appearance.corner = match corner.active_id().as_deref() {
                Some("top_left") => Corner::TopLeft,
                Some("bottom_left") => Corner::BottomLeft,
                Some("bottom_right") => Corner::BottomRight,
                _ => Corner::TopRight,
            };
            s.appearance.margin = margin.value() as i32;

            s.audio.input_device = select_or_none(mic_combo.active_id(), "<default>");
            s.audio.meeting_device = select_or_none(monitor_combo.active_id(), "<none>");
            s.audio.capture_both = capture_both.is_active();
            s.audio.show_level_meter = level_meter.is_active();
            s.audio.language = language.active_id().map(|v| v.to_string()).unwrap_or_else(|| "auto".into());
            s.audio.chunk_seconds = chunk.value() as u32;
            s.stt.binary = stt_binary.text().to_string();
            s.stt.model = stt_model.text().to_string();
            s.stt.threads = threads.value() as u32;
            s.stt.translate = translate.is_active();

            s.ai.provider = provider.active_id().map(|v| v.to_string()).unwrap_or_else(|| "groq".into());
            s.ai.model = model.text().to_string();
            s.ai.response_language = response_lang.active_id().map(|v| v.to_string()).unwrap_or_else(|| "auto".into());
            s.ai.answer_length = match length_combo.active_id().as_deref() {
                Some("short") => AnswerLength::Short,
                Some("detailed") => AnswerLength::Detailed,
                _ => AnswerLength::Medium,
            };
            s.ai.answer_style = match style_combo.active_id().as_deref() {
                Some("concise") => AnswerStyle::Concise,
                Some("technical") => AnswerStyle::Technical,
                Some("star") => AnswerStyle::Star,
                Some("beginner_friendly") => AnswerStyle::BeginnerFriendly,
                Some("english_interview") => AnswerStyle::EnglishInterview,
                _ => AnswerStyle::Professional,
            };
            s.ai.timeout_secs = timeout.value() as u64;
            s.ai.max_retries = retries.value() as u32;

            s.privacy.confirm_recording = confirm_rec.is_active();
            s.privacy.show_recording_timer = show_timer.is_active();
            s.privacy.save_audio = save_audio.is_active();
            s.privacy.audio_retention_days = retention.value() as u32;
            s.privacy.disclose_external_ai = disclose.is_active();

            s.performance.cpu_friendly = cpu_friendly.is_active();
            s.performance.max_transcript_context = max_context.value() as usize;
            s.performance.ai_debounce_ms = debounce.value() as u64;
            s.performance.reduce_background = reduce_bg.is_active();

            s.practice.default_mode = practice_default.is_active();
            s.practice.category = practice_cat.active_id().map(|v| v.to_string()).unwrap_or_else(|| "mixed".into());

            s.normalize();
            match s.save() {
                Ok(()) => {
                    save_status.set_label("Pengaturan tersimpan. Pola widget diperbarui.");
                    drop(s);
                    on_saved();
                }
                Err(e) => {
                    save_status.set_label(&format!("Gagal menyimpan: {e}"));
                    drop(s);
                }
            }
            let _ = &win;
        });
    }

    win.present();
}

fn api_key_status(settings: &Settings) -> String {
    if std::env::var(&settings.ai.api_key_env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
    {
        return format!("API key tersedia dari environment variable {}.", settings.ai.api_key_env);
    }
    if settings.credential_from_file().is_some() {
        return "API key tersedia dari file kredensial lokal.".into();
    }
    format!(
        "API key belum diatur. Set {} atau simpan lewat form di bawah \
         (disimpan dengan permission 0600, bukan di database).",
        settings.ai.api_key_env
    )
}

fn select_or_none(value: Option<gtk::glib::GString>, none_value: &str) -> Option<String> {
    match value {
        Some(v) if v.as_str() != none_value => Some(v.to_string()),
        _ => None,
    }
}

fn section(form: &gtk::Box, title: &str) -> gtk::Box {
    let header = gtk::Label::new(Some(title));
    header.add_css_class("panel-title");
    header.set_xalign(0.0);
    header.set_margin_top(6);
    form.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    body.add_css_class("panel");
    form.append(&body);
    body
}

fn add_row(section: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let lbl = gtk::Label::new(Some(label));
    lbl.set_width_chars(24);
    lbl.set_xalign(0.0);
    lbl.set_valign(gtk::Align::Center);
    lbl.set_wrap(true);
    row.append(&lbl);
    widget.set_hexpand(true);
    row.append(widget);
    section.append(&row);
}

fn appearance_child(section: &gtk::Box, widget: &impl IsA<gtk::Widget>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let spacer = gtk::Label::new(None);
    spacer.set_width_chars(24);
    row.append(&spacer);
    widget.set_hexpand(true);
    row.append(widget);
    section.append(&row);
}

fn combo(ids: &[&str], labels: &[&str]) -> gtk::ComboBoxText {
    let c = gtk::ComboBoxText::new();
    for (id, label) in ids.iter().zip(labels.iter()) {
        c.append(Some(id), label);
    }
    c
}

fn check(label: &str, active: bool) -> gtk::CheckButton {
    let c = gtk::CheckButton::new();
    let l = gtk::Label::new(Some(label));
    l.set_wrap(true);
    l.set_xalign(0.0);
    c.set_child(Some(&l));
    c.set_active(active);
    c
}

fn entry(value: &str, placeholder: &str) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_text(value);
    e.set_placeholder_text(Some(placeholder));
    e
}

fn spin(min: f64, max: f64, step: f64, value: f64) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, step);
    s.set_value(value);
    s
}

fn dim(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("dim");
    l.set_wrap(true);
    l.set_xalign(0.0);
    l
}
