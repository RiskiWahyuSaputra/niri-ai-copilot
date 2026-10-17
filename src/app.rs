//! Orkestrasi aplikasi: lifecycle sesi, event loop UI, worker audio/STT/AI, IPC.

use gtk4 as gtk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ai::{self, prompts::Suggestion, PracticeReport};
use crate::audio;
use crate::audio::capture::{AudioSourceKind, CaptureEvent, CaptureHandle};
use crate::config::{paths, Settings};
use crate::error::{AppError, Result};
use crate::ipc::{IpcCommand, IpcServer, SharedStatus};
use crate::practice::bank;
use crate::privacy::ConsentGate;
use crate::storage::profile::Profile;
use crate::storage::session::{self, SegmentSource, SessionKind, TranscriptSegment};
use crate::storage::Db;
use crate::transcription::{self, SttWorker};
use crate::ui::state::{AppEvent, StartOptions, WidgetStatus};
use crate::ui::{self, dialog, widget::Widgets};

pub const APP_ID: &str = "io.github.riskiwahyusaputra.niri-ai-copilot";

pub struct RunningSession {
    pub id: i64,
    pub kind: SessionKind,
    pub started_ms: i64,
    pub paused_total_ms: i64,
    pub paused_at: Option<i64>,
    pub paused: Arc<AtomicBool>,
    pub captures: Vec<CaptureHandle>,
    pub stt: Option<SttWorker>,
    pub stt_event_tx: mpsc::Sender<transcription::SttEvent>,
    pub detector: ai::QuestionDetector,
    pub transcript: Vec<TranscriptSegment>,
    pub questions: Vec<String>,
    pub last_question_id: Option<i64>,
    pub last_suggestion: Option<Suggestion>,
    pub capture_rx: mpsc::Receiver<CaptureEvent>,
    pub stt_rx: mpsc::Receiver<transcription::SttEvent>,
    pub external_ai_ok: bool,
    pub save_audio: bool,
}

pub struct AppCtx {
    pub settings: Rc<RefCell<Settings>>,
    pub db: Rc<Db>,
    pub profile: RefCell<Profile>,
    pub consent: ConsentGate,
    pub widgets: RefCell<Option<Widgets>>,
    pub session: RefCell<Option<RunningSession>>,
    pub ai_rx: RefCell<mpsc::Receiver<AppEvent>>,
    pub ai_tx: mpsc::Sender<AppEvent>,
    pub ai_pending: Arc<AtomicBool>,
    pub ipc_rx: RefCell<Option<mpsc::Receiver<IpcCommand>>>,
    pub ipc_server: RefCell<Option<IpcServer>>,
    pub status_snapshot: SharedStatus,
    pub last_session_id: RefCell<Option<i64>>,
    pub db_degraded: RefCell<Option<String>>,
}

impl AppCtx {
    fn status(&self) -> WidgetStatus {
        match self.session.borrow().as_ref() {
            None => WidgetStatus::Idle,
            Some(s) => {
                if s.paused_at.is_some() {
                    WidgetStatus::Paused
                } else {
                    WidgetStatus::Listening
                }
            }
        }
    }

    fn set_hint(&self, text: &str) {
        if let Some(w) = self.widgets.borrow().as_ref() {
            ui::widget::set_hint(w, text);
        }
    }

    fn refresh_status(&self, detail: &str) {
        let status = self.status();
        if let Some(w) = self.widgets.borrow().as_ref() {
            ui::widget::set_status(w, status, detail);
        }
        self.update_snapshot(status);
    }

    fn update_snapshot(&self, status: WidgetStatus) {
        let session_desc = match self.session.borrow().as_ref() {
            Some(s) => format!("sesi {} ({})", s.id, s.kind.label()),
            None => "tidak ada sesi".to_string(),
        };
        let text = format!(
            "status={}; {}; transkripsi={}",
            status.label().to_lowercase(),
            session_desc,
            if self.stt_ready() { "siap" } else { "belum siap" }
        );
        if let Ok(mut slot) = self.status_snapshot.lock() {
            *slot = text;
        }
    }

    fn stt_ready(&self) -> bool {
        let settings = self.settings.borrow();
        transcription::build_engine(&settings).check_ready().is_ok()
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Entry point aplikasi GTK.
pub fn run(smoke: bool) -> Result<()> {
    crate::config::load_dotenv();
    paths::ensure_dirs()?;

    let settings = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("peringatan: gagal memuat konfigurasi ({e}); memakai default");
            Settings::default()
        }
    };

    if settings.privacy.save_audio {
        let _ = crate::privacy::enforce_audio_retention(
            &paths::recordings_dir(),
            settings.privacy.audio_retention_days,
        );
    }

    let (db, db_degraded) = match Db::open(&paths::db_path()) {
        Ok(db) => (db, None),
        Err(e) => {
            eprintln!("peringatan: database tidak dapat dibuka ({e}); memakai database sementara");
            (
                Db::open_in_memory().map_err(AppError::from)?,
                Some(format!(
                    "Database utama tidak dapat dibuka ({e}). Riwayat sesi tidak akan tersimpan permanen."
                )),
            )
        }
    };
    let profile = crate::storage::load_profile(&db).unwrap_or_default();

    let (ai_tx, ai_rx) = mpsc::channel::<AppEvent>();

