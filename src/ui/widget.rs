//! Widget utama: mode compact & expanded, indikator status, panel-panel.

use gtk4 as gtk;
use gtk::prelude::*;

use crate::ai::prompts::Suggestion;
use crate::config::Settings;
use crate::storage::session::TranscriptSegment;
use crate::ui::panels;
use crate::ui::state::WidgetStatus;

/// Kumpulan widget yang perlu diakses app.rs untuk update.
pub struct Widgets {
    pub window: gtk::ApplicationWindow,
    pub compact_width: i32,
    pub status_pill: gtk::Label,
    pub rec_dot: gtk::Label,
    pub rec_label: gtk::Label,
    pub timer_label: gtk::Label,
    pub spinner: gtk::Spinner,
    pub hint_label: gtk::Label,
    pub level_bar: gtk::ProgressBar,
    pub transcript_box: gtk::Box,
    pub question_label: gtk::Label,
    pub question_meta: gtk::Label,
    pub answer_label: gtk::Label,
    pub keywords_row: gtk::FlowBox,
    pub notes_buffer: gtk::TextBuffer,
    pub revealer: gtk::Revealer,
    pub btn_start: gtk::Button,
    pub btn_pause: gtk::Button,
    pub btn_stop: gtk::Button,
    pub btn_expand: gtk::Button,
    pub btn_close: gtk::Button,
    pub btn_next_question: gtk::Button,
    pub btn_alt_answer: gtk::Button,
    pub btn_explain: gtk::Button,
    pub btn_settings: gtk::Button,
    pub btn_history: gtk::Button,
    pub btn_profile: gtk::Button,
}

