//! The Import Video dialog and the progress of the reading. The dialog says, as each choice
//! changes, what it makes: how many frames and how much of the video, how long the animation
//! runs at the document's rate and whether that keeps the video's timing, the memory the pictures
//! take of the room the document has, and, for a video too large, a length and a size that fit,
//! each a click away. Import stays dimmed until the choices fit.

use flare_core::FrameRate;
use flare_tools::video::{MAX_VIDEO_SECONDS, MAX_VIDEO_SIDE, MIN_VIDEO_SIDE, Plan, TooLarge};
use flare_ui::dialog;

use super::video_import::{VideoForm, VideoImport};
use super::*;

/// Room for the labels, the number fields and the buttons beside them.
const DIALOG_WIDTH: f32 = 460.0;
/// The slowest and fastest rate the dialog takes pictures at.
const RATES: std::ops::RangeInclusive<f64> = 1.0..=120.0;

impl Workspace {
    /// The Import Video dialog, or the progress of the reading, while either is open.
    pub(super) fn video_import_window(&mut self, ctx: &egui::Context) {
        match &self.video_import {
            VideoImport::Asking(_) => self.video_dialog(ctx),
            VideoImport::Reading { .. } => self.video_progress_window(ctx),
            VideoImport::Idle | VideoImport::Probing { .. } => {}
        }
    }

    fn video_dialog(&mut self, ctx: &egui::Context) {
        let VideoImport::Asking(mut form) = std::mem::take(&mut self.video_import) else {
            return;
        };
        let document_rate = self.session.document().frame_rate();
        let plan = form.plan(self.session.document());
        let texts = DialogText::new(self, &form, document_rate, plan.as_ref());
        let (mut confirm, mut cancelled) = (false, false);
        let modal = dialog::modal(ctx, egui::Id::unique("video_import_dialog")).show(ctx, |ui| {
            ui.set_width(DIALOG_WIDTH);
            dialog::title(ui, &texts.title);
            dialog::field(ui, &texts.file, |ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&form.name).strong());
                    ui.label(egui::RichText::new(&texts.about).color(flare_ui::theme::text_dim()));
                });
            });
            ui.add_space(6.0);
            video_choices(ui, &mut form, document_rate, &texts);
            ui.add_space(10.0);
            if let Some(plan) = &plan {
                video_summary(ui, &mut form, plan, &texts);
            }
            ui.add_space(8.0);
            dialog::message(ui, &texts.note);
            let fits = plan.as_ref().is_some_and(Plan::fits);
            dialog::footer(
                ui,
                |_| {},
                |ui| {
                    if dialog::primary_button(ui, &texts.import, fits).clicked() {
                        confirm = true;
                    }
                    if dialog::button(ui, &texts.cancel).clicked() {
                        cancelled = true;
                    }
                },
            );
            if fits && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                confirm = true;
            }
        });
        if modal.should_close() || cancelled {
            return;
        }
        self.video_import = VideoImport::Asking(form);
        if confirm {
            self.confirm_video_import();
        }
    }

    fn video_progress_window(&mut self, ctx: &egui::Context) {
        let VideoImport::Reading {
            name,
            total,
            progress,
            ..
        } = &self.video_import
        else {
            return;
        };
        let (read, total) = (progress.read(), (*total).max(1));
        let title = tr!(self, "video-reading-title");
        let line = tr!(
            self,
            "video-reading",
            &[
                ("name", name),
                ("read", &read.to_string()),
                ("total", &total.to_string())
            ]
        );
        let stop = tr!(self, "start-cancel");
        let stopping = progress.is_stopped();
        let mut stop_clicked = false;
        let modal = dialog::modal(ctx, egui::Id::unique("video_import_progress")).show(ctx, |ui| {
            ui.set_width(360.0);
            dialog::title(ui, &title);
            dialog::message(ui, &line);
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(read as f32 / total as f32).show_percentage());
            dialog::footer(
                ui,
                |_| {},
                |ui| {
                    if ui
                        .add_enabled_ui(!stopping, |ui| dialog::button(ui, &stop))
                        .inner
                        .clicked()
                    {
                        stop_clicked = true;
                    }
                },
            );
        });
        if stop_clicked || modal.should_close() {
            self.stop_video_import();
        }
        // The worker wakes the window only when it is done; the bar moves meanwhile.
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// The rate, the length, the size and the opacity.
fn video_choices(
    ui: &mut egui::Ui,
    form: &mut VideoForm,
    document_rate: FrameRate,
    texts: &DialogText,
) {
    dialog::field(ui, &texts.rate, |ui| {
        let mut fps = form.settings.rate.per_second();
        let field = egui::DragValue::new(&mut fps)
            .range(RATES)
            .speed(0.1)
            .max_decimals(3)
            .suffix(" fps");
        if ui.add(field).on_hover_text(&texts.rate_tip).changed()
            && let Ok(rate) = FrameRate::from_per_second(fps)
        {
            form.settings.rate = rate;
        }
        if ui.small_button(&texts.rate_document).clicked() {
            form.settings.rate = document_rate;
        }
        let video = form.source.rate;
        if RATES.contains(&video.per_second()) && ui.small_button(&texts.rate_video).clicked() {
            form.settings.rate = video;
        }
    });
    dialog::field(ui, &texts.most, |ui| {
        let mut minutes = form.settings.max_seconds / 60.0;
        let field = egui::DragValue::new(&mut minutes)
            .range(0.05..=MAX_VIDEO_SECONDS / 60.0)
            .speed(0.05)
            .max_decimals(2)
            .suffix(format!(" {}", texts.minutes));
        if ui.add(field).on_hover_text(&texts.most_tip).changed() {
            form.settings.max_seconds = (minutes * 60.0).clamp(1.0, MAX_VIDEO_SECONDS);
        }
    });
    dialog::field(ui, &texts.size, |ui| {
        let field = egui::DragValue::new(&mut form.settings.max_side)
            .range(MIN_VIDEO_SIDE..=MAX_VIDEO_SIDE)
            .speed(4.0)
            .suffix(" px");
        ui.add(field).on_hover_text(&texts.size_tip);
        ui.label(egui::RichText::new(&texts.size_result).color(flare_ui::theme::text_dim()));
    });
    dialog::field(ui, &texts.opacity, |ui| {
        ui.add(egui::Slider::new(&mut form.settings.opacity, 0..=100).suffix("%"))
            .on_hover_text(&texts.opacity_tip);
    });
}

