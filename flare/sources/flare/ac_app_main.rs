// Release builds on Windows are GUI apps: no console window behind the editor.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! Flare desktop editor. Standard layout: menu bar, Tools (left), Stage
//! (centre), Properties + Library (right), Timeline (bottom); each dock can be
//! moved by its grip (dock.rs, Workspace menu).

#[path = "ac_app/color_panel.rs"]
mod color_panel;
#[path = "ac_app/dock.rs"]
mod dock;
#[path = "ac_app/files.rs"]
mod files;
#[path = "ac_app/history.rs"]
mod history;
#[path = "ac_app/import.rs"]
mod import;
#[path = "ac_app/motion_ui.rs"]
mod motion_ui;
#[path = "ac_app/navigator.rs"]
mod navigator;
#[path = "ac_app/options_bar.rs"]
mod options_bar;
#[path = "ac_app/palette.rs"]
mod palette;
#[path = "ac_app/prefs.rs"]
mod prefs;
#[path = "ac_app/presets.rs"]
mod presets;
#[path = "ac_app/panels.rs"]
mod panels;
#[path = "ac_app/publish.rs"]
mod publish;
#[path = "ac_app/rulers.rs"]
mod rulers;
#[path = "ac_app/scripting.rs"]
mod scripting;
#[path = "ac_app/startup.rs"]
mod startup;
#[path = "ac_app/swatches.rs"]
mod swatches;
#[path = "ac_app/stage.rs"]
mod stage;
#[path = "ac_app/timeline.rs"]
mod timeline;
#[path = "ac_app/toolbar.rs"]
mod toolbar;
#[path = "ac_app/windows.rs"]
mod windows;

use toolbar::Tool;

use ac_core::{demo::demo_document, Color, Editor, ElementKind, Matrix, Pt, SymbolId, SymbolKind};
use eframe::egui::{self, vec2, Align2, Color32, Key, Modifiers, Rect, RichText, Stroke as EStroke, Vec2};

pub(crate) const ACCENT: Color32 = Color32::from_rgb(0x2D, 0x8C, 0xEB);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PropsTab {
    Tool,
    Object,
    Frame,
    Doc,
}

pub(crate) enum Dialog {
    None,
    Path { title: String, path: String, action: FileAction },
    Symbol { convert: bool, name: String, kind: SymbolKind, reg: [u8; 2], folder: String },
    RenameSymbol { id: SymbolId, name: String },
    DeleteSymbol { id: SymbolId, uses: usize },
    RenameBitmap { id: u32, name: String },
    DeleteBitmap { id: u32, uses: usize },
    Unsaved(files::Pending),
    Recover { original: Option<String>, age_secs: u64 },
    ScaleRotate { scale: f32, rotate: f32 },
    TraceBitmap { threshold: u32, min_area: u32, fit: usize, corners: usize, centerline: bool, ink: u8, uniform: bool },
    LayerProps { i: usize, name: String, kind: ac_core::LayerKind, parent: Option<usize>, color: Color32, visible: bool, locked: bool, outline: bool, height: u16, filters: Vec<ac_core::Filter> },
    About,
}

pub(crate) use files::FileAction;

pub struct App {
    pub(crate) ctx: egui::Context,
    pub(crate) ed: Editor,
    pub(crate) tool: Tool,
    pub(crate) fill: Color32,
    pub(crate) fill_on: bool,
    pub(crate) stroke: Color32,
    pub(crate) stroke_on: bool,
    pub(crate) stroke_w: f32,
    pub(crate) zoom: f32,
    pub(crate) pan: Vec2,
    pub(crate) fit_pending: bool,
    /// The box the next fit shows (None = the stage).
    pub(crate) fit_bounds: Option<(ac_core::Pt, ac_core::Pt)>,
    pub(crate) playing: bool,
    /// Sound while playing (None = silent / not started yet).
    #[cfg(feature = "audio")]
    pub(crate) audio: Option<ac_audio::playback::Player>,
    pub(crate) audio_started: bool,
    /// A frame script stopped the main timeline; movie clips keep playing.
    pub(crate) halted: bool,
    pub(crate) loop_play: bool,
    play_accum: f64,
    pub(crate) onion: bool,
    pub(crate) onion_opts: timeline::OnionOpts,
    pub(crate) stage_tex: Option<egui::TextureHandle>,
    pub(crate) lib_tex: Option<egui::TextureHandle>,
    pub(crate) lib_selected: Option<SymbolId>,
    /// Library item being dragged towards the stage.
    pub(crate) lib_drag: Option<SymbolId>,
    pub(crate) stage_rect: Rect,
    pub(crate) drag: stage::Drag,
    pub(crate) prop_dragging: bool,
    pub(crate) dialog: Dialog,
    /// The Startup window (new scene / open existing), when shown.
    pub(crate) startup: Option<startup::Startup>,
    /// Name given in the Startup window to a document not saved yet.
    pub(crate) untitled_name: Option<String>,
    /// Where the docks sit (dock.rs); saved in the settings.
    pub(crate) layout: dock::Layout,
    /// The module being dragged by its grip.
    pub(crate) dock_drag: Option<dock::Module>,
    /// Escape cancelled a dock drag; ignored until the mouse button is released.
    pub(crate) dock_cancelled: bool,
    pub(crate) swatch_ui: swatches::State,
    pub(crate) nav: navigator::Nav,
    pub(crate) palette: palette::Palette,
    /// Window › History (Ctrl+F10).
    pub(crate) history_open: bool,
    /// Modify › Shape › Expand Fill / Soften Fill Edges dialog.
    pub(crate) shape_dlg: Option<windows::ShapeDlg>,
    /// Modify › Document… dialog.
    pub(crate) doc_dlg: Option<windows::DocDlg>,
    /// Edit › Preferences.
    pub(crate) prefs_open: bool,
    pub(crate) prefs_page: prefs::Page,
    /// Window › Color.
    pub(crate) colors: color_panel::ColorPanel,
    pub(crate) status: String,
    pub(crate) file_path: Option<String>,
    pub(crate) settings: files::Settings,
    /// Name typed in Filters › Presets › Save As.
    pub(crate) preset_name: String,
    /// Edit in place levels (innermost last).
    pub(crate) place: Vec<crate::stage::InPlace>,
    /// Free camera: look through it while authoring.
    pub(crate) vcam_preview: bool,
    /// Free camera: dim the scene outside the camera frame.
    pub(crate) vcam_dim: bool,
    /// Search text in Add filter › EffectCraft effects.
    pub(crate) effect_search: String,
    /// Editor revision at the last save/open (dirty = different).
    pub(crate) saved_rev: u64,
    pub(crate) autosave_rev: u64,
    pub(crate) dirty_since: Option<f64>,
    pub(crate) quit_confirmed: bool,
    /// Run after the next successful save (Save in the unsaved-changes prompt).
    pub(crate) after_save: Option<files::Pending>,
    title_shown: String,
    pub(crate) tl: timeline::TimelineState,
    pub(crate) topts: toolbar::ToolOpts,
    pub(crate) pen: Option<stage::PenState>,
    pub(crate) poly_lasso: Vec<Pt>,
    /// Bender tool: the segment drawn across the lines (stage coordinates).
    pub(crate) bender_seg: Option<(Pt, Pt)>,
    /// Assistants tool: the selected assistant.
    pub(crate) assist_sel: Option<usize>,
    /// Set while committing one already-assisted stroke (no second pass).
    pub(crate) assist_bypass: bool,
    /// Ctrl/Cmd held during the current stage press (Ctrl-drag adds a corner).
    pub(crate) cmd_down: bool,
    /// Copied variable-width profile (Width tool ▸ Copy width profile).
    pub(crate) width_clip: Option<Vec<(f32, f32)>>,
    pub(crate) text_edit: Option<stage::TextEdit>,
    pub(crate) snap_guides: (Option<f32>, Option<f32>),
    pub(crate) props_tab: PropsTab,
    pub(crate) scripting: scripting::ScriptingState,
    /// View rotation (Rotation tool), radians.
    pub(crate) view_rot: f32,
    /// Free Transform's transformation point for a given selection.
    pub(crate) ft_pivot: Option<(Vec<ac_core::Selection>, Pt)>,
    pub(crate) transform_open: bool,
    pub(crate) xf: panels::TransformPanel,
    pub(crate) lib: panels::LibraryState,
    pub(crate) publish: publish::PublishSettings,
    pub(crate) font_search: String,
    pub(crate) view: rulers::ViewOpts,
    pub(crate) align_open: bool,
    pub(crate) align_to_stage: bool,
    pub(crate) info_open: bool,
    pub(crate) motion: motion_ui::MotionUi,
    pub(crate) presets: presets::PresetsPanel,
    /// Control ▸ Enable Simple Buttons: buttons react to the pointer on the stage.
    pub(crate) simple_buttons: bool,
    pub(crate) scene_open: bool,
    /// Bind tool: the shape bone being bound to.
    pub(crate) bind_bone: Option<(ac_core::Selection, usize)>,
    pub(crate) scene_rename: Option<(usize, String)>,
    pub(crate) info_cache: Option<((u64, u32), tiny_skia::Pixmap)>,
    last_sel: (Vec<ac_core::Selection>, Option<ac_core::FrameSel>),
    screenshot: Option<(String, u32)>,
}

pub(crate) fn c32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}
pub(crate) fn from32(c: Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color { r, g, b, a }
}