    let app = gtk::Application::builder().application_id(APP_ID).build();
    let smoke_flag = Rc::new(smoke);

    let ctx = Rc::new(AppCtx {
        settings: Rc::new(RefCell::new(settings)),
        db: Rc::new(db),
        profile: RefCell::new(profile),
        consent: ConsentGate::new(),
        widgets: RefCell::new(None),
        session: RefCell::new(None),
        ai_rx: RefCell::new(ai_rx),
        ai_tx,
        ai_pending: Arc::new(AtomicBool::new(false)),
        ipc_rx: RefCell::new(None),
        ipc_server: RefCell::new(None),
        status_snapshot: Arc::new(std::sync::Mutex::new("status=starting".into())),
        last_session_id: RefCell::new(None),
        db_degraded: RefCell::new(db_degraded),
    });

    let ctx_activate = Rc::clone(&ctx);
    app.connect_activate(move |app| {
        activate(app, &ctx_activate);
        if *smoke_flag {
            let app = app.clone();
            glib::timeout_add_seconds_local(1, move || {
                println!("smoke-test: aplikasi berhasil start tanpa error");
                app.quit();
                glib::ControlFlow::Break
            });
        }
    });

    let ctx_shutdown = Rc::clone(&ctx);
    app.connect_shutdown(move |_| {
        shutdown(&ctx_shutdown);
    });

    app.run_with_args::<&str>(&[]);
    Ok(())
}

fn activate(app: &gtk::Application, ctx: &Rc<AppCtx>) {
    let settings = ctx.settings.borrow().clone();
    ui::theme::apply(&settings);

    let widgets = ui::widget::build(app, &settings);
    ui::widget::apply_overlay(&widgets.window, &settings);
    wire_widgets(ctx, &widgets);
    ctx.widgets.replace(Some(widgets));

    let (ipc_tx, ipc_rx) = mpsc::channel::<IpcCommand>();
    match IpcServer::start(ipc_tx, Arc::clone(&ctx.status_snapshot)) {
        Ok(server) => {
            *ctx.ipc_server.borrow_mut() = Some(server);
        }
        Err(e) => {
            ctx.set_hint(&format!(
                "IPC tidak aktif ({e}) — shortcut Niri via `niri-ai-copilot <cmd>` tidak akan bekerja."
            ));
        }
    }
    *ctx.ipc_rx.borrow_mut() = Some(ipc_rx);

    let ctx_poll = Rc::clone(ctx);
    glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
        poll_events(&ctx_poll);
        glib::ControlFlow::Continue
    });

    if let Some(w) = ctx.widgets.borrow().as_ref() {
        if let Some(degraded) = ctx.db_degraded.borrow().clone() {
            ui::widget::set_hint(w, &degraded);
        } else {
            let stt = transcription::status_text(&settings);
            if stt.starts_with("STT belum siap") {
                ui::widget::set_hint(w, &format!("{stt}. Atur di Pengaturan → Audio."));
            }
        }
        w.window.set_visible(true);
    }
    ctx.update_snapshot(ctx.status());
}