/// What the plan makes, said in sentences, with the ways out when it does not fit.
fn video_summary(ui: &mut egui::Ui, form: &mut VideoForm, plan: &Plan, texts: &DialogText) {
    egui::Frame::new()
        .fill(flare_ui::theme::field())
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            for line in [&texts.takes, &texts.length, &texts.timing, &texts.memory] {
                ui.label(line);
            }
            if let Some(problem) = &texts.problem {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(problem).color(flare_ui::theme::danger()));
                ui.horizontal(|ui| {
                    if let Some(text) = &texts.fit_length
                        && ui.button(text).clicked()
                    {
                        form.settings.max_seconds = plan.fitting_seconds.floor().max(1.0);
                    }
                    if let (Some(text), Some(side)) = (&texts.fit_size, plan.fitting_side)
                        && ui.button(text).clicked()
                    {
                        form.settings.max_side = side;
                    }
                });
            } else if plan.is_large() {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&texts.large).color(flare_ui::theme::warning()));
            }
        });
}

/// Every word of the dialog, in the language spoken, made before it is drawn.
struct DialogText {
    title: String,
    file: String,
    about: String,
    rate: String,
    rate_tip: String,
    rate_document: String,
    rate_video: String,
    most: String,
    most_tip: String,
    minutes: String,
    size: String,
    size_tip: String,
    size_result: String,
    opacity: String,
    opacity_tip: String,
    takes: String,
    length: String,
    timing: String,
    memory: String,
    problem: Option<String>,
    fit_length: Option<String>,
    fit_size: Option<String>,
    large: String,
    note: String,
    import: String,
    cancel: String,
}

