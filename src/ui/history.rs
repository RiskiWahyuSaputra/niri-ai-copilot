//! Halaman Riwayat Sesi: daftar sesi, pencarian, transkrip, ekspor, hapus.

use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

use crate::storage::{self, export, Db};
use crate::ui::dialog;
use crate::ui::panels;

pub fn show(parent: &gtk::Window, db: Rc<Db>) {
    let win = gtk::Window::builder()
        .title("Riwayat Sesi — Niri AI Copilot")
        .transient_for(parent)
        .default_width(880)
        .default_height(620)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_top(10);
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);
    win.set_child(Some(&root));

    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Cari sesi atau pertanyaan…"));
    search.set_hexpand(true);
    top.append(&search);
    let btn_refresh = panels::tool_button("Muat ulang");
    top.append(&btn_refresh);
    root.append(&top);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    body.set_vexpand(true);
    root.append(&body);

    // Kiri: daftar sesi
    let list_scroll = gtk::ScrolledWindow::builder()
        .width_request(300)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list_scroll.set_child(Some(&list));
    body.append(&list_scroll);

    // Kanan: detail
    let right = gtk::Box::new(gtk::Orientation::Vertical, 8);
    right.set_hexpand(true);
    let detail_scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let detail = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .build();
    detail.buffer().set_text(
        "Pilih sesi di kiri untuk melihat transkrip, atau gunakan pencarian.",
    );
    detail_scroll.set_child(Some(&detail));
    right.append(&detail_scroll);

    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let btn_export_txt = panels::tool_button("Ekspor TXT");
    let btn_export_md = panels::tool_button("Ekspor MD");
    let btn_delete = panels::danger_button("Hapus sesi");
    for b in [&btn_export_txt, &btn_export_md, &btn_delete] {
        b.set_sensitive(false);
    }
    actions.append(&btn_export_txt);
    actions.append(&btn_export_md);
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    actions.append(&spacer);
    actions.append(&btn_delete);
    right.append(&actions);

    let status = gtk::Label::new(None);
    status.add_css_class("dim");
    status.set_xalign(0.0);
    status.set_wrap(true);
    right.append(&status);
    body.append(&right);

    let selected: Rc<Cell<Option<i64>>> = Rc::new(Cell::new(None));

    let refresh: Rc<dyn Fn(Option<&str>)> = {
        let list = list.clone();
        let db = Rc::clone(&db);
        let detail = detail.clone();
        let status = status.clone();
        let selected = Rc::clone(&selected);
        let btn_export_txt = btn_export_txt.clone();
        let btn_export_md = btn_export_md.clone();
        let btn_delete = btn_delete.clone();
        Rc::new(move |query: Option<&str>| {
            selected.set(None);
            btn_export_txt.set_sensitive(false);
            btn_export_md.set_sensitive(false);
            btn_delete.set_sensitive(false);
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            let sessions = storage::session::list_sessions(&db, query, 200).unwrap_or_default();
            if sessions.is_empty() {
                let row = gtk::ListBoxRow::new();
                let lbl = gtk::Label::new(Some("Tidak ada sesi."));
                lbl.add_css_class("dim");
                lbl.set_margin_top(8);
                lbl.set_margin_bottom(8);
                row.set_child(Some(&lbl));
                list.append(&row);
            }
            for s in &sessions {
                let row = gtk::ListBoxRow::new();
                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
                vbox.set_margin_top(6);
                vbox.set_margin_bottom(6);
                vbox.set_margin_start(8);
                vbox.set_margin_end(8);
                let title = gtk::Label::new(Some(&s.title));
                title.set_xalign(0.0);
                title.set_wrap(true);
                let meta = gtk::Label::new(Some(&format!(
                    "{} · {} · {}",
                    storage::format_ts(s.started_at_ms),
                    s.kind.label(),
                    if s.ended_at_ms.is_some() { "selesai" } else { "berjalan" }
                )));
                meta.add_css_class("transcript-meta");
                meta.set_xalign(0.0);
                vbox.append(&title);
                vbox.append(&meta);
                row.set_child(Some(&vbox));
                unsafe { row.set_data("session_id", s.id) };
                list.append(&row);
            }

            // Bila ada query, tampilkan juga pertanyaan yang cocok.
            if let Some(q) = query {
                if !q.trim().is_empty() {
                    let questions = storage::session::search_questions(&db, q, 50).unwrap_or_default();
                    let mut text = format!("Pertanyaan yang cocok dengan \"{}\":\n\n", q.trim());
                    if questions.is_empty() {
                        text.push_str("(tidak ada)\n");
                    }
                    for qr in &questions {
                        text.push_str(&format!(
                            "- [sesi {}] {}\n",
                            qr.session_id,
                            qr.text
                        ));
                    }
                    detail.buffer().set_text(&text);
                }
            }
            status.set_label(&format!("{} sesi ditemukan.", sessions.len()));
        })
    };

    refresh(None);

    {
        let refresh = Rc::clone(&refresh);
        search.connect_search_changed(move |entry| {
            let text = entry.text().to_string();
            let query = if text.trim().is_empty() { None } else { Some(text) };
            refresh(query.as_deref());
        });
    }
    {
        let refresh = Rc::clone(&refresh);
        btn_refresh.connect_clicked(move |_| refresh(None));
    }

    // Pilih sesi → tampilkan detail.
    {
        let db = Rc::clone(&db);
        let detail = detail.clone();
        let selected = Rc::clone(&selected);
        let btn_export_txt = btn_export_txt.clone();
        let btn_export_md = btn_export_md.clone();
        let btn_delete = btn_delete.clone();
        let status = status.clone();
        list.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let Some(id) = (unsafe { row.data::<i64>("session_id") }) else {
                return;
            };
            let id = unsafe { *id.as_ref() };
            selected.set(Some(id));
            btn_export_txt.set_sensitive(true);
            btn_export_md.set_sensitive(true);
            btn_delete.set_sensitive(true);

            let mut text = String::new();
            if let Ok(Some(info)) = storage::session::get_session(&db, id) {
                text.push_str(&format!("# {}\n", info.title));
                text.push_str(&format!(
                    "Jenis: {} · Mulai: {}\n\n",
                    info.kind.label(),
                    storage::format_ts(info.started_at_ms)
                ));
                if let Some(summary) = info.summary {
                    text.push_str("## Ringkasan\n");
                    text.push_str(&summary);
                    text.push_str("\n\n");
                }
            }
            if let Ok(segments) = storage::session::transcript_for(&db, id) {
                text.push_str("## Transkrip\n");
                if segments.is_empty() {
                    text.push_str("(kosong)\n");
                }
                for seg in &segments {
                    text.push_str(&format!(
                        "[{}] {}: {}\n",
                        storage::format_duration_ms(seg.ts_ms),
                        seg.source.label(),
                        seg.text
                    ));
                }
            }
            if let Ok(questions) = storage::session::questions_for(&db, id) {
                if !questions.is_empty() {
                    text.push_str("\n## Pertanyaan\n");
                    for q in &questions {
                        text.push_str(&format!(
                            "- ({}) {}\n",
                            q.category.as_deref().unwrap_or("-"),
                            q.text
                        ));
                    }
                }
            }
            detail.buffer().set_text(&text);
            status.set_label("");
        });
    }

    // Ekspor.
    for (format, btn) in [
        (export::ExportFormat::Txt, &btn_export_txt),
        (export::ExportFormat::Markdown, &btn_export_md),
    ] {
        let db = Rc::clone(&db);
        let selected = Rc::clone(&selected);
        let win = win.clone();
        let status = status.clone();
        btn.connect_clicked(move |_| {
            let Some(id) = selected.get() else { return };
            let content = match export::export_session(&db, id, format) {
                Ok(c) => c,
                Err(e) => {
                    status.set_label(&format!("Gagal mengekspor: {e}"));
                    return;
                }
            };
            let dialog = gtk::FileDialog::builder()
                .title("Simpan transkrip")
                .initial_name(format!("sesi-{id}.{}", format.extension()))
                .build();
            let status = status.clone();
            dialog.save(Some(&win), gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        match std::fs::write(&path, &content) {
                            Ok(()) => status.set_label(&format!("Diekspor ke {}", path.display())),
                            Err(e) => status.set_label(&format!("Gagal menulis file: {e}")),
                        }
                    }
                }
            });
        });
    }

    // Hapus.
    {
        let db = Rc::clone(&db);
        let selected = Rc::clone(&selected);
        let win = win.clone();
        let refresh = Rc::clone(&refresh);
        let status = status.clone();
        btn_delete.connect_clicked(move |_| {
            let Some(id) = selected.get() else { return };
            let refresh = Rc::clone(&refresh);
            let status = status.clone();
            let db_inner = Rc::clone(&db);
            dialog::show_confirm(
                &win,
                "Hapus sesi ini?",
                "Transkrip, pertanyaan, dan saran sesi ini akan dihapus permanen.",
                move || {
                    match storage::session::delete_session(&db_inner, id) {
                        Ok(()) => {
                            status.set_label("Sesi dihapus.");
                            refresh(None);
                        }
                        Err(e) => status.set_label(&format!("Gagal menghapus: {e}")),
                    }
                },
            );
        });
    }

    win.present();
}
