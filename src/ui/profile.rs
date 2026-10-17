//! Halaman Profil Konteks AI: data nyata pengguna untuk dasar saran jawaban.
//!
//! Catatan: `ComboBoxText` deprecated di binding sejak GTK 4.10 tetapi masih
//! didukung runtime GTK 4.x (lihat docs/TESTING.md).
#![allow(deprecated)]

use gtk4 as gtk;
use gtk::prelude::*;
use std::rc::Rc;

use crate::storage::{cv, profile, Db};
use crate::ui::panels;

pub fn show<F>(parent: &gtk::Window, db: Rc<Db>, on_saved: F)
where
    F: Fn() + 'static,
{
    let current = profile::load_profile(&db).unwrap_or_default();

    let win = gtk::Window::builder()
        .title("Profil Interview — Niri AI Copilot")
        .transient_for(parent)
        .default_width(640)
        .default_height(700)
        .build();

    let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root.set_margin_top(10);
    root.set_margin_bottom(10);
    root.set_margin_start(10);
    root.set_margin_end(10);
    win.set_child(Some(&root));

    let note = gtk::Label::new(Some(
        "Profil ini menjadi dasar saran jawaban. Aplikasi tidak akan mengarang \
         pengalaman — bagian kosong akan ditandai [PERLU DILENGKAPI].",
    ));
    note.add_css_class("dim");
    note.set_wrap(true);
    note.set_xalign(0.0);
    root.append(&note);

    let scrolled = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let form = gtk::Box::new(gtk::Orientation::Vertical, 8);
    scrolled.set_child(Some(&form));
    root.append(&scrolled);

    let name = entry_row(&form, "Nama", &current.name);
    let target = entry_row(&form, "Posisi yang dilamar", &current.target_position);

    let lang = gtk::ComboBoxText::new();
    lang.append(Some("auto"), "Auto (ikuti pertanyaan)");
    lang.append(Some("id"), "Indonesia");
    lang.append(Some("en"), "Inggris");
    lang.set_active_id(Some(if current.interview_language.is_empty() {
        "auto"
    } else {
        current.interview_language.as_str()
    }));
    labeled_row(&form, "Bahasa interview", &lang);

    let job_desc = text_row(&form, "Deskripsi pekerjaan", &current.job_description, 80);
    let experience = text_row(&form, "Pengalaman kerja/organisasi", &current.experience, 110);
    let skills = text_row(&form, "Skill teknis", &current.skills, 80);
    let projects = text_row(&form, "Project portofolio", &current.projects, 110);
    let education = text_row(&form, "Pendidikan", &current.education, 70);
    let certifications = text_row(&form, "Sertifikasi", &current.certifications, 70);

    // CV
    let cv_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cv_label = gtk::Label::new(Some("CV (PDF/TXT/MD)"));
    cv_label.set_xalign(0.0);
    cv_label.set_hexpand(true);
    let cv_path = gtk::Entry::new();
    cv_path.set_text(&current.cv_path);
    cv_path.set_placeholder_text(Some("/path/ke/cv.pdf"));
    let btn_load_cv = panels::tool_button("Muat dari file…");
    cv_header.append(&cv_label);
    cv_header.append(&cv_path);
    cv_header.append(&btn_load_cv);
    form.append(&cv_header);

    let cv_text = text_row(&form, "Teks CV", &current.cv_text, 160);
    let cv_status = gtk::Label::new(None);
    cv_status.add_css_class("dim");
    cv_status.set_wrap(true);
    cv_status.set_xalign(0.0);
    form.append(&cv_status);

    {
        let win = win.clone();
        let cv_path = cv_path.clone();
        let cv_text = cv_text.clone();
        let cv_status = cv_status.clone();
        btn_load_cv.connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder()
                .title("Pilih file CV")
                .build();
            let cv_path = cv_path.clone();
            let cv_text = cv_text.clone();
            let cv_status = cv_status.clone();
            let win = win.clone();
            dialog.open(Some(&win), gtk::gio::Cancellable::NONE, move |res| {
                let Ok(file) = res else { return };
                let Some(path) = file.path() else { return };
                match cv::extract_text(&path) {
                    Ok(text) => {
                        cv_path.set_text(&path.display().to_string());
                        cv_text.buffer().set_text(&text);
                        cv_status.set_label(&format!(
                            "CV dimuat: {} karakter.",
                            text.chars().count()
                        ));
                    }
                    Err(e) => cv_status.set_label(&format!("Gagal memuat CV: {e}")),
                }
            });
        });
    }

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let missing = gtk::Label::new(None);
    missing.add_css_class("hint");
    missing.set_xalign(0.0);
    missing.set_hexpand(true);
    missing.set_wrap(true);
    let btn_save = panels::primary_button("Simpan Profil");
    let btn_close = panels::tool_button("Tutup");
    footer.append(&missing);
    footer.append(&btn_save);
    footer.append(&btn_close);
    root.append(&footer);

    {
        let win = win.clone();
        btn_close.connect_clicked(move |_| win.close());
    }

    let on_saved = Rc::new(on_saved);
    {
        let db = Rc::clone(&db);
        let missing = missing.clone();
        btn_save.connect_clicked(move |_| {
            let p = profile::Profile {
                name: name.text().to_string(),
                target_position: target.text().to_string(),
                job_description: text_of(&job_desc),
                experience: text_of(&experience),
                skills: text_of(&skills),
                projects: text_of(&projects),
                education: text_of(&education),
                certifications: text_of(&certifications),
                interview_language: lang
                    .active_id()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "auto".into()),
                cv_text: text_of(&cv_text),
                cv_path: cv_path.text().to_string(),
            };
            match profile::save_profile(&db, &p) {
                Ok(()) => {
                    let missing_fields = p.missing_fields();
                    if missing_fields.is_empty() {
                        missing.set_label("Profil tersimpan.");
                    } else {
                        missing.set_label(&format!(
                            "Profil tersimpan. Perlu dilengkapi: {}.",
                            missing_fields.join(", ")
                        ));
                    }
                    on_saved();
                }
                Err(e) => missing.set_label(&format!("Gagal menyimpan: {e}")),
            }
        });
    }

    // Tampilkan peringatan awal bila profil masih kosong.
    {
        let missing_fields = current.missing_fields();
        if !missing_fields.is_empty() {
            missing.set_label(&format!(
                "Belum lengkap: {}.",
                missing_fields.join(", ")
            ));
        }
    }

    win.present();
}

fn entry_row(form: &gtk::Box, label: &str, value: &str) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_text(value);
    labeled_row(form, label, &e);
    e
}

fn labeled_row(form: &gtk::Box, label: &str, widget: &impl IsA<gtk::Widget>) {
    let l = gtk::Label::new(Some(label));
    l.add_css_class("panel-title");
    l.set_xalign(0.0);
    form.append(&l);
    form.append(widget);
}

fn text_row(form: &gtk::Box, label: &str, value: &str, height: i32) -> gtk::TextView {
    let l = gtk::Label::new(Some(label));
    l.add_css_class("panel-title");
    l.set_xalign(0.0);
    form.append(&l);
    let view = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .height_request(height)
        .build();
    view.add_css_class("notes");
    view.buffer().set_text(value);
    form.append(&view);
    view
}

/// Nama diubah agar menyerupai helper yang dipakai pengujian (tetap TextView).
type TextWidget = gtk::TextView;
fn text_of(view: &TextWidget) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}