/// Bangun jendela widget. Callback tombol *tidak* disambungkan di sini —
/// app.rs yang menyambungkannya agar logika tetap terpusat.
pub fn build(app: &gtk::Application, settings: &Settings) -> Widgets {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Niri AI Copilot")
        .decorated(false)
        .resizable(false)
        .build();

    let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
    card.add_css_class("card");
    card.set_width_request(settings.appearance.compact_width);
    card.set_margin_top(settings.appearance.margin);
    card.set_margin_bottom(settings.appearance.margin);
    card.set_margin_start(settings.appearance.margin);
    card.set_margin_end(settings.appearance.margin);
    window.set_child(Some(&card));

    // ── Header ──────────────────────────────────────────────────────────
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let logo = gtk::Label::new(Some("N"));
    logo.set_valign(gtk::Align::Center);
    logo.add_css_class("logo-dot");
    header.append(&logo);

    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_hexpand(true);
    let title = gtk::Label::new(Some("Niri AI Copilot"));
    title.add_css_class("header-title");
    title.set_xalign(0.0);
    let subtitle = gtk::Label::new(Some("Interview assistant · lokal & privat"));
    subtitle.add_css_class("header-sub");
    subtitle.set_xalign(0.0);
    titles.append(&title);
    titles.append(&subtitle);
    header.append(&titles);

    let btn_close = panels::tool_button("✕");
    btn_close.set_tooltip_text(Some("Sembunyikan widget (shortcut: toggle)"));
    header.append(&btn_close);
    card.append(&header);

    // ── Baris status ────────────────────────────────────────────────────
    let status_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let status_pill = gtk::Label::new(Some(WidgetStatus::Idle.label()));
    status_pill.add_css_class("pill");
    status_pill.add_css_class(WidgetStatus::Idle.css_class());
    status_row.append(&status_pill);

    let rec_dot = gtk::Label::new(Some("●"));
    rec_dot.add_css_class("rec-dot");
    rec_dot.set_visible(false);
    status_row.append(&rec_dot);

    let rec_label = gtk::Label::new(Some("REC"));
    rec_label.add_css_class("rec-label");
    rec_label.set_visible(false);
    status_row.append(&rec_label);

    let timer_label = gtk::Label::new(None);
    timer_label.add_css_class("timer");
    status_row.append(&timer_label);

    let spinner = gtk::Spinner::new();
    status_row.append(&spinner);

    let level_bar = gtk::ProgressBar::new();
    level_bar.add_css_class("level-bar");
    level_bar.set_hexpand(true);
    level_bar.set_valign(gtk::Align::Center);
    level_bar.set_fraction(0.0);
    status_row.append(&level_bar);
    card.append(&status_row);

    let hint_label = gtk::Label::new(None);
    hint_label.add_css_class("hint");
    hint_label.set_wrap(true);
    hint_label.set_xalign(0.0);
    hint_label.set_visible(false);
    card.append(&hint_label);

    // ── Tombol kontrol utama ────────────────────────────────────────────
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let btn_start = panels::primary_button("Mulai");
    btn_start.set_tooltip_text(Some("Mulai sesi (konfirmasi izin akan muncul)"));
    let btn_pause = panels::tool_button("Pause");
    btn_pause.set_tooltip_text(Some("Jeda / lanjutkan perekaman"));
    btn_pause.set_sensitive(false);
    let btn_stop = panels::danger_button("Stop");
    btn_stop.set_tooltip_text(Some("Hentikan sesi dan simpan transkrip"));
    btn_stop.set_sensitive(false);
    let btn_expand = panels::tool_button("Detail ▾");
    btn_expand.set_tooltip_text(Some("Perluas / kecilkan widget"));
    controls.append(&btn_start);
    controls.append(&btn_pause);
    controls.append(&btn_stop);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    controls.append(&spacer);
    controls.append(&btn_expand);
    card.append(&controls);

    // ── Area expanded ───────────────────────────────────────────────────
    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .transition_duration(220)
        .reveal_child(false)
        .build();

    let panels_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    panels_box.set_margin_top(4);

    // Transcript
    let transcript_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let transcript_scroll = gtk::ScrolledWindow::builder()
        .min_content_height(150)
        .max_content_height(240)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(true)
        .build();
    transcript_scroll.set_child(Some(&transcript_box));
    panels_box.append(&panels::panel("Live Transcript", &transcript_scroll));

    // Question
    let question_label = panels::body_label("Belum ada pertanyaan terdeteksi.");
    let question_meta = gtk::Label::new(None);
    question_meta.add_css_class("transcript-meta");
    question_meta.set_xalign(0.0);
    let q_box = gtk::Box::new(gtk::Orientation::Vertical, 2);
    q_box.append(&question_label);
    q_box.append(&question_meta);
    panels_box.append(&panels::panel("Detected Question", &q_box));

    // Suggested answer
    let answer_label = panels::body_label(
        "Saran jawaban akan muncul di sini setelah pertanyaan terdeteksi \
         (butuh API key di Settings → AI).",
    );
    answer_label.add_css_class("answer-body");
    panels_box.append(&panels::panel("Suggested Answer", &answer_label));

    // Keywords
    let keywords_row = panels::keyword_row();
    panels_box.append(&panels::panel("Important Keywords", &keywords_row));

    // Notes
    let notes_view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .height_request(70)
        .build();
    notes_view.add_css_class("notes");
    let notes_buffer = notes_view.buffer();
    panels_box.append(&panels::panel("Interview Notes", &notes_view));

    // Session controls
    let session_controls = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let btn_next_question = panels::tool_button("Pertanyaan ▸");
    btn_next_question.set_tooltip_text(Some("Minta pertanyaan berikutnya (Practice Mode)"));
    let btn_alt_answer = panels::tool_button("Alternatif ✓");
    btn_alt_answer.set_tooltip_text(Some("Buat alternatif jawaban terakhir"));
    let btn_explain = panels::tool_button("Jelaskan");
    btn_explain.set_tooltip_text(Some("Jelaskan istilah teknis dari saran terakhir"));
    session_controls.append(&btn_next_question);
    session_controls.append(&btn_alt_answer);
    session_controls.append(&btn_explain);
    panels_box.append(&panels::panel("Session Controls", &session_controls));

    // Navigasi halaman
    let nav = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let btn_settings = panels::tool_button("Pengaturan");
    let btn_history = panels::tool_button("Riwayat");
    let btn_profile = panels::tool_button("Profil");
    nav.append(&btn_settings);
    nav.append(&btn_history);
    nav.append(&btn_profile);
    panels_box.append(&nav);

    revealer.set_child(Some(&panels_box));
    card.append(&revealer);

    // Close → sembunyikan, bukan keluar.
    window.connect_close_request(|w| {
        w.set_visible(false);
        gtk::glib::Propagation::Stop
    });

    Widgets {
        window,
        compact_width: settings.appearance.compact_width,
        status_pill,
        rec_dot,
        rec_label,
        timer_label,
        spinner,
        hint_label,
        level_bar,
        transcript_box,
        question_label,
        question_meta,
        answer_label,
        keywords_row,
        notes_buffer,
        revealer,
        btn_start,
        btn_pause,
        btn_stop,
        btn_expand,
        btn_close,
        btn_next_question,
        btn_alt_answer,
        btn_explain,
        btn_settings,
        btn_history,
        btn_profile,
    }
}