fn wire_widgets(ctx: &Rc<AppCtx>, w: &Widgets) {
    {
        let ctx = Rc::clone(ctx);
        w.btn_start.connect_clicked(move |_| begin_start_flow(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_pause.connect_clicked(move |_| toggle_pause(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_stop.connect_clicked(move |_| stop_session(&ctx, None));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_expand.connect_clicked(move |_| toggle_expand(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_close.connect_clicked(move |_| {
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.window.set_visible(false);
            }
        });
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_next_question
            .connect_clicked(move |_| request_practice_question(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_alt_answer
            .connect_clicked(move |_| request_alternative(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_explain.connect_clicked(move |_| request_explain(&ctx));
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_settings.connect_clicked(move |_| {
            if let Some(parent) = parent_window(&ctx) {
                let ctx2 = Rc::clone(&ctx);
                ui::settings::show(
                    &parent,
                    Rc::clone(&ctx.settings),
                    Rc::clone(&ctx.db),
                    move || {
                        let settings = ctx2.settings.borrow().clone();
                        ui::theme::apply_dynamic(&settings);
                        let stt = transcription::status_text(&settings);
                        if stt.starts_with("STT belum siap") {
                            ctx2.set_hint(&format!("{stt}. Atur di Pengaturan → Audio."));
                        } else {
                            ctx2.set_hint("");
                        }
                    },
                );
            }
        });
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_history.connect_clicked(move |_| {
            if let Some(parent) = parent_window(&ctx) {
                ui::history::show(&parent, Rc::clone(&ctx.db));
            }
        });
    }
    {
        let ctx = Rc::clone(ctx);
        w.btn_profile.connect_clicked(move |_| {
            if let Some(parent) = parent_window(&ctx) {
                let ctx2 = Rc::clone(&ctx);
                ui::profile::show(&parent, Rc::clone(&ctx.db), move || {
                    if let Ok(p) = crate::storage::load_profile(&ctx2.db) {
                        *ctx2.profile.borrow_mut() = p;
                    }
                });
            }
        });
    }
}

fn parent_window(ctx: &Rc<AppCtx>) -> Option<gtk::Window> {
    ctx.widgets
        .borrow()
        .as_ref()
        .map(|w| w.window.clone().upcast::<gtk::Window>())
}

fn begin_start_flow(ctx: &Rc<AppCtx>) {
    if ctx.session.borrow().is_some() {
        ctx.set_hint("Sesi sedang berjalan — hentikan dulu sebelum memulai yang baru.");
        return;
    }
    let Some(parent) = parent_window(ctx) else { return };
    let settings = ctx.settings.borrow().clone();
    let devices = match audio::list_sources() {
        Ok(d) => d,
        Err(e) => {
            dialog::show_error(
                &parent,
                "PipeWire tidak tersedia",
                &format!(
                    "{e}\n\nPastikan PipeWire berjalan (mis. `systemctl --user status pipewire`) \
                     dan `pactl info` berhasil."
                ),
            );
            return;
        }
    };
    let ctx = Rc::clone(ctx);
    dialog::show_consent_dialog(&parent, &settings, &devices, move |opts| {
        start_session(&ctx, opts);
    });
}

fn start_session(ctx: &Rc<AppCtx>, opts: StartOptions) {
    if !opts.consent_confirmed {
        ctx.set_hint("Sesi tidak dimulai: izin perekaman belum dikonfirmasi.");
        return;
    }

    let settings = ctx.settings.borrow().clone();

    let engine = transcription::build_engine(&settings);
    if let Err(e) = engine.check_ready() {
        if let Some(parent) = parent_window(ctx) {
            dialog::show_error(
                &parent,
                "Speech-to-text belum siap",
                &format!(
                    "{e}\n\nPerekaman tidak dimulai karena transkripsi lokal belum dapat berjalan. \
                     Pasang whisper.cpp + model GGML, lalu atur di Pengaturan → Audio."
                ),
            );
        }
        return;
    }

    let mut sources = Vec::new();
    if let Some(mic) = &opts.mic_device {
        sources.push(mic.clone());
    }
    if let Some(meeting) = &opts.meeting_device {
        sources.push(meeting.clone());
    }
    ctx.consent.confirm(sources, opts.meeting_device.is_some());

    let started_ms = now_ms();
    let title = format!(
        "{} — {}",
        opts.kind.label(),
        crate::storage::format_ts(started_ms)
    );
    let session_id = match session::create_session(&ctx.db, &opts.kind, &title) {
        Ok(id) => id,
        Err(e) => {
            ctx.set_hint(&format!("Gagal membuat sesi di database: {e}"));
            ctx.consent.revoke();
            return;
        }
    };
    *ctx.last_session_id.borrow_mut() = Some(session_id);

    let (cap_tx, cap_rx) = mpsc::sync_channel::<CaptureEvent>(8);
    let paused = Arc::new(AtomicBool::new(false));
    let mut captures = Vec::new();

    let want_mic = opts.mic_device.is_some() || opts.meeting_device.is_none();
    if want_mic {
        match CaptureHandle::spawn(
            AudioSourceKind::Mic,
            opts.mic_device.clone(),
            settings.audio.chunk_seconds,
            Arc::clone(&paused),
            cap_tx.clone(),
        ) {
            Ok(h) => captures.push(h),
            Err(e) => ctx.set_hint(&format!("Gagal memulai microphone: {e}")),
        }
    }
    if let Some(meeting) = opts.meeting_device.clone() {
        match CaptureHandle::spawn(
            AudioSourceKind::Meeting,
            Some(meeting),
            settings.audio.chunk_seconds,
            Arc::clone(&paused),
            cap_tx.clone(),
        ) {
            Ok(h) => captures.push(h),
            Err(e) => ctx.set_hint(&format!("Gagal memulai audio meeting: {e}")),
        }
    }
    drop(cap_tx);

    if captures.is_empty() {
        let _ = session::end_session(&ctx.db, session_id, Some("gagal memulai audio"));
        ctx.consent.revoke();
        ctx.set_hint("Sesi dibatalkan: tidak ada sumber audio yang berhasil dimulai.");
        return;
    }

    let (stt_tx, stt_rx) = mpsc::channel();
    let stt = SttWorker::spawn(
        engine,
        settings.audio.language.clone(),
        settings.audio.max_chunk_queue,
        stt_tx.clone(),
    );

    let detector = ai::QuestionDetector::new(
        settings.performance.ai_debounce_ms,
        settings.ai.question_min_words,
    );

    let kind = opts.kind.clone();
    let running = RunningSession {
        id: session_id,
        kind: kind.clone(),
        started_ms,
        paused_total_ms: 0,
        paused_at: None,
        paused,
        captures,
        stt: Some(stt),
        stt_event_tx: stt_tx,
        detector,
        transcript: Vec::new(),
        questions: Vec::new(),
        last_question_id: None,
        last_suggestion: None,
        capture_rx: cap_rx,
        stt_rx,
        external_ai_ok: opts.external_ai_ok,
        save_audio: settings.privacy.save_audio,
    };
    *ctx.session.borrow_mut() = Some(running);

    if let Some(w) = ctx.widgets.borrow().as_ref() {
        ui::widget::clear_session_view(w);
        ui::widget::set_recording(w, true, false, "00:00", settings.privacy.show_recording_timer);
        ui::widget::set_level(w, 0.0, settings.audio.show_level_meter);
        w.btn_start.set_sensitive(false);
        w.btn_pause.set_sensitive(true);
        w.btn_pause.set_label("Pause");
        w.btn_stop.set_sensitive(true);
        w.btn_next_question.set_sensitive(kind == SessionKind::Practice);
    }
    let hint = match kind {
        SessionKind::Practice => "Sesi latihan dimulai — jawab dengan suara Anda.",
        SessionKind::Live => {
            "Interview langsung dimulai. Pastikan semua peserta sudah memberi izin."
        }
        SessionKind::Transcription => "Sesi transkripsi dimulai.",
    };
    ctx.refresh_status(hint);

    if kind == SessionKind::Practice {
        request_practice_question(ctx);
    }
}

fn toggle_pause(ctx: &Rc<AppCtx>) {
    let settings = ctx.settings.borrow().clone();
    {
        let mut session = ctx.session.borrow_mut();
        let Some(s) = session.as_mut() else { return };
        let now = now_ms();
        if let Some(paused_at) = s.paused_at.take() {
            s.paused_total_ms += now - paused_at;
            s.paused.store(false, Ordering::SeqCst);
        } else {
            s.paused_at = Some(now);
            s.paused.store(true, Ordering::SeqCst);
        }
    }
    let paused = ctx
        .session
        .borrow()
        .as_ref()
        .map(|s| s.paused_at.is_some())
        .unwrap_or(false);
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        ui::widget::set_recording(w, true, paused, "", settings.privacy.show_recording_timer);
        w.btn_pause.set_label(if paused { "Resume" } else { "Pause" });
    }
    ctx.refresh_status(if paused { "Perekaman dijeda." } else { "" });
}

fn stop_session(ctx: &Rc<AppCtx>, reason: Option<String>) {
    let running = ctx.session.borrow_mut().take();
    let Some(mut running) = running else { return };

    // 1. Hentikan capture, lalu kirim sisa chunk ke STT.
    let captures = std::mem::take(&mut running.captures);
    for c in captures {
        c.stop();
    }
    while let Ok(ev) = running.capture_rx.try_recv() {
        if let CaptureEvent::Chunk(chunk) = ev {
            let job = transcription::SttJob {
                wav: chunk.wav,
                source: chunk.source,
                start_ms: chunk.start_ms,
                seq: chunk.seq,
            };
            if let Some(worker) = running.stt.as_ref() {
                let _ = worker.submit(job, &running.stt_event_tx);
            }
        }
    }

    // 2. Tutup worker STT (memproses sisa antrean) dan serap hasilnya.
    if let Some(stt) = running.stt.take() {
        stt.shutdown();
    }
    let mut final_segments: Vec<TranscriptSegment> = Vec::new();
    while let Ok(ev) = running.stt_rx.try_recv() {
        if let transcription::SttEvent::Done {
            source,
            start_ms,
            text,
        } = ev
        {
            final_segments.push(TranscriptSegment {
                ts_ms: start_ms,
                source: to_segment_source(source),
                text,
            });
        }
    }
    for seg in final_segments {
        running.transcript.push(seg.clone());
        persist_and_show_segment(ctx, &seg);
    }

    let transcript_count = running.transcript.len();
    let session_id = running.id;
    let kind = running.kind.clone();
    let external_ai_ok = running.external_ai_ok;
    let transcript_snapshot = running.transcript.clone();
    let questions_snapshot = running.questions.clone();

    let note = reason.unwrap_or_else(|| "dihentikan pengguna".into());
    let _ = session::end_session(&ctx.db, session_id, None);
    ctx.consent.revoke();

    if let Some(w) = ctx.widgets.borrow().as_ref() {
        ui::widget::set_recording(w, false, false, "", false);
        ui::widget::set_level(w, 0.0, false);
        w.btn_start.set_sensitive(true);
        w.btn_pause.set_sensitive(false);
        w.btn_pause.set_label("Pause");
        w.btn_stop.set_sensitive(false);
        w.btn_next_question.set_sensitive(false);
    }
    ctx.refresh_status(&format!(
        "Sesi selesai — {transcript_count} segmen tersimpan ({note})."
    ));

    // 3. Ringkasan / evaluasi AI.
    let settings = ctx.settings.borrow().clone();
    let ai_allowed = external_ai_ok && matches!(kind, SessionKind::Practice | SessionKind::Live);
    let has_key = ai::build_provider(&settings).is_ok();
    if ai_allowed && has_key && !transcript_snapshot.is_empty() {
        if let Some(w) = ctx.widgets.borrow().as_ref() {
            w.answer_label.set_label(if kind == SessionKind::Practice {
                "Menyiapkan evaluasi latihan…"
            } else {
                "Menyiapkan ringkasan sesi…"
            });
        }
        let settings2 = settings.clone();
        let practice = kind == SessionKind::Practice;
        spawn_ai_job(ctx, move || {
            let provider = ai::build_provider(&settings2)?;
            if practice {
                let report = ai::evaluate_practice_session(
                    provider.as_ref(),
                    &transcript_snapshot,
                    &questions_snapshot,
                )?;
                Ok(AiOutcome::Report(report))
            } else {
                let summary = ai::summarize_session(provider.as_ref(), &transcript_snapshot)?;
                Ok(AiOutcome::Summary(summary))
            }
        });
    } else if kind == SessionKind::Practice {
        if let Some(w) = ctx.widgets.borrow().as_ref() {
            w.answer_label.set_label(
                "Latihan selesai. Evaluasi AI tidak dijalankan (API key/izin AI belum tersedia) \
                 — transkrip tetap tersimpan di Riwayat.",
            );
        }
    }

    if settings.privacy.save_audio {
        let _ = crate::privacy::enforce_audio_retention(
            &paths::recordings_dir(),
            settings.privacy.audio_retention_days,
        );
    }
}

enum AiOutcome {
    Report(PracticeReport),
    Summary(String),
    Question { text: String, from_ai: bool },
    Suggestion { question_id: i64, suggestion: Suggestion },
    Alternative { question_id: i64, suggestion: Suggestion },
    Explanation(String),
}

/// Jalankan tugas AI di thread terpisah; hasilnya dikirim ke UI sebagai AppEvent.
/// Hanya satu tugas AI berjalan pada satu waktu (debounce & anti-spam).
fn spawn_ai_job<F>(ctx: &Rc<AppCtx>, job: F)
where
    F: FnOnce() -> Result<AiOutcome> + Send + 'static,
{
    if ctx.ai_pending.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        w.spinner.start();
        ui::widget::set_status(w, WidgetStatus::Processing, "AI sedang bekerja…");
    }
    let pending = Arc::clone(&ctx.ai_pending);
    let tx = ctx.ai_tx.clone();
    std::thread::spawn(move || {
        let outcome = job();
        pending.store(false, Ordering::SeqCst);
        let event = match outcome {
            Ok(AiOutcome::Report(r)) => AppEvent::PracticeReportReady(r),
            Ok(AiOutcome::Summary(t)) => AppEvent::SummaryReady(t),
            Ok(AiOutcome::Question { text, from_ai }) => AppEvent::PracticeQuestion { text, from_ai },
            Ok(AiOutcome::Suggestion {
                question_id,
                suggestion,
            }) => AppEvent::SuggestionReady {
                question_id,
                suggestion,
            },
            Ok(AiOutcome::Alternative {
                question_id,
                suggestion,
            }) => AppEvent::AlternativeReady {
                question_id,
                suggestion,
            },
            Ok(AiOutcome::Explanation(text)) => AppEvent::ExplanationReady(text),
            Err(e) => AppEvent::Error(e.to_string()),
        };
        let _ = tx.send(event);
    });
}

fn request_practice_question(ctx: &Rc<AppCtx>) {
    let settings = ctx.settings.borrow().clone();
    let profile = ctx.profile.borrow().clone();

    let (category, external_ai_ok, asked, kind) = {
        let session = ctx.session.borrow();
        let Some(s) = session.as_ref() else {
            ctx.set_hint("Mulai sesi Practice Mode dulu untuk meminta pertanyaan.");
            return;
        };
        (
            settings.practice.category.clone(),
            s.external_ai_ok,
            s.questions.clone(),
            s.kind.clone(),
        )
    };
    if kind != SessionKind::Practice {
        ctx.set_hint("Pertanyaan latihan hanya tersedia di Practice Mode.");
        return;
    }

    let provider_ok = external_ai_ok && ai::build_provider(&settings).is_ok();
    if provider_ok {
        let category_ai = category.clone();
        let asked_ai = asked.clone();
        let settings_ai = settings.clone();
        let profile_ai = profile.clone();
        spawn_ai_job(ctx, move || {
            let provider = ai::build_provider(&settings_ai)?;
            match ai::ask_practice_question(
                provider.as_ref(),
                &category_ai,
                &asked_ai,
                &profile_ai,
                false,
            ) {
                Ok(q) => Ok(AiOutcome::Question {
                    text: q,
                    from_ai: true,
                }),
                Err(_) => Ok(AiOutcome::Question {
                    text: bank::next_question(&category_ai, &asked_ai, epoch_salt()),
                    from_ai: false,
                }),
            }
        });
    } else {
        let q = bank::next_question(&category, &asked, epoch_salt());
        let _ = ctx.ai_tx.send(AppEvent::PracticeQuestion {
            text: q,
            from_ai: false,
        });
    }
}

fn request_alternative(ctx: &Rc<AppCtx>) {
    let settings = ctx.settings.borrow().clone();
    let profile = ctx.profile.borrow().clone();

    let (question_id, question, previous) = {
        let session = ctx.session.borrow();
        let Some(s) = session.as_ref() else {
            ctx.set_hint("Belum ada sesi aktif.");
            return;
        };
        let Some(qid) = s.last_question_id else {
            ctx.set_hint("Belum ada pertanyaan yang terdeteksi.");
            return;
        };
        let question = s
            .questions
            .last()
            .cloned()
            .unwrap_or_else(|| "pertanyaan terakhir".into());
        let previous = s
            .last_suggestion
            .as_ref()
            .map(|sug| sug.answer.clone())
            .unwrap_or_default();
        (qid, question, previous)
    };

    let has_key = ai::build_provider(&settings).is_ok();
    if !has_key {
        ctx.set_hint("Alternatif jawaban butuh API key (Pengaturan → AI).");
        return;
    }
    spawn_ai_job(ctx, move || {
        let provider = ai::build_provider(&settings)?;
        let suggestion = ai::generate_alternative(
            provider.as_ref(),
            &settings,
            &profile,
            &question,
            &previous,
        )?;
        Ok(AiOutcome::Alternative {
            question_id,
            suggestion,
        })
    });
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        w.answer_label.set_label("Membuat alternatif jawaban…");
    }
}

fn request_explain(ctx: &Rc<AppCtx>) {
    let settings = ctx.settings.borrow().clone();
    let (term, context) = {
        let session = ctx.session.borrow();
        let Some(s) = session.as_ref() else {
            ctx.set_hint("Belum ada sesi aktif.");
            return;
        };
        let term = s
            .last_suggestion
            .as_ref()
            .and_then(|sug| sug.technical_keywords.first().cloned())
            .or_else(|| {
                s.last_suggestion
                    .as_ref()
                    .and_then(|sug| sug.key_points.first().cloned())
            });
        let Some(term) = term else {
            ctx.set_hint("Belum ada istilah untuk dijelaskan (buat saran jawaban dulu).");
            return;
        };
        let context = s
            .transcript
            .last()
            .map(|seg| seg.text.clone())
            .unwrap_or_default();
        (term, context)
    };
    if ai::build_provider(&settings).is_err() {
        ctx.set_hint("Fitur penjelasan istilah butuh API key (Pengaturan → AI).");
        return;
    }
    spawn_ai_job(ctx, move || {
        let provider = ai::build_provider(&settings)?;
        let text = ai::explain_term(provider.as_ref(), &term, &context)?;
        Ok(AiOutcome::Explanation(text))
    });
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        w.answer_label.set_label("Menjelaskan istilah…");
    }
}

fn spawn_suggestion(ctx: &Rc<AppCtx>, question_id: i64, question: String, context: String) {
    let settings = ctx.settings.borrow().clone();
    let profile = ctx.profile.borrow().clone();
    spawn_ai_job(ctx, move || {
        let provider = ai::build_provider(&settings)?;
        let suggestion = ai::generate_suggestion(
            provider.as_ref(),
            &settings,
            &profile,
            &context,
            &question,
        )?;
        Ok(AiOutcome::Suggestion {
            question_id,
            suggestion,
        })
    });
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        w.answer_label.set_label("Menghasilkan saran jawaban…");
    }
    ctx.refresh_status("Memproses saran jawaban…");
}

fn epoch_salt() -> u64 {
    (now_ms() / 1000) as u64
}

fn to_segment_source(source: AudioSourceKind) -> SegmentSource {
    match source {
        AudioSourceKind::Mic => SegmentSource::Mic,
        AudioSourceKind::Meeting => SegmentSource::Meeting,
    }
}

/// Simpan segmen ke DB dan tampilkan di UI.
fn persist_and_show_segment(ctx: &Rc<AppCtx>, seg: &TranscriptSegment) {
    if let Some(s) = ctx.session.borrow().as_ref() {
        let _ = session::insert_transcript(&ctx.db, s.id, seg.ts_ms, &seg.source, &seg.text);
    }
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        let max_items = ctx
            .settings
            .borrow()
            .performance
            .max_transcript_context
            .min(400);
        ui::widget::append_segment(w, seg, max_items);
    }
}

fn handle_segment_event(ctx: &Rc<AppCtx>, source: AudioSourceKind, ts_ms: i64, text: String) {
    let settings = ctx.settings.borrow().clone();
    let seg = TranscriptSegment {
        ts_ms,
        source: to_segment_source(source),
        text,
    };

    // Deteksi pertanyaan di dalam scope terpisah agar borrow tidak menumpuk.
    let detected: Option<(i64, String, String, Vec<String>)> = {
        let mut session = ctx.session.borrow_mut();
        let Some(s) = session.as_mut() else { return };
        s.transcript.push(seg.clone());
        let _ = session::insert_transcript(&ctx.db, s.id, seg.ts_ms, &seg.source, &seg.text);
        if let Some(w) = ctx.widgets.borrow().as_ref() {
            let max_items = settings.performance.max_transcript_context.min(400);
            ui::widget::append_segment(w, &seg, max_items);
        }

        // Hanya mode Live yang mendeteksi pertanyaan interviewer.
        if s.kind != SessionKind::Live {
            None
        } else if let Some(c) = s.detector.feed(&seg.text, now_ms()) {
            let category = c.category.to_string();
            let qid = session::insert_question(
                &ctx.db,
                s.id,
                seg.ts_ms,
                &c.text,
                Some(&category),
                &c.keywords,
            )
            .unwrap_or(0);
            s.questions.push(c.text.clone());
            s.last_question_id = Some(qid);
            Some((qid, c.text, category, c.keywords))
        } else {
            None
        }
    };

    if let Some((qid, question, category, keywords)) = detected {
        if let Some(w) = ctx.widgets.borrow().as_ref() {
            ui::widget::set_question(w, &question, &category, &keywords);
            ui::widget::set_keywords(w, &keywords);
        }
        let ai_allowed = ctx
            .session
            .borrow()
            .as_ref()
            .map(|s| s.external_ai_ok)
            .unwrap_or(false);
        if ai_allowed && ai::build_provider(&settings).is_ok() {
            let context = {
                let session = ctx.session.borrow();
                let Some(s) = session.as_ref() else { return };
                let n = settings.ai.context_segments.min(s.transcript.len());
                ai::prompts::transcript_context(
                    &s.transcript[s.transcript.len().saturating_sub(n)..],
                    4000,
                )
            };
            spawn_suggestion(ctx, qid, question, context);
        } else {
            ctx.set_hint(
                "Pertanyaan terdeteksi. Saran jawaban butuh API key (Pengaturan → AI) \
                 dan izin pengiriman teks ke AI eksternal.",
            );
        }
    }
    ctx.refresh_status("");
}

fn handle_event(ctx: &Rc<AppCtx>, ev: AppEvent) {
    match ev {
        AppEvent::Segment {
            source,
            ts_ms,
            text,
        } => handle_segment_event(ctx, source, ts_ms, text),
        AppEvent::PracticeQuestion { text, from_ai } => {
            let category = ctx.settings.borrow().practice.category.clone();
            let new_qid = {
                let mut session = ctx.session.borrow_mut();
                let Some(s) = session.as_mut() else { return };
                let qid =
                    session::insert_question(&ctx.db, s.id, 0, &text, Some(&category), &[])
                        .unwrap_or(0);
                s.questions.push(text.clone());
                s.last_question_id = Some(qid);
                qid
            };
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                ui::widget::set_question(
                    w,
                    &text,
                    if from_ai { "ai-interviewer" } else { "offline-bank" },
                    &[],
                );
            }
            let _ = new_qid;
            ctx.refresh_status("Pertanyaan latihan — jawab dengan suara Anda.");
        }
        AppEvent::SuggestionReady {
            question_id,
            suggestion,
        } => {
            let payload = serde_json::to_string(&suggestion).unwrap_or_default();
            let _ = session::insert_suggestion(&ctx.db, question_id, "suggestion", &payload);
            if let Some(s) = ctx.session.borrow_mut().as_mut() {
                s.last_suggestion = Some(suggestion.clone());
            }
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                ui::widget::set_suggestion(w, &suggestion);
            }
            ctx.refresh_status("Saran jawaban siap — tinjau sebelum dipakai.");
        }
        AppEvent::AlternativeReady {
            question_id,
            suggestion,
        } => {
            let payload = serde_json::to_string(&suggestion).unwrap_or_default();
            let _ = session::insert_suggestion(&ctx.db, question_id, "alternative", &payload);
            if let Some(s) = ctx.session.borrow_mut().as_mut() {
                s.last_suggestion = Some(suggestion.clone());
            }
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                ui::widget::set_suggestion(w, &suggestion);
            }
            ctx.refresh_status("Alternatif jawaban siap.");
        }
        AppEvent::ExplanationReady(text) => {
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.answer_label.set_label(&format!("Penjelasan istilah:\n{text}"));
            }
            ctx.refresh_status("Penjelasan istilah siap.");
        }
        AppEvent::SummaryReady(text) => {
            if let Some(id) = *ctx.last_session_id.borrow() {
                let _ = session::end_session(&ctx.db, id, Some(&text));
            }
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.answer_label.set_label(&format!("Ringkasan sesi:\n{text}"));
            }
            ctx.refresh_status("Ringkasan sesi tersimpan di Riwayat.");
        }
        AppEvent::PracticeReportReady(report) => {
            let md = ai::render_report_markdown(&report);
            if let Some(id) = *ctx.last_session_id.borrow() {
                let _ = session::end_session(&ctx.db, id, Some(&md));
            }
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.answer_label.set_label(&md);
            }
            ctx.refresh_status("Laporan latihan siap (tersimpan di Riwayat).");
        }
        AppEvent::Error(msg) => {            let redacted = crate::privacy::redact_all(&msg);
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                ui::widget::set_status(w, WidgetStatus::Error, "");
                ui::widget::set_hint(w, &redacted);
            }
            self_update_snapshot_error(ctx, &redacted);
        }
        AppEvent::CaptureEnded { source, message } => {
            if ctx.session.borrow().is_none() {
                return;
            }
            let label = match source {
                AudioSourceKind::Mic => "Microphone",
                AudioSourceKind::Meeting => "Audio meeting",
            };
            let detail = message.unwrap_or_else(|| "stream berakhir".into());
            stop_session(ctx, Some(format!("{label} terputus: {detail}")));
        }
    }
}