/// Layer Properties › OK: (layer, name, type, inside, colour, show, lock, outline, height %, effects).
type LayerPropsOk = (usize, String, ac_core::LayerKind, Option<usize>, Color32, bool, bool, bool, u16, Vec<ac_core::Filter>);

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        // timeline semantics for this (the UI) thread, from the saved setting
        ac_core::frames::set_retain_tween_curves(files::Settings::load().retain_tween_curves);
        // installed fonts as fallbacks for characters the UI font lacks
        // (CJK names, the full-width ＂ Windows puts in file names, symbols)
        let fallbacks = ac_render::fonts::ui_fallback_fonts();
        if !fallbacks.is_empty() {
            let mut fonts = egui::FontDefinitions::default();
            for (name, bytes, index) in fallbacks {
                let mut fd = egui::FontData::from_owned(bytes);
                fd.index = index;
                fonts.font_data.insert(name.clone(), std::sync::Arc::new(fd));
                for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts.families.entry(fam).or_default().push(name.clone());
                }
            }
            cc.egui_ctx.set_fonts(fonts);
        }
        let mut v = egui::Visuals::dark();
        v.panel_fill = Color32::from_gray(0x32);
        v.window_fill = Color32::from_gray(0x2B);
        v.extreme_bg_color = Color32::from_gray(0x1E);
        v.selection.bg_fill = ACCENT;
        v.widgets.noninteractive.bg_stroke = EStroke::new(1.0_f32, Color32::from_gray(0x24));
        // Dark regardless of the OS setting: egui would otherwise switch to its light
        // style on a light-mode Windows desktop, half-overriding our panel colours.
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        cc.egui_ctx.set_visuals_of(egui::Theme::Dark, v);
        cc.egui_ctx.all_styles_mut(|s| s.spacing.item_spacing = vec2(6.0, 5.0));

        ac_render::fonts::install_system_fonts();
        ac_audio::register();
        let mut app = App {
            ctx: cc.egui_ctx.clone(),
            ed: Editor::new(demo_document()),
            tool: Tool::Select,
            fill: Color32::from_rgb(0x2D, 0x8C, 0xEB),
            fill_on: true,
            stroke: Color32::from_rgb(0x10, 0x10, 0x10),
            stroke_on: true,
            stroke_w: 2.0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            fit_bounds: None,
            playing: false,
            #[cfg(feature = "audio")]
            audio: None,
            audio_started: false,
            halted: false,
            loop_play: true,
            play_accum: 0.0,
            onion: false,
            onion_opts: Default::default(),
            stage_tex: None,
            lib_tex: None,
            lib_selected: None,
            lib_drag: None,
            stage_rect: Rect::NOTHING,
            drag: stage::Drag::None,
            prop_dragging: false,
            dialog: Dialog::None,
            status: "Demo loaded. Enter = play · click frames to select · Delete removes the selection · right-click for menus".into(),
            file_path: None,
            settings: files::Settings::load(),
            preset_name: String::new(),
            place: vec![],
            vcam_preview: false,
            vcam_dim: true,
            effect_search: String::new(),
            saved_rev: 0,
            autosave_rev: 0,
            dirty_since: None,
            quit_confirmed: false,
            after_save: None,
            title_shown: String::new(),
            tl: Default::default(),
            topts: Default::default(),
            pen: None,
            poly_lasso: vec![],
            bender_seg: None,
            assist_sel: None,
            assist_bypass: false,
            cmd_down: false,
            width_clip: None,
            text_edit: None,
            snap_guides: (None, None),
            props_tab: PropsTab::Frame,
            scripting: Default::default(),
            view_rot: 0.0,
            ft_pivot: None,
            transform_open: false,
            xf: Default::default(),
            lib: Default::default(),
            publish: Default::default(),
            font_search: String::new(),
            view: Default::default(),
            align_open: false,
            align_to_stage: false,
            info_open: false,
            motion: Default::default(),
            presets: Default::default(),
            simple_buttons: false,
            scene_open: false,
            bind_bone: None,
            scene_rename: None,
            info_cache: None,
            last_sel: (vec![], None),
            screenshot: None,
            startup: None,
            untitled_name: None,
            layout: dock::Layout::default(),
            dock_drag: None,
            dock_cancelled: false,
            swatch_ui: swatches::State::default(),
            nav: navigator::Nav::default(),
            palette: palette::Palette::default(),
            history_open: false,
            shape_dlg: None,
            doc_dlg: None,
            prefs_open: false,
            prefs_page: prefs::Page::General,
            colors: color_panel::ColorPanel::default(),
        };
        app.swatch_ui.open = app.settings.swatches_open;
        app.tl.cell_w = app.settings.tl_cell_w;
        app.tl.seconds = app.settings.tl_seconds;
        app.ed.auto_keyframe = app.settings.auto_keyframe;
        prefs::apply(&mut app, &cc.egui_ctx);
        app.topts.object_draw = app.settings.prefs.object_drawing;
        app.topts.text_size = app.settings.prefs.text_size;
        app.scripting.frame_scripts = app.settings.prefs.frame_scripts;
        app.layout = app.settings.layout.unwrap_or(dock::Layout { library: if app.settings.library_left { dock::LibraryDock::Left } else { dock::LibraryDock::WithProperties }, ..Default::default() });
        // `flare file.acx.json` opens that document
        let arg = std::env::args().nth(1).filter(|a| !a.starts_with('-'));
        if let Some(path) = &arg {
            app.do_file(files::FileAction::Open, path);
        }
        if std::env::var_os("AC_SCREENSHOT").is_none() && arg.is_none() {
            app.check_recovery();
        }
        // Startup window (Flare's): new scene or open one; a blank document behind it
        if startup::at_launch(&app, arg.is_some()) && matches!(app.dialog, Dialog::None) {
            app.load_doc(ac_core::Document::default(), None);
            app.startup = Some(startup::Startup::new(&app));
            app.status = "Create a new scene or open one (Window › Startup Window… shows this again).".into();
        }
        // Headless screenshot hook used by CI/docs: AC_SCREENSHOT=out.png [AC_SCREENSHOT_FRAME=n]
        if let Ok(p) = std::env::var("AC_SCREENSHOT") {
            let frame: u32 = std::env::var("AC_SCREENSHOT_FRAME").ok().and_then(|s| s.parse().ok()).unwrap_or(14);
            app.ed.set_frame(frame.saturating_sub(1));
            app.onion = true;
            app.ed.selection = vec![ac_core::Selection { layer: 0, element: 0 }];
            app.lib_selected = Some(1);
            app.screenshot = Some((p, 0));
        }
        app
    }

    // ------------------------------------------------------------ actions
    /// Keep the dock layout in the settings (and the old Library flag for
    /// older versions reading the same file).
    pub(crate) fn save_layout(&mut self) {
        self.settings.layout = Some(self.layout);
        self.settings.library_left = self.layout.library == dock::LibraryDock::Left;
        self.settings.save();
    }

    pub(crate) fn toggle_swatches(&mut self) {
        self.swatch_ui.open = !self.swatch_ui.open;
        self.settings.swatches_open = self.swatch_ui.open;
        self.settings.save();
    }

    pub(crate) fn toggle_options_bar(&mut self) {
        self.settings.options_bar = !self.settings.options_bar;
        self.settings.save();
    }

    pub(crate) fn reset_layout(&mut self) {
        self.layout = dock::Layout::default();
        self.save_layout();
        self.status = "Layout reset: Tools left, Properties and Library right, Timeline below".into();
    }

    pub(crate) fn set_tool(&mut self, t: Tool) {
        if !t.usable() {
            self.status = format!("{} is planned: {}", t.info().name, t.info().planned.unwrap_or(""));
            return;
        }
        if t != self.tool {
            if self.pen.is_some() {
                self.finish_pen(false);
            }
            if self.text_edit.is_some() && t != Tool::Text {
                self.finish_text();
            }
            self.poly_lasso.clear();
            self.bender_seg = None;
        }
        self.tool = t;
        // remember the tool in its toolbar slot
        let mut idx = 0;
        for section in toolbar::SECTIONS {
            for slot in section.iter() {
                if slot.contains(&t) {
                    self.topts.slot_memory[idx] = t;
                }
                idx += 1;
            }
        }
        self.props_tab = PropsTab::Tool;
    }

    /// Start the movie's sound with playback, stop it when playback stops.
    fn audio_tick(&mut self) {
        if !self.playing {
            self.audio_started = false;
            #[cfg(feature = "audio")]
            {
                self.audio = None;
            }
            return;
        }
        if self.audio_started {
            return;
        }
        self.audio_started = true;
        #[cfg(feature = "audio")]
        if ac_audio::has_sound(&self.ed.doc) && self.ed.editing.is_none() {
            let rate = ac_audio::playback::output_rate().unwrap_or(44100);
            let from = self.ed.doc.scene_start(self.ed.scene) + self.ed.frame;
            let samples = std::sync::Arc::new(ac_audio::mix(&self.ed.doc, from, rate));
            match ac_audio::playback::Player::start(samples, rate) {
                Ok(p) => self.audio = Some(p),
                Err(e) => self.status = format!("Sound off: {e}"),
            }
        }
    }
    pub(crate) fn restart_audio(&mut self) {
        self.audio_started = false;
        #[cfg(feature = "audio")]
        {
            self.audio = None;
        }
    }

    /// Go to the next (or previous) keyframe on any visible layer.
    pub(crate) fn step_keyframe(&mut self, forward: bool) {
        let cur = self.ed.frame;
        let keys: std::collections::BTreeSet<u32> = self.ed.timeline().layers.iter().filter(|l| l.visible).flat_map(|l| l.keyframes.keys().copied().collect::<Vec<_>>()).collect();
        let to = if forward { keys.range(cur + 1..).next().copied() } else { keys.range(..cur).next_back().copied() };
        if let Some(f) = to {
            self.step(f as i64 - cur as i64);
        }
    }

    /// Control › Loop Range from Selection (Ctrl+Alt+L): the selected frames, or
    /// the current keyframe span; turns looping on. Again with the same range clears it.
    pub(crate) fn set_loop_range(&mut self) {
        let r = match self.ed.frame_sel {
            Some(s) if s.f0 != s.f1 => (s.f0.min(s.f1), s.f0.max(s.f1)),
            _ => {
                let l = self.ed.timeline().layers.get(self.ed.layer);
                match l.and_then(|l| l.span_at(self.ed.frame).map(|(st, _, nx)| (st, nx.unwrap_or(l.end).min(l.end).max(st + 1) - 1))) {
                    Some(r) => r,
                    None => (0, self.ed.frame_count().saturating_sub(1)),
                }
            }
        };
        if self.tl.loop_range == Some(r) && self.loop_play {
            self.tl.loop_range = None;
            self.status = "Loop range cleared".into();
        } else {
            self.tl.loop_range = Some(r);
            self.loop_play = true;
            self.status = format!("Looping frames {}–{}", r.0 + 1, r.1 + 1);
        }
    }

    pub(crate) fn toggle_play(&mut self) {
        self.playing = !self.playing;
        self.play_accum = 0.0;
        self.halted = false;
        self.ed.clock = None;
        if self.playing {
            self.ed.selection.clear();
            match self.tl.loop_range.filter(|_| self.loop_play) {
                // a loop range starts playback at its first frame when the playhead is outside it
                Some((a, b)) if self.ed.frame < a || self.ed.frame > b => self.ed.set_frame(a),
                _ if self.ed.frame + 1 >= self.ed.frame_count() => self.ed.set_frame(0),
                _ => {}
            }
            scripting::start_playback(self);
        }
    }

    pub(crate) fn step(&mut self, delta: i64) {
        self.playing = false;
        let n = self.ed.frame_count() as i64;
        let f = (self.ed.frame as i64 + delta).clamp(0, (n - 1).max(0)) as u32;
        self.ed.set_frame(f);
        self.ed.frame_sel = None;
    }

    /// Delete key: objects first, otherwise the selected frames.
    pub(crate) fn delete(&mut self) {
        if !self.ed.selection.is_empty() {
            let n = self.ed.delete_selected();
            self.status = if n == 0 { "Selected objects are on a locked or hidden layer.".into() } else { format!("Deleted {n} object{}", if n == 1 { "" } else { "s" }) };
        } else if self.ed.frame_sel.is_some() {
            self.ed.clear_frames();
            self.status = "Cleared frames".into();
        } else {
            self.status = "Nothing selected. Click an object (or drag a box around it), or select frames in the timeline.".into();
        }
    }

    pub(crate) fn copy(&mut self) {
        if !self.ed.selection.is_empty() {
            let n = self.ed.copy_selected();
            self.status = format!("Copied {n} object(s)");
        } else if self.ed.frame_sel.is_some() {
            self.ed.copy_frames();
            self.status = "Copied frames".into();
        }
        self.mark_clipboard();
    }
    /// Put a marker on the system clipboard: platforms only deliver a Paste
    /// event when the system clipboard isn't empty.
    fn mark_clipboard(&self) {
        self.ctx.copy_text("Flare clipboard".into());
    }
    pub(crate) fn cut(&mut self) {
        if !self.ed.selection.is_empty() {
            self.ed.cut_selected();
            self.status = "Cut".into();
        } else if self.ed.frame_sel.is_some() {
            self.ed.cut_frames();
            self.status = "Cut frames".into();
        }
        self.mark_clipboard();
    }
    pub(crate) fn paste(&mut self, in_place: bool) {
        if self.ed.frame_sel.is_some() && self.ed.can_paste_frames() {
            self.ed.paste_frames();
            self.status = "Pasted frames".into();
        } else if self.ed.can_paste() {
            if self.ed.layer_locked() {
                self.status = "Current layer is locked or hidden.".into();
            } else {
                self.ed.paste(in_place);
                self.status = "Pasted".into();
            }
        } else if self.ed.can_paste_frames() {
            self.ed.paste_frames();
            self.status = "Pasted frames".into();
        }
    }

    pub(crate) fn open_symbol_dialog(&mut self, convert: bool) {
        if convert && self.ed.selection.is_empty() {
            self.status = "Select something on the stage first, then press F8.".into();
            return;
        }
        self.dialog = Dialog::Symbol { convert, name: format!("Symbol {}", self.ed.doc.next_symbol_id), kind: SymbolKind::Graphic, reg: self.settings.symbol_reg, folder: String::new() };
    }

    pub(crate) fn edit_symbol(&mut self, id: SymbolId) {
        self.place.clear();
        self.ed.enter_symbol(id);
        self.fit_pending = true;
        self.status = "Editing symbol. Press Esc or click the scene name to go back.".into();
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) || !matches!(self.dialog, Dialog::None) {
            return;
        }
        // Copy/cut/paste arrive as events, not key presses, on most platforms.
        let (copy, cut, paste, shift_held) = ctx.input(|i| {
            let mut c = (false, false, false);
            for e in &i.events {
                match e {
                    egui::Event::Copy => c.0 = true,
                    egui::Event::Cut => c.1 = true,
                    egui::Event::Paste(_) => c.2 = true,
                    _ => {}
                }
            }
            (c.0, c.1, c.2, i.modifiers.shift)
        });
        if copy { self.copy(); }
        if cut { self.cut(); }
        if paste { self.paste(shift_held); }

        let cmd = Modifiers::COMMAND;
        let sh = Modifiers::SHIFT;
        let none = Modifiers::NONE;
        let k = |m: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(m, key));
        if k(cmd | sh, Key::Z) || k(cmd, Key::Y) {
            if let Some(l) = self.ed.redo() { self.status = format!("Redo {l}"); }
        }
        if k(cmd, Key::Z) {
            if let Some(l) = self.ed.undo() { self.status = format!("Undo {l}"); }
        }
        // Copy / Paste Motion (before the plain Ctrl+C / Ctrl+V fallbacks, which would match them too)
        if k(cmd | sh | Modifiers::ALT, Key::C) { palette::copy_motion(self); }
        if k(cmd | sh | Modifiers::ALT, Key::V) { palette::paste_motion(self, true); }
        // Fallbacks in case the platform reports these as plain keys.
        if k(cmd, Key::C) { self.copy(); }
        if k(cmd, Key::X) { self.cut(); }
        if k(cmd | sh, Key::V) { self.paste(true); }
        if k(cmd, Key::V) { self.paste(false); }
        if k(cmd, Key::D) { self.ed.duplicate(); }
        if k(cmd, Key::A) { self.ed.select_all(); }
        // Ctrl+Alt+B before Ctrl+B: a Ctrl+B match would also take Ctrl+Alt+B
        if k(cmd | Modifiers::ALT, Key::B) { self.simple_buttons = !self.simple_buttons; self.status = format!("Simple buttons {}", if self.simple_buttons { "on" } else { "off" }); }
        if k(cmd, Key::B) { self.ed.break_apart(); }
        if k(cmd, Key::T) { self.transform_open = !self.transform_open; }
        if k(cmd | sh, Key::F9) { self.colors.open = !self.colors.open; }
        if k(cmd, Key::F9) { self.toggle_swatches(); }
        if k(cmd, Key::U) { self.prefs_open = true; }
        if k(cmd, Key::J) { self.doc_dlg = Some(windows::DocDlg::of(self)); }
        if k(cmd, Key::F10) { self.history_open = !self.history_open; }
        if k(cmd, Key::K) { self.align_open = !self.align_open; }
        if k(cmd | sh | Modifiers::ALT, Key::R) { self.view.rulers = !self.view.rulers; }
        if k(cmd | sh, Key::W) { self.view.clip = !self.view.clip; }
        if k(cmd | sh, Key::Quote) { self.view.snap_grid = !self.view.snap_grid; }
        if k(cmd, Key::Quote) { self.view.grid = !self.view.grid; }
        if k(cmd | Modifiers::ALT, Key::Semicolon) { self.view.lock_guides = !self.view.lock_guides; }
        if k(cmd | sh, Key::Semicolon) { self.view.snap_guides = !self.view.snap_guides; }
        if k(cmd, Key::Semicolon) { self.view.guides = !self.view.guides; }
        if k(cmd, Key::I) { self.info_open = !self.info_open; }
        if k(cmd | sh, Key::H) { self.add_shape_hint(); }
        if k(cmd | sh, Key::G) { let n = self.ed.ungroup_selected(); if n > 0 { self.status = format!("Ungrouped {n} group(s)"); } }
        if k(cmd, Key::G) { let n = self.ed.group_selected(); if n > 0 { self.status = "Grouped".into(); } }
        if k(cmd | Modifiers::ALT, Key::L) { self.set_loop_range(); }
        if k(cmd, Key::L) { self.lib.open = !self.lib.open; }
        if k(cmd | Modifiers::ALT, Key::S) { self.open_scale_rotate(); }
        if k(cmd | sh, Key::S) { self.file(FileAction::Save); }
        if k(cmd, Key::S) { self.save(); }
        if k(cmd, Key::O) { self.guarded(files::Pending::Open); }
        if k(cmd, Key::N) { self.guarded(files::Pending::New); }
        if k(cmd, Key::Q) { self.guarded(files::Pending::Quit); }
        if k(cmd | sh, Key::F12) { self.publish.open = true; }
        if k(cmd, Key::R) { self.file(FileAction::ImportImage); }
        if k(cmd, Key::Equals) || k(cmd, Key::Plus) { self.zoom *= 1.25; }
        if k(cmd, Key::Minus) { self.zoom /= 1.25; }
        if k(cmd, Key::Num1) { self.zoom = 1.0; }
        if k(cmd, Key::Num2) { self.fit_pending = true; }
        if k(sh, Key::F2) { self.scene_open = !self.scene_open; }
        if k(sh, Key::F5) { self.ed.remove_frame(); }
        if k(none, Key::F5) { self.ed.insert_frame(); }
        if k(sh, Key::F6) { self.ed.clear_keyframe(); }
        if k(none, Key::F6) { self.ed.insert_keyframe(); }
        if k(none, Key::F7) { self.ed.insert_blank_keyframe(); }
        if k(cmd, Key::F8) { self.open_symbol_dialog(false); }
        if k(none, Key::F8) { self.open_symbol_dialog(true); }
        if k(cmd, Key::Enter) { publish::test_movie(self); }
        let busy = self.pen.is_some() || !self.poly_lasso.is_empty() || self.text_edit.is_some();
        if !busy && k(none, Key::Enter) { self.toggle_play(); }
        if k(sh, Key::Period) { self.step_keyframe(true); }
        if k(sh, Key::Comma) { self.step_keyframe(false); }
        if k(none, Key::Period) { self.step(1); }
        if k(none, Key::Comma) { self.step(-1); }
        if k(none, Key::Delete) || k(none, Key::Backspace) { self.delete(); }
        // Arrow keys nudge the selection (Shift = 10 px).
        for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
            let big = k(sh, key);
            if big || k(none, key) {
                let s = if big { 10.0 } else { 1.0 };
                self.ed.modify_selected("Nudge", |e| { e.matrix.tx += dx * s; e.matrix.ty += dy * s; });
            }
        }
        if !busy && k(none, Key::Escape) {
            if !self.ed.selection.is_empty() || self.ed.frame_sel.is_some() {
                self.ed.deselect();
                self.ed.frame_sel = None;
            } else if self.ed.editing.is_some() {
                self.exit_editing(false);
            }
        }
        toolbar::tool_shortcuts(self, ctx);
    }

    // ------------------------------------------------------------ menus
    /// Object commands shared by the Edit/Modify menus and the stage context menu.
    /// Onion-skin range (inclusive), from the markers.
    pub(crate) fn onion_range(&self) -> (u32, u32) {
        let o = &self.onion_opts;
        match o.anchor {
            Some(r) => r,
            None => (self.ed.frame.saturating_sub(o.before), self.ed.frame + o.after),
        }
    }

    pub(crate) fn add_shape_hint(&mut self) {
        match self.ed.add_shape_hint() {
            Some(i) => self.status = format!("Shape hint '{}' added: drag it onto the outline on both keyframes", (b'a' + i as u8) as char),
            None => self.status = "Shape hints need a shape tween: put the playhead on its first or last keyframe".into(),
        }
    }

    pub(crate) fn object_menu(&mut self, ui: &mut egui::Ui) {
        let has_sel = !self.ed.selection.is_empty();
        if ui.add_enabled(has_sel, egui::Button::new("Cut").shortcut_text("Ctrl+X")).clicked() { self.cut(); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Copy").shortcut_text("Ctrl+C")).clicked() { self.copy(); ui.close(); }
        if ui.add_enabled(self.ed.can_paste(), egui::Button::new("Paste in Center").shortcut_text("Ctrl+V")).clicked() { self.paste(false); ui.close(); }
        if ui.add_enabled(self.ed.can_paste(), egui::Button::new("Paste in Place").shortcut_text("Ctrl+Shift+V")).clicked() { self.paste(true); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Duplicate").shortcut_text("Ctrl+D")).clicked() { self.ed.duplicate(); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Delete").shortcut_text("Del")).clicked() { self.delete(); ui.close(); }
        ui.separator();
        if ui.add(egui::Button::new("Select All").shortcut_text("Ctrl+A")).clicked() { self.ed.select_all(); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Deselect All").shortcut_text("Esc")).clicked() { self.ed.deselect(); ui.close(); }
        ui.separator();
        if ui.add_enabled(has_sel, egui::Button::new("Convert to Symbol…").shortcut_text("F8")).clicked() { self.open_symbol_dialog(true); ui.close(); }
        let inst = self.ed.selected_element().and_then(|e| if let ElementKind::Instance(i) = e.kind { Some(i.symbol) } else { None });
        if ui.add_enabled(inst.is_some(), egui::Button::new("Break Apart").shortcut_text("Ctrl+B")).clicked() { self.ed.break_apart(); ui.close(); }
        if let Some(id) = inst {
            if ui.button("Edit Symbol").clicked() { self.edit_symbol(id); ui.close(); }
        }
        let has_bitmap = self.ed.selection.iter().any(|s| matches!(self.ed.element_at(*s).map(|e| e.kind), Some(ElementKind::Bitmap(_))));
        if ui.add_enabled(has_bitmap, egui::Button::new("Trace Bitmap…")).on_hover_text("Modify › Bitmap › Trace Bitmap: turn the selected bitmap into vector fills").clicked() {
            self.dialog = Dialog::TraceBitmap { threshold: 100, min_area: 8, fit: 3, corners: 1, centerline: false, ink: 128, uniform: false };
            ui.close();
        }
        if ui.add_enabled(has_sel, egui::Button::new("Group").shortcut_text("Ctrl+G")).clicked() { self.ed.group_selected(); ui.close(); }
        let has_group = self.ed.selection.iter().any(|s| matches!(self.ed.element_at(*s).map(|e| e.kind), Some(ElementKind::Group(_))));
        if ui.add_enabled(has_group, egui::Button::new("Ungroup").shortcut_text("Ctrl+Shift+G")).clicked() { self.ed.ungroup_selected(); ui.close(); }
        ui.menu_button("Combine Objects", |ui| {
            use ac_core::merge::BoolOp;
            let float = self.ed.merge_float().is_some();
            if ui.add_enabled(has_sel, egui::Button::new("Union")).on_hover_text("Drawing objects → one object; a shape selection → a drawing object").clicked() {
                if float {
                    self.ed.merge_to_object();
                } else if let Err(e) = self.ed.combine_objects(BoolOp::Union) {
                    self.status = e;
                }
                ui.close();
            }
            for (label, op) in [("Intersect", BoolOp::Intersect), ("Punch", BoolOp::Punch), ("Crop", BoolOp::Crop)] {
                if ui.add_enabled(has_sel, egui::Button::new(label)).clicked() {
                    if let Err(e) = self.ed.combine_objects(op) {
                        self.status = e;
                    }
                    ui.close();
                }
            }
        });
        ui.menu_button("Shape", |ui| {
            if ui.add_enabled(has_sel, egui::Button::new("Smooth")).clicked() { self.ed.shape_cleanup("smooth"); ui.close(); }
            if ui.add_enabled(has_sel, egui::Button::new("Straighten")).clicked() { self.ed.shape_cleanup("straighten"); ui.close(); }
            if ui.add_enabled(has_sel, egui::Button::new("Optimize")).clicked() { self.ed.shape_cleanup("optimize"); ui.close(); }
            ui.separator();
            if ui.add_enabled(has_sel, egui::Button::new("Convert Lines to Fills")).on_hover_text("Each selected line becomes a filled outline").clicked() { self.ed.shape_fill_op("lines_to_fills", 0.0, 0, false); ui.close(); }
            if ui.add_enabled(has_sel, egui::Button::new("Expand Fill…")).clicked() { self.shape_dlg = Some(windows::ShapeDlg { soften: false, dist: 4.0, steps: 4, inset: false }); ui.close(); }
            if ui.add_enabled(has_sel, egui::Button::new("Soften Fill Edges…")).clicked() { self.shape_dlg = Some(windows::ShapeDlg { soften: true, dist: 4.0, steps: 4, inset: false }); ui.close(); }
            ui.separator();
            let ok = self.ed.shape_tween_keys().is_some();
            if ui.add_enabled(ok, egui::Button::new("Add Shape Hint").shortcut_text("Ctrl+Shift+H")).clicked() { self.add_shape_hint(); ui.close(); }
            if ui.add_enabled(ok, egui::Button::new("Remove All Hints")).clicked() { self.ed.remove_all_shape_hints(); ui.close(); }
        });
        ui.menu_button("Align", |ui| {
            use ac_core::editor::AlignOp::*;
            ui.checkbox(&mut self.align_to_stage, "To Stage");
            ui.separator();
            for (l, o) in [("Left", Left), ("Horizontal Center", HCenter), ("Right", Right), ("Top", Top), ("Vertical Center", VCenter), ("Bottom", Bottom)] {
                if ui.button(l).clicked() { self.ed.align(o, self.align_to_stage); ui.close(); }
            }
            ui.separator();
            for (l, o) in [("Distribute Widths Evenly", SpaceH), ("Distribute Heights Evenly", SpaceV), ("Make Same Width", MatchWidth), ("Make Same Height", MatchHeight)] {
                if ui.button(l).clicked() { self.ed.align(o, self.align_to_stage); ui.close(); }
            }
            ui.separator();
            if ui.button("Align Panel…").clicked() { self.align_open = true; ui.close(); }
        });
        ui.menu_button("Arrange", |ui| {
            if ui.button("Bring to Front").clicked() { self.ed.arrange(true); ui.close(); }
            if ui.button("Send to Back").clicked() { self.ed.arrange(false); ui.close(); }
        });
        ui.menu_button("Transform", |ui| self.transform_menu(ui));
    }

    /// Modify › Transform (also on the stage context menu).
    pub(crate) fn transform_menu(&mut self, ui: &mut egui::Ui) {
        use toolbar::FtMode;
        let has_sel = !self.ed.selection.is_empty();
        for (mode, label, key) in [(FtMode::Normal, "Free Transform", "Q"), (FtMode::Distort, "Distort", ""), (FtMode::Envelope, "Envelope", "")] {
            let on = self.tool == Tool::FreeTransform && self.topts.ft_mode == mode;
            if ui.add(egui::Button::selectable(on, label)).on_hover_text(if key.is_empty() { "Free Transform mode (shapes only)" } else { "Scale, rotate, skew; drag the white dot to move the transformation point" }).clicked() {
                self.topts.ft_mode = mode;
                self.set_tool(Tool::FreeTransform);
                ui.close();
            }
        }
        ui.separator();
        if ui.add_enabled(has_sel, egui::Button::new("Scale and Rotate…").shortcut_text("Ctrl+Alt+S")).clicked() { self.open_scale_rotate(); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Rotate 90° CW")).clicked() { self.rotate_selection(std::f32::consts::FRAC_PI_2); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Rotate 90° CCW")).clicked() { self.rotate_selection(-std::f32::consts::FRAC_PI_2); ui.close(); }
        ui.separator();
        if ui.add_enabled(has_sel, egui::Button::new("Flip Horizontal")).clicked() { self.flip(true); ui.close(); }
        if ui.add_enabled(has_sel, egui::Button::new("Flip Vertical")).clicked() { self.flip(false); ui.close(); }
        ui.separator();
        if ui.add_enabled(has_sel, egui::Button::new("Remove Transform")).on_hover_text("Reset scale, rotation, skew and 3D (keeps position)").clicked() {
            self.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); e.three_d = None; });
            ui.close();
        }
        if ui.add(egui::Button::new("Transform Panel").shortcut_text("Ctrl+T")).clicked() { self.transform_open = true; ui.close(); }
    }

    pub(crate) fn open_layer_props(&mut self, i: usize) {
        let Some(l) = self.ed.timeline().layers.get(i) else { return };
        self.dialog = Dialog::LayerProps { i, name: l.name.clone(), kind: l.kind, parent: l.parent, color: c32(l.color), visible: l.visible, locked: l.locked, outline: l.outline, height: l.row_height, filters: l.filters.clone() };
    }

    pub(crate) fn open_scale_rotate(&mut self) {
        if self.ed.selection.is_empty() {
            self.status = "Select something first.".into();
            return;
        }
        self.dialog = Dialog::ScaleRotate { scale: 100.0, rotate: 0.0 };
    }

    fn selection_center(&self) -> Option<Pt> {
        self.pivot()
    }
    pub(crate) fn flip(&mut self, horizontal: bool) {
        let Some(c) = self.selection_center() else { return };
        let s = if horizontal { Matrix::scale(-1.0, 1.0) } else { Matrix::scale(1.0, -1.0) };
        let m = Matrix::translate(c.x, c.y).concat(&s).concat(&Matrix::translate(-c.x, -c.y));
        self.ed.modify_selected(if horizontal { "Flip Horizontal" } else { "Flip Vertical" }, |e| e.matrix = m.concat(&e.matrix));
    }
    pub(crate) fn rotate_selection(&mut self, rad: f32) {
        let Some(c) = self.selection_center() else { return };
        let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(rad)).concat(&Matrix::translate(-c.x, -c.y));
        self.ed.modify_selected("Rotate", |e| e.matrix = m.concat(&e.matrix));
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.brand(ui);
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new("New…").shortcut_text("Ctrl+N")).clicked() { self.guarded(files::Pending::New); ui.close(); }
                    if ui.add(egui::Button::new("Open…").shortcut_text("Ctrl+O")).clicked() { self.guarded(files::Pending::Open); ui.close(); }
                    ui.menu_button("Open Recent", |ui| {
                        if self.settings.recent.is_empty() {
                            ui.label(RichText::new("No recent files").color(Color32::from_gray(140)));
                        }
                        for r in self.settings.recent.clone() {
                            let name = std::path::Path::new(&r).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(r.clone());
                            if ui.button(name).on_hover_text(&r).clicked() { self.guarded(files::Pending::OpenPath(r)); ui.close(); }
                        }
                        if !self.settings.recent.is_empty() {
                            ui.separator();
                            if ui.button("Clear Recent").clicked() { self.settings.recent.clear(); self.settings.save(); ui.close(); }
                        }
                    });
                    if ui.button("Open Demo Scene").clicked() { self.guarded(files::Pending::OpenDemo); ui.close(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Save").shortcut_text("Ctrl+S")).clicked() { self.save(); ui.close(); }
                    if ui.add(egui::Button::new("Save As…").shortcut_text("Ctrl+Shift+S")).clicked() { self.file(FileAction::Save); ui.close(); }
                    ui.separator();
                    ui.menu_button("Import", |ui| {
                        if ui.add(egui::Button::new("Import to Stage…").shortcut_text("Ctrl+R")).on_hover_text("PNG, JPG, GIF, BMP, TGA, TIFF, WebP, OpenEXR").clicked() { self.file(FileAction::ImportImage); ui.close(); }
                        if ui.button("Import Image Sequence…").on_hover_text("walk0001.png, walk0002.png, … onto consecutive keyframes").clicked() { self.file(FileAction::ImportSequence); ui.close(); }
                        if ui.button("Import Photoshop (PSD)…").on_hover_text("Each Photoshop layer becomes a bitmap on its own layer, in place").clicked() { self.file(FileAction::ImportPsd); ui.close(); }
                        if ui.button("Import XDTS Timing Sheet…").on_hover_text("Exposure sheet from OpenToonz, Clip Studio Paint, TVPaint…: columns named like layers re-time them; others load numbered images next to the sheet (name_0001.png) or become labelled blank keyframes").clicked() { self.file(FileAction::ImportXdts); ui.close(); }
                        if ui.button("Import SVG…").clicked() { self.file(FileAction::ImportSvg); ui.close(); }
                        if ui.button("Import Sound…").on_hover_text("WAV, MP3, OGG Vorbis, FLAC, AIFF").clicked() { self.file(FileAction::ImportSound); ui.close(); }
                    });
                    ui.menu_button("Export", |ui| {
                        if ui.button("Export Image…").on_hover_text("The current frame as PNG, JPEG or GIF").clicked() { self.publish.image_open = true; ui.close(); }
                        if ui.button("Export SVG…").clicked() { self.file(FileAction::ExportSvg); ui.close(); }
                        if ui.button("Export Animated GIF…").clicked() { self.file(FileAction::ExportGif); ui.close(); }
                        if ui.button("Export PNG Sequence…").clicked() { self.file(FileAction::ExportPngSequence); ui.close(); }
                        if ui.button("Export Video…").on_hover_text("MP4 / WebM / MOV (needs ffmpeg)").clicked() { self.file(FileAction::ExportVideo); ui.close(); }
                        ui.separator();
                        if ui.button("Publish HTML5 Canvas…").on_hover_text("A web page that plays the movie: scripts, buttons and sounds included").clicked() { self.file(FileAction::ExportHtml5); ui.close(); }
                        if ui.button("Generate Sprite Sheet…").on_hover_text("All frames in one PNG + JSON / Starling XML data").clicked() { self.file(FileAction::ExportSpriteSheet); ui.close(); }
                        if ui.button("Export Animated SVG…").clicked() { self.file(FileAction::ExportAnimatedSvg); ui.close(); }
                        if ui.button("Export Lottie…").clicked() { self.file(FileAction::ExportLottie); ui.close(); }
                        if ui.button("Export Timing Sheet (XDTS)…").on_hover_text("Layer timing for exposure-sheet tools (Clip Studio Paint, OpenToonz, TVPaint)").clicked() { self.file(FileAction::ExportXdts); ui.close(); }
                        ui.separator();
                        if ui.button("Export .fla / XFL…").on_hover_text("CS5+ .fla (zipped) or an uncompressed XFL folder").clicked() { self.file(FileAction::ExportXfl); ui.close(); }
                    });
                    if ui.add(egui::Button::new("Publish Settings…").shortcut_text("Ctrl+Shift+F12")).clicked() { self.publish.open = true; ui.close(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Quit").shortcut_text("Ctrl+Q")).clicked() { self.guarded(files::Pending::Quit); ui.close(); }
                });
                ui.menu_button("Edit", |ui| {
                    let u = self.ed.undo_label().map(|l| format!("Undo {l}")).unwrap_or("Undo".into());
                    if ui.add_enabled(self.ed.undo_label().is_some(), egui::Button::new(u).shortcut_text("Ctrl+Z")).clicked() { self.ed.undo(); ui.close(); }
                    let r = self.ed.redo_label().map(|l| format!("Redo {l}")).unwrap_or("Redo".into());
                    if ui.add_enabled(self.ed.redo_label().is_some(), egui::Button::new(r).shortcut_text("Ctrl+Shift+Z")).clicked() { self.ed.redo(); ui.close(); }
                    ui.separator();
                    self.object_menu(ui);
                    ui.separator();
                    ui.menu_button("Timeline", |ui| timeline::frame_menu(self, ui));
                    if ui.add_enabled(self.ed.editing.is_some(), egui::Button::new("Edit Document")).clicked() { self.exit_editing(true); ui.close(); }
                    ui.separator();
                    if ui.add(egui::Button::new("Preferences…").shortcut_text("Ctrl+U")).clicked() { self.prefs_open = true; ui.close(); }
                });
                ui.menu_button("View", |ui| {
                    if ui.add(egui::Button::new("Zoom In").shortcut_text("Ctrl+=")).clicked() { self.zoom *= 1.25; ui.close(); }
                    if ui.add(egui::Button::new("Zoom Out").shortcut_text("Ctrl+-")).clicked() { self.zoom /= 1.25; ui.close(); }
                    if ui.add(egui::Button::new("100%").shortcut_text("Ctrl+1")).clicked() { self.zoom = 1.0; ui.close(); }
                    if ui.add(egui::Button::new("Fit in Window").shortcut_text("Ctrl+2")).clicked() { self.fit_pending = true; ui.close(); }
                    if ui.add_enabled(self.view_rot != 0.0, egui::Button::new("Reset View Rotation")).clicked() { let r = self.stage_rect; self.set_view_rot(r, 0.0); ui.close(); }
                    ui.separator();
                    ui.checkbox(&mut self.topts.show_bones, "Show Bones");
                    ui.add(egui::Checkbox::new(&mut self.view.rulers, "Rulers")).on_hover_text("Ctrl+Alt+Shift+R");
                    ui.add(egui::Checkbox::new(&mut self.view.clip, "Clip Content Outside Stage")).on_hover_text("Ctrl+Shift+W");
                    ui.menu_button("Grid", |ui| {
                        ui.checkbox(&mut self.view.grid, "Show Grid  (Ctrl+')");
                        ui.checkbox(&mut self.view.snap_grid, "Snap to Grid  (Ctrl+Shift+')");
                        if ui.button("Edit Grid…").clicked() { self.view.edit_grid_open = true; ui.close(); }
                    });
                    ui.menu_button("Snapping", |ui| {
                        ui.checkbox(&mut self.topts.snap, "Snap to Objects");
                        ui.checkbox(&mut self.view.snap_grid, "Snap to Grid  (Ctrl+Shift+')");
                        ui.checkbox(&mut self.view.snap_guides, "Snap to Guides  (Ctrl+Shift+;)");
                        ui.checkbox(&mut self.view.snap_pixels, "Snap to Pixels");
                    });
                    ui.menu_button("Guides", |ui| {
                        ui.checkbox(&mut self.view.guides, "Show Guides  (Ctrl+;)");
                        ui.checkbox(&mut self.view.lock_guides, "Lock Guides  (Ctrl+Alt+;)");
                        ui.checkbox(&mut self.view.snap_guides, "Snap to Guides  (Ctrl+Shift+;)");
                        if ui.button("Edit Guides…").clicked() { self.view.edit_guides_open = true; ui.close(); }
                        if ui.button("Clear Guides").clicked() { self.ed.clear_guides(); ui.close(); }
                        ui.label(RichText::new("Drag from a ruler to add a guide").small().color(Color32::from_gray(140)));
                    });
                    ui.separator();
                    ui.label(RichText::new("Library position").small().color(Color32::from_gray(150)));
                    let before = self.layout;
                    ui.radio_value(&mut self.layout.library, dock::LibraryDock::WithProperties, "Under Properties");
                    ui.radio_value(&mut self.layout.library, dock::LibraryDock::Left, "Own panel on the left");
                    ui.radio_value(&mut self.layout.library, dock::LibraryDock::Right, "Own panel on the right");
                    if self.layout != before { self.save_layout(); }
                });
                ui.menu_button("Insert", |ui| {
                    if ui.add(egui::Button::new("New Symbol…").shortcut_text("Ctrl+F8")).clicked() { self.open_symbol_dialog(false); ui.close(); }
                    if ui.button("New Layer").clicked() { self.ed.add_layer(); ui.close(); }
                    if ui.button("Scene").clicked() { self.ed.add_scene(); self.fit_pending = true; self.status = format!("Added {}", self.ed.scene_tl().name); ui.close(); }
                    if ui.button("Layer Folder").clicked() { self.ed.wrap_layer(ac_core::LayerKind::Folder, "Folder"); ui.close(); }
                    if ui.button("Classic Motion Guide").clicked() { self.ed.wrap_layer(ac_core::LayerKind::Guide, "Motion Guide"); ui.close(); }
                    if ui.button("Mask Layer").on_hover_text("Makes a mask above the current layer").clicked() { self.ed.wrap_layer(ac_core::LayerKind::Mask, "Mask"); ui.close(); }
                    if self.ed.camera_layer().is_none() {
                        if ui.add_enabled(self.ed.editing.is_none(), egui::Button::new("Camera Layer")).clicked() { self.ed.add_camera(); self.set_tool(Tool::Camera); ui.close(); }
                    } else if ui.button("Remove Camera Layer").clicked() { self.ed.remove_camera(); ui.close(); }
                    ui.separator();
                    timeline::frame_menu(self, ui);
                });
                ui.menu_button("Modify", |ui| {
                    if ui.add(egui::Button::new("Document…").shortcut_text("Ctrl+J")).clicked() { self.doc_dlg = Some(windows::DocDlg::of(self)); ui.close(); }
                    ui.separator();
                    self.object_menu(ui);
                });
                ui.menu_button("Control", |ui| {
                    if ui.add(egui::Button::new(if self.playing { "Stop" } else { "Play" }).shortcut_text("Enter")).clicked() { self.toggle_play(); ui.close(); }
                    if ui.add(egui::Button::new("Test Movie").shortcut_text("Ctrl+Enter")).on_hover_text("Plays the movie in your browser (HTML5 publish), with scripts, buttons and sounds").clicked() { publish::test_movie(self); ui.close(); }
                    ui.separator();
                    if ui.button("Rewind").clicked() { self.step(-100_000); ui.close(); }
                    if ui.button("Go to End").clicked() { self.step(100_000); ui.close(); }
                    if ui.add(egui::Button::new("Step Forward One Frame").shortcut_text(".")).clicked() { self.step(1); ui.close(); }
                    if ui.add(egui::Button::new("Step Backward One Frame").shortcut_text(",")).clicked() { self.step(-1); ui.close(); }
                    if ui.add(egui::Button::new("Next Keyframe").shortcut_text("Shift+.")).clicked() { self.step_keyframe(true); ui.close(); }
                    if ui.add(egui::Button::new("Previous Keyframe").shortcut_text("Shift+,")).clicked() { self.step_keyframe(false); ui.close(); }
                    ui.separator();
                    ui.checkbox(&mut self.loop_play, "Loop Playback");
                    if ui.add(egui::Button::new("Loop Range from Selection").shortcut_text("Ctrl+Alt+L")).on_hover_text("Playback loops over the selected frames (drag the orange markers above the frames to change it)").clicked() { self.set_loop_range(); ui.close(); }
                    if ui.add_enabled(self.tl.loop_range.is_some(), egui::Button::new("Clear Loop Range")).clicked() { self.tl.loop_range = None; ui.close(); }
                    ui.checkbox(&mut self.onion, "Onion Skin");
                    ui.checkbox(&mut self.scripting.frame_scripts, "Enable Frame Scripts");
                    ui.checkbox(&mut self.simple_buttons, "Enable Simple Buttons  (Ctrl+Alt+B)");
                });
                ui.menu_button("Commands", |ui| {
                    if ui.button("Run Script…").clicked() { self.scripting.console_open = true; ui.close(); }
                    if ui.button("Script Console…").clicked() { self.scripting.console_open = true; ui.close(); }
                    if ui.button("Command Reference").clicked() { self.scripting.reference_open = true; ui.close(); }
                    ui.separator();
                    ui.menu_button("Run Example", |ui| {
                        for l in ac_script::Language::ALL {
                            if ui.button(l.name()).clicked() {
                                scripting::request_run(self, scripting::Pending::Code(l, l.example().to_string()));
                                ui.close();
                            }
                        }
                    });
                });
                ui.menu_button("Window", |ui| {
                    if ui.add(egui::Button::new("Actions").shortcut_text("F9")).clicked() { self.scripting.actions_open = true; ui.close(); }
                    if ui.add(egui::Button::selectable(self.lib.open, "Library  (Ctrl+L)")).clicked() { self.lib.open = !self.lib.open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.colors.open, "Color  (Ctrl+Shift+F9)")).clicked() { self.colors.open = !self.colors.open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.swatch_ui.open, "Swatches  (Ctrl+F9)")).on_hover_text("Colour chips beside the tools").clicked() { self.toggle_swatches(); ui.close(); }
                    if ui.add(egui::Button::selectable(self.history_open, "History  (Ctrl+F10)")).clicked() { self.history_open = !self.history_open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.nav.open, "Navigator")).clicked() { self.nav.open = !self.nav.open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.settings.options_bar, "Tool Options Bar")).clicked() { self.toggle_options_bar(); ui.close(); }
                    if ui.add(egui::Button::new("Command Search…").shortcut_text("Ctrl+Shift+P")).clicked() { self.palette.open = true; ui.close(); }
                    ui.separator();
                    if ui.add(egui::Button::selectable(self.transform_open, "Transform  (Ctrl+T)")).clicked() { self.transform_open = !self.transform_open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.align_open, "Align  (Ctrl+K)")).clicked() { self.align_open = !self.align_open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.info_open, "Info  (Ctrl+I)")).clicked() { self.info_open = !self.info_open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.motion.open, "Motion Editor")).clicked() { self.motion.open = !self.motion.open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.presets.open, "Motion Presets")).on_hover_text("Ready-made movements: bounce, fly-in, fade, pulse, shake, spin, slides… and your own").clicked() { self.presets.open = !self.presets.open; ui.close(); }
                    if ui.add(egui::Button::selectable(self.scene_open, "Scene  (Shift+F2)")).clicked() { self.scene_open = !self.scene_open; ui.close(); }
                    if ui.add(egui::Button::new("Output").shortcut_text("F2")).clicked() { self.scripting.output_open = true; ui.close(); }
                    if ui.button("Script Console").clicked() { self.scripting.console_open = true; ui.close(); }
                    if ui.button("Command Reference").clicked() { self.scripting.reference_open = true; ui.close(); }
                    ui.separator();
                    if ui.button("Startup Window…").on_hover_text("New scene or open an existing one").clicked() { self.startup = Some(startup::Startup::new(self)); ui.close(); }
                    if ui.button("Edit Toolbar…").clicked() { self.topts.edit_toolbar_open = true; ui.close(); }
                    ui.separator();
                    if ui.button("Reset Layout").on_hover_text("Tools left, Properties and Library right, Timeline below").clicked() { self.reset_layout(); ui.close(); }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About Flare").clicked() { self.dialog = Dialog::About; ui.close(); }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.workspace_menu(ui);
                });
            });
        });
    }

    /// Flare's icon and wordmark at the left of the menu bar (as in Flare
    /// Studio); clicking it shows About.
    fn brand(&mut self, ui: &mut egui::Ui) {
        let text = RichText::new("Flare").strong().size(14.0).color(Color32::from_rgb(0xE8, 0x7A, 0x3C));
        let button = match startup::icon_texture(ui.ctx()) {
            Some(t) => egui::Button::image_and_text(egui::Image::new(&t).fit_to_exact_size(egui::vec2(16.0, 16.0)), text),
            None => egui::Button::new(text),
        };
        if ui.add(button.frame(false)).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text("About Flare").clicked() {
            self.dialog = Dialog::About;
        }
        ui.add_space(6.0);
    }

    /// The Workspace menu at the right of the menu bar: where each dock sits.
    fn workspace_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button(RichText::new("Workspace").color(Color32::from_gray(170)), |ui| {
            let before = self.layout;
            ui.label(RichText::new("Drag the dots beside a panel's name to move it, or choose here").small().color(Color32::from_gray(150)));
            ui.separator();
            ui.checkbox(&mut self.layout.tools_right, "Tools on the right");
            ui.checkbox(&mut self.layout.props_left, "Properties on the left");
            ui.checkbox(&mut self.layout.timeline_top, "Timeline above the stage");
            ui.menu_button("Library", |ui| {
                ui.radio_value(&mut self.layout.library, dock::LibraryDock::WithProperties, "Under Properties");
                ui.radio_value(&mut self.layout.library, dock::LibraryDock::Left, "Own panel on the left");
                ui.radio_value(&mut self.layout.library, dock::LibraryDock::Right, "Own panel on the right");
            });
            if self.layout != before { self.save_layout(); }
            ui.separator();
            if ui.button("Reset Layout").clicked() { self.reset_layout(); ui.close(); }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").exact_size(22.0).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{:.0}%", self.zoom * 100.0)).small());
                    ui.label(egui::RichText::new(self.tool.info().name).small().color(Color32::from_gray(150)));
                    let n = self.ed.selection.len();
                    if n > 0 {
                        ui.label(egui::RichText::new(format!("{n} selected")).small().color(ACCENT));
                    }
                });
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut d = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        let mut run: Option<(FileAction, String)> = None;
        let mut symbol_ok: Option<(bool, String, SymbolKind, [u8; 2])> = None;
        let mut rename_ok: Option<(SymbolId, String)> = None;
        let mut delete_ok: Option<SymbolId> = None;
        let mut scale_rot: Option<(f32, f32)> = None;
        let mut unsaved_choice: Option<(files::Pending, bool)> = None;
        let mut recover: Option<bool> = None;
        let mut path_cancelled = false;
        let mut bitmap_rename: Option<(u32, String)> = None;
        let mut bitmap_delete: Option<u32> = None;
        #[allow(clippy::type_complexity)]
        let mut trace_ok: Option<ac_core::trace::TraceOpts> = None;
        let mut layer_ok: Option<LayerPropsOk> = None;
        let enter = ctx.input(|i| i.key_pressed(Key::Enter));
        let esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let win = |t: &str| egui::Window::new(t.to_string()).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]);
        match &mut d {
            Dialog::None => {}
            Dialog::Path { title, path, action } => {
                win(title).show(ctx, |ui| {
                    ui.label("File path:");
                    let r = ui.add(egui::TextEdit::singleline(path).desired_width(360.0));
                    r.request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter {
                            run = Some((*action, path.clone()));
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || esc { close = true; path_cancelled = true; }
                    });
                });
            }
            Dialog::Symbol { convert, name, kind, reg, folder } => {
                let folders = self.ed.library_folders();
                let convert = *convert;
                win(if convert { "Convert to Symbol" } else { "Create New Symbol" }).show(ctx, |ui| {
                    egui::Grid::new("sym").show(ui, |ui| {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(name)).request_focus();
                        ui.end_row();
                        ui.label("Type");
                        ui.horizontal(|ui| {
                            ui.selectable_value(kind, SymbolKind::Graphic, "Graphic");
                            ui.selectable_value(kind, SymbolKind::MovieClip, "Movie Clip");
                            ui.selectable_value(kind, SymbolKind::Button, "Button");
                        });
                        ui.end_row();
                        ui.label("Folder");
                        egui::ComboBox::from_id_salt("sym-folder").selected_text(if folder.is_empty() { "Library root" } else { folder.as_str() }).show_ui(ui, |ui| {
                            ui.selectable_value(folder, String::new(), "Library root");
                            for f in &folders {
                                ui.selectable_value(folder, f.clone(), f.as_str());
                            }
                        });
                        ui.end_row();
                        if convert {
                            ui.label("Registration");
                            registration_grid(ui, reg);
                            ui.end_row();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { symbol_ok = Some((convert, ac_core::join_item_path(folder, name.trim_start_matches('/')), *kind, *reg)); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::RenameSymbol { id, name } => {
                win("Rename Library Item").show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(name)).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { rename_ok = Some((*id, name.clone())); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::DeleteSymbol { id, uses } => {
                let name = self.ed.doc.symbol(*id).map(|s| s.name.clone()).unwrap_or_default();
                win("Delete Library Item").show(ctx, |ui| {
                    ui.label(format!("Delete “{name}”?"));
                    if *uses > 0 {
                        ui.label(egui::RichText::new(format!("{uses} instance(s) of it will also be removed from the document.")).color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() || enter { delete_ok = Some(*id); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::TraceBitmap { threshold, min_area, fit, corners, centerline, ink, uniform } => {
                win("Trace Bitmap").show(ctx, |ui| {
                    egui::Grid::new("tb").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Mode");
                        ui.horizontal(|ui| {
                            ui.selectable_value(centerline, false, "Outline").on_hover_text("Colour regions become fills");
                            ui.selectable_value(centerline, true, "Centerline").on_hover_text("Line art becomes lines along the middle of the ink, with its thickness (OpenToonz)");
                        });
                        ui.end_row();
                        if *centerline {
                            ui.label("Ink threshold");
                            ui.add(egui::Slider::new(ink, 1..=254)).on_hover_text("Pixels darker than this are ink");
                            ui.end_row();
                            ui.label("Line width");
                            ui.checkbox(uniform, "Uniform (ignore the ink's thickness)");
                            ui.end_row();
                        } else {
                            ui.label("Color threshold");
                            ui.add(egui::DragValue::new(threshold).range(0..=500));
                            ui.end_row();
                        }
                        ui.label("Minimum area");
                        ui.add(egui::DragValue::new(min_area).range(1..=1000).suffix(" pixels"));
                        ui.end_row();
                        ui.label("Curve fit");
                        egui::ComboBox::from_id_salt("tb-fit").selected_text(ac_core::trace::CurveFit::ALL[*fit].name()).show_ui(ui, |ui| {
                            for (k, c) in ac_core::trace::CurveFit::ALL.iter().enumerate() {
                                ui.selectable_value(fit, k, c.name());
                            }
                        });
                        ui.end_row();
                        ui.label("Corner threshold");
                        egui::ComboBox::from_id_salt("tb-corner").selected_text(ac_core::trace::Corners::ALL[*corners].name()).show_ui(ui, |ui| {
                            for (k, c) in ac_core::trace::Corners::ALL.iter().enumerate() {
                                ui.selectable_value(corners, k, c.name());
                            }
                        });
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter {
                            trace_ok = Some(ac_core::trace::TraceOpts {
                                color_threshold: *threshold,
                                min_area: *min_area,
                                curve_fit: ac_core::trace::CurveFit::ALL[*fit],
                                corners: ac_core::trace::Corners::ALL[*corners],
                                mode: if *centerline { ac_core::trace::TraceMode::Centerline } else { ac_core::trace::TraceMode::Outline },
                                line_threshold: *ink,
                                uniform_width: *uniform,
                            });
                            close = true;
                        }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::ScaleRotate { scale, rotate } => {
                win("Scale and Rotate").show(ctx, |ui| {
                    egui::Grid::new("sr").show(ui, |ui| {
                        ui.label("Scale");
                        ui.add(egui::DragValue::new(scale).range(1.0..=10000.0).suffix(" %"));
                        ui.end_row();
                        ui.label("Rotate");
                        ui.add(egui::DragValue::new(rotate).range(-360.0..=360.0).suffix("°"));
                        ui.end_row();
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { scale_rot = Some((*scale, *rotate)); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::RenameBitmap { id, name } => {
                win("Rename Bitmap").show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(name)).request_focus();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { bitmap_rename = Some((*id, name.clone())); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::DeleteBitmap { id, uses } => {
                win("Delete Library Item").show(ctx, |ui| {
                    ui.label("Delete this bitmap from the library?");
                    if *uses > 0 {
                        ui.label(RichText::new(format!("{uses} placed copy/copies will also be removed.")).color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() || enter { bitmap_delete = Some(*id); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::Unsaved(pending) => {
                let what = match pending { files::Pending::Quit => "quitting", files::Pending::New => "starting a new document", _ => "opening another document" };
                win("Unsaved changes").show(ctx, |ui| {
                    ui.label(format!("Save changes to “{}” before {what}?", self.doc_name()));
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() || enter { unsaved_choice = Some((pending.clone(), true)); close = true; }
                        if ui.button("Don't Save").clicked() { unsaved_choice = Some((pending.clone(), false)); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::Recover { original, age_secs } => {
                win("Recover unsaved work").show(ctx, |ui| {
                    ui.label("Flare didn't close normally last time.");
                    ui.label(format!("An autosave from {} is available{}.", files::fmt_age(*age_secs), original.as_ref().map(|o| format!(" for {o}")).unwrap_or_default()));
                    ui.horizontal(|ui| {
                        if ui.button("Restore").clicked() || enter { recover = Some(true); close = true; }
                        if ui.button("Discard").clicked() || esc { recover = Some(false); close = true; }
                    });
                });
            }
            Dialog::LayerProps { i, name, kind, parent, color, visible, locked, outline, height, filters } => {
                let li = *i;
                let containers: Vec<(usize, String)> = self.ed.timeline().layers.iter().enumerate().filter(|(j, l)| *j < li && matches!(l.kind, ac_core::LayerKind::Folder | ac_core::LayerKind::Mask | ac_core::LayerKind::Guide)).map(|(j, l)| (j, l.name.clone())).collect();
                win("Layer Properties").show(ctx, |ui| {
                    egui::Grid::new("lp").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.end_row();
                        ui.label("Show / Lock");
                        ui.horizontal(|ui| { ui.checkbox(visible, "Show"); ui.checkbox(locked, "Lock"); });
                        ui.end_row();
                        ui.label("Type");
                        ui.vertical(|ui| {
                            if *kind == ac_core::LayerKind::Camera {
                                ui.label("Camera");
                            } else {
                                ui.radio_value(kind, ac_core::LayerKind::Normal, "Normal");
                                ui.radio_value(kind, ac_core::LayerKind::Mask, "Mask (reveals its layers through its shapes)");
                                ui.radio_value(kind, ac_core::LayerKind::Guide, "Guide (not exported; motion guide for its layers)");
                                ui.radio_value(kind, ac_core::LayerKind::Folder, "Folder");
                            }
                        });
                        ui.end_row();
                        ui.label("Inside");
                        let cur = parent.and_then(|p| containers.iter().find(|c| c.0 == p).map(|c| c.1.clone())).unwrap_or("— (top level)".into());
                        egui::ComboBox::from_id_salt("lp-parent").selected_text(cur).show_ui(ui, |ui| {
                            ui.selectable_value(parent, None, "— (top level)");
                            for (j, n) in &containers {
                                ui.selectable_value(parent, Some(*j), n);
                            }
                        });
                        ui.end_row();
                        ui.label("Outline colour");
                        ui.horizontal(|ui| { ui.color_edit_button_srgba(color); ui.checkbox(outline, "View layer as outlines"); });
                        ui.end_row();
                        ui.label("Layer height");
                        ui.horizontal(|ui| {
                            for h in [100u16, 200, 300] {
                                ui.radio_value(height, h, format!("{h}%"));
                            }
                        });
                        ui.end_row();
                    });
                    ui.separator();
                    ui.label(RichText::new("Layer effects").strong());
                    ui.label(RichText::new("Filters applied to everything this layer draws, as one picture").small().color(Color32::from_gray(150)));
                    egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        panels::filter_list_ui(self, ui, filters, "layer-fx");
                    });
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() || enter { layer_ok = Some((li, name.clone(), *kind, *parent, *color, *visible, *locked, *outline, *height, filters.clone())); close = true; }
                        if ui.button("Cancel").clicked() || esc { close = true; }
                    });
                });
            }
            Dialog::About => {
                win("About Flare").show(ctx, |ui| {
                    ui.heading(format!("Flare {} (Rust)", env!("CARGO_PKG_VERSION")));
                    ui.label("Open-source 2D animation authoring in Rust, following the Flash / Animate workflow, with tools from OpenToonz.");
                    ui.label("The Rust conversion of Flare (github.com/Flare-Animate), built on the AnimationCraft engine.");
                    ui.label("Not affiliated with Adobe. Licensed MIT OR Apache-2.0; third-party notices in THIRD-PARTY-NOTICES.md.");
                    if ui.button("Close").clicked() || esc { close = true; }
                });
            }
        }
        if !close {
            self.dialog = d;
        }
        if path_cancelled { self.after_save = None; }
        if let Some(o) = trace_ok {
            let n = self.ed.trace_bitmap(&o);
            self.status = format!("Trace Bitmap: {n} fill region{}", if n == 1 { "" } else { "s" });
        }
        if let Some((id, n)) = bitmap_rename {
            // the dialog edits the item's own name; it stays in its folder
            let folder = self.ed.doc.bitmap(id).map(|b| ac_core::split_item_path(&b.name).0.to_string()).unwrap_or_default();
            self.ed.rename_bitmap(id, &ac_core::join_item_path(&folder, n.trim_start_matches('/')));
        }
        if let Some(id) = bitmap_delete { self.ed.delete_bitmap(id); self.lib.bitmap = None; }
        if let Some((i, name, kind, parent, color, visible, locked, outline, height, filters)) = layer_ok {
            let mark = self.ed.undo_depth();
            if self.ed.timeline().layers[i].name != name && !name.trim().is_empty() { self.ed.rename_layer(i, name.trim()); }
            self.ed.set_layer_kind(i, kind);
            self.ed.set_layer_parent(i, parent);
            self.ed.set_layer_look(i, from32(color), visible, locked, outline);
            self.ed.set_layer_row_height(i, height);
            self.ed.set_layer_filters(i, filters);
            self.ed.squash_undo(mark, "Layer Properties");
        }
        if let Some((a, p)) = run { self.do_file(a, &p); }
        if let Some((pending, save)) = unsaved_choice {
            if save {
                self.after_save = Some(pending);
                self.save();
            } else {
                self.saved_rev = self.ed.revision;
                self.run_pending(pending);
            }
        }
        match recover {
            Some(true) => self.restore_recovery(),
            Some(false) => files::clear_recovery(),
            None => {}
        }
        if let Some((convert, n, k, reg)) = symbol_ok {
            if convert {
                self.settings.symbol_reg = reg;
                self.settings.save();
                match self.ed.convert_to_symbol_at(&n, k, reg) {
                    Some(id) => { self.lib_selected = Some(id); self.status = format!("Converted to symbol “{n}”"); }
                    None => self.status = "Couldn't convert: nothing selected on an editable layer.".into(),
                }
            } else {
                let id = self.ed.new_symbol(&n, k);
                self.lib_selected = Some(id);
                self.fit_pending = true;
                self.status = format!("Editing new symbol “{n}”. Press Esc to return to the scene.");
            }
        }
        if let Some((id, n)) = rename_ok {
            let folder = self.ed.doc.symbol(id).map(|s| ac_core::split_item_path(&s.name).0.to_string()).unwrap_or_default();
            self.ed.rename_symbol(id, &ac_core::join_item_path(&folder, n.trim_start_matches('/')));
        }
        if let Some((sc, rot)) = scale_rot {
            if let Some(c) = self.pivot() {
                let k = sc / 100.0;
                let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(rot.to_radians())).concat(&Matrix::scale(k, k)).concat(&Matrix::translate(-c.x, -c.y));
                self.ed.modify_selected("Scale and Rotate", |e| e.matrix = m.concat(&e.matrix));
            }
        }
        if let Some(id) = delete_ok {
            self.ed.delete_symbol(id);
            self.lib_selected = None;
            self.status = "Library item deleted".into();
        }
    }
}

impl eframe::App for App {
    fn on_exit(&mut self) {
        // A clean exit leaves no recovery file behind.
        files::clear_recovery();
        self.save_layout();
    }

    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx_owned = root_ui.ctx().clone();
        let ctx: &egui::Context = &ctx_owned;
        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_confirmed && self.dirty() && self.screenshot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Dialog::Unsaved(files::Pending::Quit);
        }
        self.autosave_tick(ctx);
        // Files dropped onto the window are imported (or opened).
        let dropped: Vec<String> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().display().to_string()).collect());
        for p in dropped {
            match import::dropped(self, &p) {
                Ok(m) if !m.is_empty() => self.status = m,
                Ok(_) => {}
                Err(e) => self.status = format!("Error: {e}"),
            }
        }
        let title = format!("{}{} — Flare", self.doc_name(), if self.dirty() { " •" } else { "" });
        if title != self.title_shown {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title_shown = title;
        }
        dock::escape(self, ctx);
        palette::shortcut(self, ctx);
        if !self.palette.open {
            self.shortcuts(ctx);
        }
        if ctx.memory(|m| m.focused().is_none()) {
            scripting::shortcuts(self, ctx);
        }
        // Properties follows what you work on, like Animate.
        let cur = (self.ed.selection.clone(), self.ed.frame_sel);
        if cur != self.last_sel {
            if !cur.0.is_empty() {
                self.props_tab = PropsTab::Object;
            } else if cur.1.is_some() || self.props_tab == PropsTab::Object {
                self.props_tab = PropsTab::Frame;
            }
            self.last_sel = cur;
        }
        if self.playing {
            let dt = ctx.input(|i| i.stable_dt) as f64;
            self.play_accum += dt;
            let spf = 1.0 / self.ed.doc.fps as f64;
            let n = self.ed.frame_count();
            while self.play_accum >= spf {
                self.play_accum -= spf;
                // movie clips run on their own clock, even while the main timeline is held
                self.ed.clock = Some(self.ed.clock.unwrap_or(self.ed.frame) + 1);
                if self.halted {
                    continue;
                }
                match timeline::play_next(self.ed.frame, n, self.loop_play, self.tl.loop_range) {
                    Some((f, wrapped)) => {
                        self.ed.frame = f;
                        if wrapped {
                            self.restart_audio();
                        }
                    }
                    None => {
                        self.playing = false;
                        break;
                    }
                }
                scripting::enter_frame(self);
                if !self.playing {
                    break;
                }
            }
            // held with nothing left to animate: end playback as before
            if self.halted && self.ed.doc.movie_clip_lengths(self.ed.doc.scene_start(self.ed.scene) + self.ed.frame).is_empty() {
                self.playing = false;
            }
            ctx.request_repaint();
        }
        if !self.playing {
            self.halted = false;
            self.ed.clock = None;
        }
        self.audio_tick();
        self.menu_bar(root_ui);
        self.status_bar(root_ui);
        options_bar::bar(self, root_ui);
        timeline::panel(self, root_ui);
        toolbar::panel(self, root_ui);
        swatches::panel(self, root_ui);
        panels::library_column(self, root_ui);
        panels::right_panel(self, root_ui);
        stage::stage(self, root_ui);
        panels::library_drag_ghost(self, ctx);
        dock::drop_targets(self, ctx);
        navigator::window(self, ctx);
        history::window(self, ctx);
        windows::shape_window(self, ctx);
        windows::doc_settings_window(self, ctx);
        panels::item_props_window(self, ctx);
        prefs::window(self, ctx);
        color_panel::window(self, ctx);
        palette::window(self, ctx);
        self.dialogs(ctx);
        startup::window(self, ctx);
        publish::window(self, ctx);
        publish::image_window(self, ctx);
        scripting::windows(self, ctx);
        panels::transform_window(self, ctx);
        self.ed.multi_frame = self.onion_opts.multi.then(|| self.onion_range());
        windows::align_window(self, ctx);
        motion_ui::motion_editor(self, ctx);
        presets::window(self, ctx);
        windows::scene_window(self, ctx);
        rulers::edit_grid_window(self, ctx);
        rulers::edit_guides_window(self, ctx);
        windows::info_window(self, ctx);

        // Screenshot hook (docs/CI): wait a few frames for layout to settle.
        if let Some((path, n)) = &mut self.screenshot {
            *n += 1;
            if *n == 6 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
            let shot = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
            if let Some(img) = shot {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(pm) = tiny_skia::Pixmap::from_vec(bytes, tiny_skia::IntSize::from_wh(w as u32, h as u32).unwrap()) {
                    let _ = pm.save_png(path.as_str());
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }
}

/// The window / taskbar icon (the same artwork the Windows exe embeds).
fn app_icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("assets/icon/flare.png")).map(|i| i.to_rgba8());
    match img {
        Ok(i) => egui::IconData { width: i.width(), height: i.height(), rgba: i.into_raw() },
        Err(_) => egui::IconData::default(),
    }
}

fn main() -> eframe::Result<()> {
    let shot = std::env::var("AC_SCREENSHOT").is_ok();
    // Start maximised so the panels fit on any screen size / display scaling.
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([800.0, 520.0])
        .with_maximized(!shot)
        .with_title("Flare")
        .with_icon(app_icon());
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Flare", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

/// The 3 × 3 registration grid of Convert to Symbol: where the symbol's
/// origin (its registration point) goes on the selection's bounds.
pub(crate) fn registration_grid(ui: &mut egui::Ui, reg: &mut [u8; 2]) {
    let cell = 12.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell * 3.0 + 4.0, cell * 3.0 + 4.0), egui::Sense::click());
    let p = ui.painter();
    p.rect_stroke(rect, 1.0, egui::Stroke::new(1.0_f32, Color32::from_gray(110)), egui::StrokeKind::Inside);
    for r in 0..3u8 {
        for c in 0..3u8 {
            let ctr = rect.min + egui::vec2(2.0 + cell * (c as f32 + 0.5), 2.0 + cell * (r as f32 + 0.5));
            let on = *reg == [c, r];
            p.rect_filled(egui::Rect::from_center_size(ctr, egui::vec2(6.0, 6.0)), 0.0, if on { Color32::BLACK } else { Color32::from_gray(200) });
            if on {
                p.rect_stroke(egui::Rect::from_center_size(ctr, egui::vec2(7.0, 7.0)), 0.0, egui::Stroke::new(1.0_f32, Color32::WHITE), egui::StrokeKind::Inside);
            }
        }
    }
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked()) {
        let v = pos - rect.min;
        *reg = [((v.x - 2.0) / cell).clamp(0.0, 2.0) as u8, ((v.y - 2.0) / cell).clamp(0.0, 2.0) as u8];
    }
    resp.on_hover_text("Where the symbol's registration point (its origin) goes on the selection");
}