/// Terapkan pengaturan overlay layer-shell (jika feature diaktifkan).
pub fn apply_overlay(window: &gtk::ApplicationWindow, settings: &Settings) {
    #[cfg(feature = "layer-shell")]
    {
        use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
        gtk4_layer_shell::init_for_window(window);
        window.set_namespace("niri-ai-copilot");
        window.set_layer(Layer::Overlay);
        // OnDemand: keyboard hanya diambil saat widget benar-benar diklik —
        // tidak mencuri fokus saat pengguna mengetik di aplikasi meeting.
        window.set_keyboard_mode(KeyboardMode::OnDemand);
        let m = settings.appearance.margin.max(0);
        match settings.appearance.corner {
            crate::config::Corner::TopLeft => {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Right, false);
                window.set_anchor(Edge::Bottom, false);
                window.set_margin(Edge::Top, m);
                window.set_margin(Edge::Left, m);
            }
            crate::config::Corner::TopRight => {
                window.set_anchor(Edge::Top, true);
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Left, false);
                window.set_anchor(Edge::Bottom, false);
                window.set_margin(Edge::Top, m);
                window.set_margin(Edge::Right, m);
            }
            crate::config::Corner::BottomLeft => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Left, true);
                window.set_anchor(Edge::Top, false);
                window.set_anchor(Edge::Right, false);
                window.set_margin(Edge::Bottom, m);
                window.set_margin(Edge::Left, m);
            }
            crate::config::Corner::BottomRight => {
                window.set_anchor(Edge::Bottom, true);
                window.set_anchor(Edge::Right, true);
                window.set_anchor(Edge::Top, false);
                window.set_anchor(Edge::Left, false);
                window.set_margin(Edge::Bottom, m);
                window.set_margin(Edge::Right, m);
            }
        }
    }
    #[cfg(not(feature = "layer-shell"))]
    {
        let _ = (window, settings);
        // Fallback: jendela GTK4 biasa. Posisi diatur oleh Niri lewat
        // window-rule (lihat docs/NIRI_SETUP.md).
    }
}

// ── Helper update UI ────────────────────────────────────────────────────

pub fn set_status(w: &Widgets, status: WidgetStatus, detail: &str) {
    w.status_pill.set_label(status.label());
    for class in WidgetStatus::all_classes() {
        w.status_pill.remove_css_class(class);
    }
    w.status_pill.add_css_class(status.css_class());
    match status {
        WidgetStatus::Processing => w.spinner.start(),
        _ => w.spinner.stop(),
    }
    set_hint(w, detail);
}

pub fn set_hint(w: &Widgets, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        w.hint_label.set_visible(false);
    } else {
        w.hint_label.set_label(text);
        w.hint_label.set_visible(true);
    }
}

pub fn set_recording(w: &Widgets, recording: bool, paused: bool, timer: &str, show_timer: bool) {
    w.rec_dot.set_visible(recording);
    w.rec_label.set_visible(recording);
    w.rec_label.set_label(if paused { "PAUSE" } else { "REC" });
    if recording && paused {
        w.rec_dot.remove_css_class("rec-dot");
        w.rec_dot.add_css_class("dim");
    } else {
        w.rec_dot.remove_css_class("dim");
        w.rec_dot.add_css_class("rec-dot");
    }
    if recording && show_timer {
        w.timer_label.set_label(timer);
        w.timer_label.set_visible(true);
    } else {
        w.timer_label.set_visible(false);
    }
}

pub fn set_level(w: &Widgets, level: f32, enabled: bool) {
    w.level_bar.set_visible(enabled);
    w.level_bar.set_fraction(level.clamp(0.0, 1.0) as f64);
}