fn self_update_snapshot_error(ctx: &Rc<AppCtx>, msg: &str) {
    if let Ok(mut slot) = ctx.status_snapshot.lock() {
        *slot = format!("status=error; {msg}");
    }
}

fn poll_events(ctx: &Rc<AppCtx>) {
    // 1. Capture & STT — hanya bila ada sesi.
    let mut stt_events: Vec<transcription::SttEvent> = Vec::new();
    let mut capture_ends: Vec<CaptureEvent> = Vec::new();
    {
        let mut session = ctx.session.borrow_mut();
        if let Some(s) = session.as_mut() {
            while let Ok(ev) = s.capture_rx.try_recv() {
                match ev {
                    CaptureEvent::Chunk(chunk) => {
                        if s.save_audio {
                            save_chunk_to_disk(s, &chunk);
                        }
                        let job = transcription::SttJob {
                            wav: chunk.wav,
                            source: chunk.source,
                            start_ms: chunk.start_ms,
                            seq: chunk.seq,
                        };
                        if let Some(worker) = s.stt.as_ref() {
                            let _ = worker.submit(job, &s.stt_event_tx);
                        }
                        if ctx.settings.borrow().audio.show_level_meter {
                            if let Some(w) = ctx.widgets.borrow().as_ref() {
                                ui::widget::set_level(w, chunk.level, true);
                            }
                        }
                    }
                    other => capture_ends.push(other),
                }
            }
            while let Ok(ev) = s.stt_rx.try_recv() {
                stt_events.push(ev);
            }
        }
    }

    for ev in capture_ends {
        match ev {
            CaptureEvent::Ended { source, message } => {
                handle_event(ctx, AppEvent::CaptureEnded { source, message });
            }
            CaptureEvent::Error(msg) => handle_event(ctx, AppEvent::Error(msg)),
            CaptureEvent::Chunk(_) => {}
        }
    }

    for ev in stt_events {
        match ev {
            transcription::SttEvent::Done {
                source,
                start_ms,
                text,
            } => handle_event(
                ctx,
                AppEvent::Segment {
                    source,
                    ts_ms: start_ms,
                    text,
                },
            ),
            transcription::SttEvent::Failed { message, .. } => {
                let redacted = crate::privacy::redact_all(&message);
                ctx.set_hint(&format!("Transkripsi gagal pada satu potongan: {redacted}"));
            }
            transcription::SttEvent::Dropped { count } => {
                ctx.set_hint(&format!(
                    "{count} potongan audio dibuang (STT lebih lambat dari audio masuk — \
                     pertimbangkan model lebih kecil atau CPU-friendly mode)."
                ));
            }
        }
    }

    // 2. Event AI.
    let mut ai_events = Vec::new();
    while let Ok(ev) = ctx.ai_rx.borrow().try_recv() {
        ai_events.push(ev);
    }
    for ev in ai_events {
        handle_event(ctx, ev);
    }

    // 3. Perintah IPC.
    let mut cmds = Vec::new();
    if let Some(rx) = ctx.ipc_rx.borrow().as_ref() {
        while let Ok(cmd) = rx.try_recv() {
            cmds.push(cmd);
        }
    }
    for cmd in cmds {
        handle_ipc(ctx, cmd);
    }

    update_timer(ctx);
}