impl DialogText {
    fn new(
        ws: &Workspace,
        form: &VideoForm,
        document_rate: FrameRate,
        plan: Option<&Plan>,
    ) -> Self {
        let source = &form.source;
        let fps_doc = fps_text(document_rate.per_second());
        let about = match source.seconds {
            Some(seconds) => tr!(
                ws,
                "video-about",
                &[
                    ("width", &source.width.to_string()),
                    ("height", &source.height.to_string()),
                    ("fps", &fps_text(source.rate.per_second())),
                    ("length", &clock(seconds)),
                ]
            ),
            None => tr!(
                ws,
                "video-about-unknown",
                &[
                    ("width", &source.width.to_string()),
                    ("height", &source.height.to_string()),
                    ("fps", &fps_text(source.rate.per_second())),
                ]
            ),
        };
        let (mut takes, mut length, mut timing, mut memory, mut size_result) = (
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
        let (mut problem, mut fit_length, mut fit_size) = (None, None, None);
        if let Some(plan) = plan {
            let frames = plan.frames;
            takes = match (plan.whole, source.seconds) {
                (true, _) => tr_n!(ws, "video-takes-whole", frames),
                (false, Some(seconds)) => tr_n!(
                    ws,
                    "video-takes-part",
                    frames,
                    &[
                        ("covered", &clock(plan.seconds)),
                        ("length", &clock(seconds))
                    ]
                ),
                (false, None) => tr_n!(ws, "video-takes-unknown", frames),
            };
            length = tr_n!(
                ws,
                "video-length",
                frames,
                &[("length", &clock(plan.length_seconds)), ("fps", &fps_doc)]
            );
            timing = if plan.keeps_timing() {
                tr!(ws, "video-timing-same")
            } else {
                let percent = format!("{:.0}", plan.speed * 100.0);
                tr!(
                    ws,
                    "video-timing-speed",
                    &[("percent", &percent), ("fps", &fps_doc)]
                )
            };
            let room = match plan.too_large {
                Some(TooLarge::Bytes { room, .. }) => room,
                _ => plan.bytes + bytes_left(ws, plan),
            };
            memory = tr!(
                ws,
                "video-memory",
                &[
                    ("mb", &flare_tools::video::megabytes(plan.bytes).to_string()),
                    ("room", &flare_tools::video::megabytes(room).to_string()),
                ]
            );
            size_result = tr!(
                ws,
                "video-size-result",
                &[
                    ("width", &plan.size.0.to_string()),
                    ("height", &plan.size.1.to_string())
                ]
            );
            if let Some(why) = plan.too_large {
                problem = Some(match why {
                    TooLarge::Frames { .. } => tr!(ws, "video-too-many"),
                    TooLarge::Bytes { .. } => tr!(ws, "video-too-large"),
                });
                if plan.fitting_seconds >= 1.0 {
                    let seconds = plan.fitting_seconds.floor();
                    fit_length = Some(tr!(ws, "video-fit-length", &[("length", &clock(seconds))]));
                }
                fit_size = plan
                    .fitting_side
                    .map(|side| tr!(ws, "video-fit-size", &[("side", &side.to_string())]));
            }
        }
        Self {
            title: tr!(ws, "video-title"),
            file: tr!(ws, "video-file"),
            about,
            rate: tr!(ws, "video-rate"),
            rate_tip: tr!(ws, "video-rate-tip"),
            rate_document: tr!(ws, "video-rate-document", &[("fps", &fps_doc)]),
            rate_video: tr!(
                ws,
                "video-rate-video",
                &[("fps", &fps_text(source.rate.per_second()))]
            ),
            most: tr!(ws, "video-most"),
            most_tip: tr!(ws, "video-most-tip"),
            minutes: tr!(ws, "video-minutes"),
            size: tr!(ws, "video-size"),
            size_tip: tr!(ws, "video-size-tip"),
            size_result,
            opacity: tr!(ws, "video-opacity"),
            opacity_tip: tr!(ws, "video-opacity-tip"),
            takes,
            length,
            timing,
            memory,
            problem,
            fit_length,
            fit_size,
            large: tr!(ws, "video-large"),
            note: tr!(ws, "video-note"),
            import: tr!(ws, "video-import"),
            cancel: tr!(ws, "start-cancel"),
        }
    }
}

/// The room left for pictures once these are in, so "of the room" reads the same either way.
fn bytes_left(ws: &Workspace, plan: &Plan) -> u64 {
    let held: u64 = ws
        .session
        .document()
        .bitmaps()
        .iter()
        .map(flare_document::Bitmap::byte_len)
        .sum();
    flare_document::MAX_TOTAL_BITMAP_BYTES
        .saturating_sub(held)
        .saturating_sub(plan.bytes)
}

/// A rate as a person types it: 24, 29.97, 12.5.
pub(super) fn fps_text(fps: f64) -> String {
    let text = format!("{fps:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A length as a clock shows it: 0:05, 1:32, 1:02:07; tenths for a short one that has them.
pub(super) fn clock(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    if seconds < 10.0 && (seconds * 10.0).round() % 10.0 != 0.0 {
        return format!("0:{:04.1}", (seconds * 10.0).round() / 10.0);
    }
    let whole = seconds.round() as u64;
    let (h, m, s) = (whole / 3600, whole / 60 % 60, whole % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