/// Tambahkan segmen transkrip ke tampilan; batasi jumlah baris.
pub fn append_segment(w: &Widgets, seg: &TranscriptSegment, max_items: usize) {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 1);
    let meta = gtk::Label::new(Some(&format!(
        "{} · {}",
        crate::storage::format_duration_ms(seg.ts_ms),
        seg.source.label()
    )));
    meta.add_css_class("transcript-meta");
    meta.set_xalign(0.0);
    let body = gtk::Label::new(Some(seg.text.trim()));
    body.set_wrap(true);
    body.set_xalign(0.0);
    body.set_selectable(true);
    body.add_css_class(match seg.source {
        crate::storage::SegmentSource::Mic => "transcript-mic",
        crate::storage::SegmentSource::Meeting => "transcript-meeting",
    });
    row.append(&meta);
    row.append(&body);
    w.transcript_box.append(&row);

    // Trim agar tidak tumbuh tanpa batas.
    let mut children = Vec::new();
    let mut child = w.transcript_box.first_child();
    while let Some(c) = child {
        children.push(c.clone());
        child = c.next_sibling();
    }
    if children.len() > max_items {
        for c in children.iter().take(children.len() - max_items) {
            w.transcript_box.remove(c);
        }
    }
    // Auto-scroll ke bawah.
    if let Some(parent) = w.transcript_box.parent() {
        if let Ok(scroll) = parent.downcast::<gtk::ScrolledWindow>() {
            let adj = scroll.vadjustment();
            adj.set_value(adj.upper() - adj.page_size());
        }
    }
}

pub fn set_question(w: &Widgets, text: &str, category: &str, keywords: &[String]) {
    w.question_label.set_label(text.trim());
    let meta = if keywords.is_empty() {
        format!("Kategori: {category}")
    } else {
        format!("Kategori: {category} · kata kunci: {}", keywords.join(", "))
    };
    w.question_meta.set_label(&meta);
}

pub fn set_suggestion(w: &Widgets, suggestion: &Suggestion) {
    let mut out = String::new();
    out.push_str("Suggested Answer:\n");
    out.push_str(suggestion.answer.trim());
    out.push('\n');
    if !suggestion.key_points.is_empty() {
        out.push_str("\nKey Points:\n");
        for p in &suggestion.key_points {
            out.push_str(&format!("• {}\n", p.trim()));
        }
    }
    if !suggestion.technical_keywords.is_empty() {
        out.push_str("\nTechnical Keywords:\n");
        for k in &suggestion.technical_keywords {
            out.push_str(&format!("• {}\n", k.trim()));
        }
    }
    if !suggestion.follow_up_questions.is_empty() {
        out.push_str("\nFollow-up Questions:\n");
        for q in &suggestion.follow_up_questions {
            out.push_str(&format!("• {}\n", q.trim()));
        }
    }
    w.answer_label.set_label(out.trim_end());

    let chips: Vec<String> = suggestion
        .technical_keywords
        .iter()
        .chain(suggestion.key_points.iter())
        .take(8)
        .cloned()
        .collect();
    set_keywords(w, &chips);
}

pub fn set_keywords(w: &Widgets, keywords: &[String]) {
    let mut child = w.keywords_row.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        w.keywords_row.remove(&c);
        child = next;
    }
    if keywords.is_empty() {
        let empty = gtk::Label::new(Some("—"));
        empty.add_css_class("dim");
        w.keywords_row.append(&empty);
        return;
    }
    for k in keywords.iter().take(10) {
        let chip = gtk::Label::new(Some(k));
        chip.add_css_class("keyword-chip");
        w.keywords_row.append(&chip);
    }
}

pub fn clear_session_view(w: &Widgets) {
    let mut child = w.transcript_box.first_child();
    while let Some(c) = child {
        let next = c.next_sibling();
        w.transcript_box.remove(&c);
        child = next;
    }
    w.question_label
        .set_label("Belum ada pertanyaan terdeteksi.");
    w.question_meta.set_label("");
    w.answer_label.set_label(
        "Saran jawaban akan muncul di sini setelah pertanyaan terdeteksi \
         (butuh API key di Settings → AI).",
    );
    set_keywords(w, &[]);
    w.notes_buffer.set_text("");
}