fn update_timer(ctx: &Rc<AppCtx>) {
    let session = ctx.session.borrow();
    let Some(s) = session.as_ref() else { return };
    let settings = ctx.settings.borrow();
    let now = now_ms();
    let paused_now = s.paused_at.map(|p| now - p).unwrap_or(0);
    let elapsed = now - s.started_ms - s.paused_total_ms - paused_now;
    let secs = (elapsed.max(0) / 1000) as u64;
    let text = format!("{:02}:{:02}", (secs / 60) % 100, secs % 60);
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        ui::widget::set_recording(
            w,
            true,
            s.paused_at.is_some(),
            &text,
            settings.privacy.show_recording_timer,
        );
    }
}

fn save_chunk_to_disk(session: &RunningSession, chunk: &crate::audio::capture::AudioChunk) {
    let dir = paths::recordings_dir().join(format!("session-{}", session.id));
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let name = format!(
        "{:05}-{}.wav",
        chunk.seq,
        match chunk.source {
            AudioSourceKind::Mic => "mic",
            AudioSourceKind::Meeting => "meeting",
        }
    );
    let _ = std::fs::write(dir.join(name), &chunk.wav);
}

fn handle_ipc(ctx: &Rc<AppCtx>, cmd: IpcCommand) {
    match cmd {
        IpcCommand::Toggle => {
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                let visible = w.window.is_visible();
                w.window.set_visible(!visible);
            }
        }
        IpcCommand::Show => {
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.window.set_visible(true);
            }
        }
        IpcCommand::Hide => {
            if let Some(w) = ctx.widgets.borrow().as_ref() {
                w.window.set_visible(false);
            }
        }
        IpcCommand::Start => begin_start_flow(ctx),
        IpcCommand::Pause => {
            if !is_paused(ctx) {
                toggle_pause(ctx);
            }
        }
        IpcCommand::Resume => {
            if is_paused(ctx) {
                toggle_pause(ctx);
            }
        }
        IpcCommand::Stop => stop_session(ctx, Some("dihentikan via shortcut".into())),
        IpcCommand::Expand | IpcCommand::Collapse => toggle_expand(ctx),
        IpcCommand::AltAnswer => request_alternative(ctx),
        IpcCommand::Status => {}
        IpcCommand::Quit => {
            if ctx.session.borrow().is_some() {
                stop_session(ctx, Some("aplikasi keluar".into()));
            }
            if let Some(app) = ctx
                .widgets
                .borrow()
                .as_ref()
                .and_then(|w| w.window.application())
            {
                app.quit();
            }
        }
    }
}

fn is_paused(ctx: &Rc<AppCtx>) -> bool {
    ctx.session
        .borrow()
        .as_ref()
        .map(|s| s.paused_at.is_some())
        .unwrap_or(false)
}

fn toggle_expand(ctx: &Rc<AppCtx>) {
    if let Some(w) = ctx.widgets.borrow().as_ref() {
        let expanded = w.revealer.reveals_child();
        w.revealer.set_reveal_child(!expanded);
        w.btn_expand.set_label(if expanded { "Detail ▾" } else { "Tutup ▴" });
    }
}

fn shutdown(ctx: &Rc<AppCtx>) {
    if ctx.session.borrow().is_some() {
        stop_session(ctx, Some("aplikasi ditutup".into()));
    }
    if let Some(server) = ctx.ipc_server.borrow_mut().take() {
        server.shutdown();
    }
}
