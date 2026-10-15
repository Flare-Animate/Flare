use eframe::egui::{self, Color32, FontId, Stroke, TextStyle};

/// Shared size for UI labels, controls, tabs and inspector section titles.
// Reference chrome is about 15 device pixels at 125% display scaling.
pub const UI_TEXT_SIZE: f32 = 12.0;
pub const TAB_HEIGHT: f32 = 20.0;
pub const PANEL_TAB_HEIGHT: f32 = 17.6;
pub const CONTROL_HEIGHT: f32 = 18.4;
pub const ICON_BUTTON_SIZE: f32 = 24.0;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub chrome: Chrome,
    pub wordmark: Color32,
    pub back: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub select: Color32,
    pub playhead: Color32,
    pub timeline: Color32,
    pub frame: Color32,
    pub frame_edge: Color32,
    pub frame_bevel: Color32,
    pub tween: Color32,
    pub tab: Color32,
    pub tab_rail: Color32,
    pub field: Color32,
    pub field_border: Color32,
    pub scroll_track: Color32,
    pub scroll_track_end: Color32,
    pub divider_bevel: Color32,
}
impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Self::Dark => Palette {
                chrome: Chrome::dark(),
                wordmark: Color32::from_rgb(168, 98, 78),
                back: Color32::from_gray(69),
                panel: Color32::from_gray(70),
                raised: Color32::from_gray(83),
                line: Color32::from_gray(46),
                text: Color32::from_gray(195),
                muted: Color32::from_gray(166),
                // Flash Dark ControlFocus is HSV (44.35, .8106, .8902),
                // rounded to #E3B32B. Flare shifts red by one step.
                accent: Color32::from_rgb(228, 179, 43),
                select: Color32::from_rgb(156, 120, 29),
                playhead: Color32::from_rgb(167, 7, 7),
                timeline: Color32::from_gray(57),
                frame: Color32::from_gray(71),
                frame_edge: Color32::from_gray(51),
                frame_bevel: Color32::from_gray(79),
                tween: Color32::from_rgb(86, 78, 111),
                tab: Color32::from_gray(111),
                tab_rail: Color32::from_gray(44),
                field: Color32::from_gray(161),
                field_border: Color32::from_gray(46),
                // Recessed rail sits below the raised scrollbar thumb.
                scroll_track: Color32::from_gray(57),
                scroll_track_end: Color32::from_gray(57),
                divider_bevel: Color32::from_gray(131),
            },
            Self::Light => Palette {
                chrome: Chrome::light(),
                wordmark: Color32::from_rgb(86, 58, 50),
                back: Color32::from_gray(215),
                panel: Color32::from_gray(206),
                raised: Color32::from_gray(210),
                line: Color32::from_gray(90),
                text: Color32::from_gray(50),
                muted: Color32::from_gray(131),
                // Flash CC 2015 Light theme ControlFocus is HSV
                // (204.95, 1.0, .8392) = #007DD6. Keep Flare one RGB step away.
                accent: Color32::from_rgb(1, 125, 214),
                select: Color32::from_rgb(147, 187, 234),
                playhead: Color32::from_rgb(181, 21, 21),
                timeline: Color32::from_gray(197),
                frame: Color32::from_gray(217),
                frame_edge: Color32::from_gray(186),
                frame_bevel: Color32::from_gray(225),
                tween: Color32::from_rgb(180, 174, 215),
                tab: Color32::from_gray(190),
                // Darker workspace behind raised document and dock tabs.
                tab_rail: Color32::from_gray(150),
                field: Color32::from_gray(238),
                field_border: Color32::from_gray(90),
                // Flash's light rail is darker than its raised thumb; shift
                // the inferred groove and edge one step toward Flare's palette.
                scroll_track: Color32::from_gray(189),
                scroll_track_end: Color32::from_gray(189),
                divider_bevel: Color32::from_gray(211),
            },
        }
    }
}

/// Numeric reference observations are documented in docs/UI_MATERIALS.md.
/// Reference grays are shifted by one RGB step. All painting is native Flare
/// code; no Adobe stylesheet, image, font or shader is embedded.
#[derive(Clone, Copy)]
pub struct Chrome {
    pub button: [u8; 2],
    pub hover: [u8; 2],
    pub pressed: [u8; 2],
    pub tab: [u8; 2],
    pub tab_hover: [u8; 2],
    pub tab_inactive: [u8; 2],
    pub tab_normal_stroke: u8,
    pub tab_over_stroke: u8,
    pub tab_inactive_stroke: u8,
    pub bar: [u8; 2],
    pub border: u8,
    pub highlight: u8,
    pub shadow: u8,
}
impl Chrome {
    const fn light() -> Self {
        Self {
            // Flash Light ControlFill* and Tab* roles + one RGB step.
            button: [210, 190],
            hover: [220, 200],
            pressed: [190, 166],
            tab: [206, 190],
            tab_hover: [230, 206],
            // TabDeactiveStroke is the stroke (#A4), not a gradient stop;
            // the mapped start/end fills are #9A and #8D.
            tab_inactive: [154, 141],
            // Tab*Stroke roles, shifted one RGB step from Flash CC.
            tab_normal_stroke: 211,
            tab_over_stroke: 220,
            tab_inactive_stroke: 164,
            bar: [217, 192],
            border: 90,
            highlight: 230,
            shadow: 90,
        }
    }
    const fn dark() -> Self {
        Self {
            // Flash Dark ControlFill* and Tab* roles + one RGB step.
            button: [83, 64],
            hover: [99, 80],
            pressed: [60, 77],
            tab: [126, 111],
            tab_hover: [151, 126],
            tab_inactive: [82, 71],
            tab_normal_stroke: 125,
            tab_over_stroke: 150,
            tab_inactive_stroke: 82,
            bar: [85, 65],
            border: 29,
            highlight: 110,
            shadow: 34,
        }
    }
}

pub fn gradient(rect: egui::Rect, top: Color32, bottom: Color32) -> egui::Shape {
    let mut mesh = egui::epaint::Mesh::default();
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    egui::Shape::mesh(mesh)
}

/// Bevels are painted inside the preallocated rectangle, at device-pixel width.
pub fn bevel(painter: &egui::Painter, rect: egui::Rect, p: Palette, hovered: bool, pressed: bool) {
    let c = p.chrome;
    let [top, bottom] = if pressed {
        c.pressed
    } else if hovered {
        c.hover
    } else {
        c.button
    };
    painter.add(gradient(
        rect,
        Color32::from_gray(top),
        Color32::from_gray(bottom),
    ));
    bevel_edges(painter, rect, p, pressed);
}

/// Snap material edges to the physical pixel grid without antialiased stroke fringes.
fn pixel_rect(painter: &egui::Painter, rect: egui::Rect) -> egui::Rect {
    let scale = painter.ctx().pixels_per_point();
    egui::Rect::from_min_max(
        (rect.min.to_vec2() * scale).round().to_pos2() / scale,
        (rect.max.to_vec2() * scale).round().to_pos2() / scale,
    )
}

/// Consecutive one-device-pixel bands, bounded by the surface they belong to.
/// `far` starts at the bottom/right and proceeds inward.
pub fn edge_bands(
    painter: &egui::Painter,
    rect: egui::Rect,
    vertical: bool,
    far: bool,
    shades: &[u8],
) {
    let r = pixel_rect(painter, rect);
    let pixel = 1.0 / painter.ctx().pixels_per_point();
    let painter = painter.with_clip_rect(painter.clip_rect().intersect(r));
    for (i, shade) in shades.iter().enumerate() {
        let mut band = r;
        let offset = i as f32 * pixel;
        if vertical {
            band.min.x = if far {
                r.right() - offset - pixel
            } else {
                r.left() + offset
            };
            band.max.x = band.min.x + pixel;
        } else {
            band.min.y = if far {
                r.bottom() - offset - pixel
            } else {
                r.top() + offset
            };
            band.max.y = band.min.y + pixel;
        }
        let color = Color32::from_gray(*shade);
        painter.add(gradient(band, color, color));
    }
}

pub fn bevel_edges(painter: &egui::Painter, rect: egui::Rect, p: Palette, pressed: bool) {
    let c = p.chrome;
    let r = pixel_rect(painter, rect);
    let light = if pressed { c.shadow } else { c.highlight };
    for vertical in [false, true] {
        edge_bands(painter, r, vertical, false, &[c.border, light]);
        edge_bands(painter, r, vertical, true, &[c.border]);
    }
}

pub fn panel_bar(painter: &egui::Painter, rect: egui::Rect, p: Palette) {
    let r = pixel_rect(painter, rect);
    painter.add(gradient(
        r,
        Color32::from_gray(p.chrome.bar[0]),
        Color32::from_gray(p.chrome.bar[1]),
    ));
    // Section seams are not button bevels: dark at the top, then the face.
    // These profiles are screenshot measurements, not a port of a draw routine.
    let top: &[u8] = if p.back.r() > 128 {
        &[194, 101, 171, 211, 217]
    } else {
        &[64, 14, 51, 77, 85]
    };
    edge_bands(painter, r, false, false, top);
    let bottom: &[u8] = if p.back.r() > 128 {
        &[194, 101, 171, 200, 194]
    } else {
        &[64, 14, 51, 67, 65]
    };
    edge_bands(painter, r, false, true, bottom);
}

/// Recessed scrollbar edges run across the rail, never along its travel axis.
pub fn scroll_groove(painter: &egui::Painter, rect: egui::Rect, vertical: bool, p: Palette) {
    let r = pixel_rect(painter, rect);
    painter.add(gradient(r, p.scroll_track, p.scroll_track_end));
    let (near, far): (&[u8], &[u8]) = if p.back.r() > 128 {
        (&[167, 134, 171, 187], &[112, 144, 175])
    } else {
        (&[48, 34, 52, 56], &[29, 39, 53])
    };
    edge_bands(painter, r, vertical, false, near);
    edge_bands(painter, r, vertical, true, far);
}

/// Narrow dock grooves use their own material, not a raised button outline.
pub fn dock_edges(painter: &egui::Painter, rect: egui::Rect, p: Palette) {
    let (near, far): (&[u8], &[u8]) = if p.back.r() > 128 {
        (&[98, 118, 103, 151], &[151, 103, 118, 98, 173])
    } else {
        (&[34, 48, 37, 51], &[51, 37, 48, 34, 59])
    };
    edge_bands(painter, rect, true, false, near);
    // `far` advances from the right inward. Reverse the reference's left-to-
    // right profile here so the shoulder faces the dock instead of its content.
    edge_bands(painter, rect, true, true, far);
}

/// A scrollbar thumb is a flat raised strip, not the button gradient.
pub fn scroll_thumb(
    painter: &egui::Painter,
    rect: egui::Rect,
    p: Palette,
    hovered: bool,
    pressed: bool,
) {
    let r = pixel_rect(painter, rect);
    let light = p.back.r() > 128;
    let base: u8 = if light { 211 } else { 67 };
    let face = if pressed {
        base.saturating_sub(5)
    } else if hovered {
        base.saturating_add(5)
    } else {
        base
    };
    let color = Color32::from_gray(face);
    painter.add(gradient(r, color, color));
    let (near, far): (&[u8], &[u8]) = if light {
        (&[124, 221, 214], &[124, 199])
    } else {
        (&[25, 80, 77], &[25, 58])
    };
    for vertical in [false, true] {
        edge_bands(painter, r, vertical, false, near);
        edge_bands(painter, r, vertical, true, far);
    }
}

/// Arrow cells continue the recessed rail; only interaction changes the face.
pub fn scroll_arrow_face(
    painter: &egui::Painter,
    rect: egui::Rect,
    p: Palette,
    hovered: bool,
    pressed: bool,
) {
    let r = pixel_rect(painter, rect);
    let base = if p.back.r() > 128 { 198u8 } else { 59u8 };
    let face = if pressed {
        base.saturating_sub(8)
    } else if hovered {
        base.saturating_add(8)
    } else {
        base
    };
    let color = Color32::from_gray(face);
    painter.add(gradient(r, color, color));
}

/// Entry fields have an inset rim with a bright exterior bottom/right shoulder.
pub fn field_edges(painter: &egui::Painter, rect: egui::Rect, p: Palette) {
    let (near, far): (&[u8], &[u8]) = if p.back.r() > 128 {
        (&[109, 170], &[227, 112])
    } else {
        (&[23, 48], &[94, 24])
    };
    for vertical in [false, true] {
        edge_bands(painter, rect, vertical, false, near);
        edge_bands(painter, rect, vertical, true, far);
    }
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let c = theme.palette();
    let mut visuals = match theme {
        Theme::Dark => egui::Visuals::dark(),
        Theme::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = c.panel;
    visuals.window_fill = c.panel;
    visuals.extreme_bg_color = c.field;
    // Ghidra's TextEditNormalBackground maps to this field only while editing.
    visuals.text_edit_bg_color = Some(c.field);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, c.field_border);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, c.accent);
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, c.accent);
    visuals.override_text_color = Some(c.text);
    visuals.selection.bg_fill = c.select;
    visuals.selection.stroke = Stroke::new(1.0, c.accent);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, c.line);
    visuals.widgets.inactive.weak_bg_fill = c.raised;
    visuals.widgets.hovered.weak_bg_fill = c.raised;
    visuals.widgets.active.weak_bg_fill = c.select;
    visuals.window_corner_radius = egui::CornerRadius::same(3);
    visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
        // Hover, focus, press and open states may change colors, never geometry.
        widget.expansion = 0.0;
        widget.bg_stroke.width = 1.0;
    }
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(UI_TEXT_SIZE));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(UI_TEXT_SIZE));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.0));
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(UI_TEXT_SIZE, egui::FontFamily::Name("Semibold".into())),
        );
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 1.0);
        style.spacing.interact_size.y = CONTROL_HEIGHT;
    });
}

#[derive(Clone, Copy)]
pub enum Icon {
    Select,
    Rectangle,
    Ellipse,
    Hand,
    Play,
    Pause,
    First,
    Last,
    Previous,
    Next,
    Plus,
    Key,
    Settings,
    Eye,
    EyeOff,
    Lock,
    Layers,
    Search,
    Zoom,
    Palette,
    Ruler,
    Undo,
    Redo,
    Close,
    Down,
    Monitor,
    Symbol,
    File,
    Check,
    Trash,
}
impl Icon {
    fn asset(self) -> (&'static str, &'static [u8]) {
        macro_rules! asset {
            ($name:literal) => {
                (
                    $name,
                    include_bytes!(concat!("assets/icons/", $name, ".svg")),
                )
            };
        }
        match self {
            Self::Select => asset!("mouse-pointer-2"),
            Self::Rectangle => asset!("square"),
            Self::Ellipse => asset!("circle"),
            Self::Hand => asset!("hand"),
            Self::Play => asset!("play"),
            Self::Pause => asset!("pause"),
            Self::First | Self::Previous => asset!("chevron-left"),
            Self::Last | Self::Next => asset!("chevron-right"),
            Self::Plus => asset!("plus"),
            Self::Key => asset!("circle-dot"),
            Self::Settings => asset!("sliders-horizontal"),
            Self::Eye => asset!("eye"),
            Self::EyeOff => asset!("eye-off"),
            Self::Lock => asset!("lock"),
            Self::Layers => asset!("layers"),
            Self::Search => asset!("search"),
            Self::Zoom => asset!("zoom-in"),
            Self::Palette => asset!("palette"),
            Self::Ruler => asset!("ruler"),
            Self::Undo => asset!("undo-2"),
            Self::Redo => asset!("redo-2"),
            Self::Close => asset!("x"),
            Self::Down => asset!("chevron-down"),
            Self::Monitor => asset!("monitor"),
            Self::Symbol => asset!("diamond"),
            Self::File => asset!("file"),
            Self::Check => asset!("check"),
            Self::Trash => asset!("trash"),
        }
    }
}
/// Source vectors are converted once to white, then tinted for each semantic role.
pub fn paint_icon(ui: &egui::Ui, rect: egui::Rect, icon: Icon, color: Color32) {
    // Original geometric glyphs: classic tool/transport marks have a solid
    // silhouette, unlike the outline-only general-purpose SVG set.
    let point = |x: f32, y: f32| rect.min + egui::vec2(x * rect.width(), y * rect.height());
    if matches!(icon, Icon::Select) {
        let mut mesh = egui::epaint::Mesh::default();
        for (x, y) in [
            (0.18, 0.05),
            (0.18, 0.87),
            (0.38, 0.65),
            (0.56, 0.96),
            (0.70, 0.87),
            (0.52, 0.57),
            (0.87, 0.57),
        ] {
            mesh.colored_vertex(point(x, y), color);
        }
        for triangle in [[0, 1, 2], [0, 2, 5], [0, 5, 6], [2, 3, 4], [2, 4, 5]] {
            mesh.add_triangle(triangle[0], triangle[1], triangle[2]);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
        return;
    }
    if matches!(
        icon,
        Icon::Play | Icon::Previous | Icon::Next | Icon::First | Icon::Last
    ) {
        let reverse = matches!(icon, Icon::Previous | Icon::First);
        let (tip, base) = if reverse { (0.25, 0.75) } else { (0.75, 0.25) };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![point(tip, 0.5), point(base, 0.2), point(base, 0.8)],
            color,
            Stroke::NONE,
        ));
        if matches!(icon, Icon::First | Icon::Last) {
            let x = if reverse { 0.12 } else { 0.88 };
            ui.painter()
                .line_segment([point(x, 0.2), point(x, 0.8)], Stroke::new(1.5, color));
        }
        return;
    }
    if matches!(icon, Icon::Pause) {
        for x in [0.25, 0.60] {
            ui.painter().rect_filled(
                egui::Rect::from_min_max(point(x, 0.2), point(x + 0.15, 0.8)),
                0.0,
                color,
            );
        }
        return;
    }
    if matches!(icon, Icon::Rectangle | Icon::Ellipse) {
        let face = color.gamma_multiply(0.35);
        let shape = if matches!(icon, Icon::Rectangle) {
            egui::Shape::rect_filled(rect.shrink(2.0), 0.0, face)
        } else {
            egui::Shape::circle_filled(rect.center(), rect.width() * 0.4, face)
        };
        ui.painter().add(shape);
    }
    let (name, bytes) = icon.asset();
    let key = egui::Id::new(("studio.icon", name));
    let svg = ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_insert_with(key, || {
            std::sync::Arc::<[u8]>::from(
                String::from_utf8_lossy(bytes)
                    .replace("currentColor", "#ffffff")
                    .replace("stroke-width=\"2\"", "stroke-width=\"1.75\"")
                    .into_bytes(),
            )
        })
        .clone()
    });
    egui::Image::from_bytes(format!("bytes://flare/icons/{name}.svg"), svg)
        .tint(color)
        .paint_at(ui, rect);
    if matches!(icon, Icon::First | Icon::Last) {
        let x = if matches!(icon, Icon::First) {
            rect.left() + 2.0
        } else {
            rect.right() - 2.0
        };
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.top() + 4.0),
                egui::pos2(x, rect.bottom() - 4.0),
            ],
            Stroke::new(1.3, color),
        );
    }
}
pub fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, tip: &str) -> egui::Response {
    let p = if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
    .palette();
    icon_button_with_selection(
        ui,
        icon,
        selected,
        tip,
        p.accent,
        p.accent.gamma_multiply(0.6),
        egui::Vec2::splat(ICON_BUTTON_SIZE),
        false,
    )
}

/// Tool/ruler selection uses neutral gray; blue remains reserved for focus and
/// numeric hot text where the reference UI uses it.
pub fn neutral_icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let p = if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
    .palette();
    icon_button_with_selection(
        ui,
        icon,
        selected,
        tip,
        p.text,
        Color32::from_gray(p.chrome.tab_inactive_stroke),
        egui::Vec2::splat(ICON_BUTTON_SIZE),
        false,
    )
}

/// Tool docks reserve a wider neutral selection frame than scene toolbar buttons.
pub fn neutral_tool_button(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    tip: &str,
) -> egui::Response {
    let p = if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
    .palette();
    icon_button_with_selection(
        ui,
        icon,
        selected,
        tip,
        p.text,
        p.line,
        egui::vec2(28.8, ICON_BUTTON_SIZE),
        true,
    )
}

fn icon_button_with_selection(
    ui: &mut egui::Ui,
    icon: Icon,
    selected: bool,
    tip: &str,
    selected_icon: Color32,
    selected_outline: Color32,
    size: egui::Vec2,
    tool_material: bool,
) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let p = if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
    .palette();
    if selected || response.hovered() {
        if selected && tool_material {
            let light = p.back.r() > 128;
            let face = if light { [180, 159] } else { [49, 49] };
            ui.painter().add(gradient(
                r,
                Color32::from_gray(face[0]),
                Color32::from_gray(face[1]),
            ));
            edge_bands(
                ui.painter(),
                r,
                false,
                false,
                if light {
                    &[147, 109, 157, 176]
                } else {
                    &[39, 19, 44, 48]
                },
            );
            edge_bands(
                ui.painter(),
                r,
                false,
                true,
                if light {
                    &[221, 158, 105, 143]
                } else {
                    &[78, 44, 18, 43]
                },
            );
            for far in [false, true] {
                edge_bands(
                    ui.painter(),
                    r,
                    true,
                    far,
                    if light {
                        &[129, 118, 153]
                    } else {
                        &[28, 25, 45]
                    },
                );
            }
        } else {
            bevel(
                ui.painter(),
                r,
                p,
                response.hovered(),
                selected || response.is_pointer_button_down_on(),
            );
        }
        if selected && !tool_material {
            ui.painter().rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, selected_outline),
                egui::StrokeKind::Inside,
            );
        }
    }
    paint_icon(
        ui,
        egui::Rect::from_center_size(r.center(), egui::Vec2::splat(17.0)),
        icon,
        if !ui.is_enabled() {
            p.muted.gamma_multiply(0.45)
        } else if selected {
            selected_icon
        } else {
            p.text
        },
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    response.on_hover_text(tip)
}

/// Use the platform UI face without redistributing Windows font files.
/// Egui's bundled fonts remain fallbacks on other platforms and for missing glyphs.
pub fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "JetBrains Mono".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "assets/fonts/JetBrainsMono-Regular.ttf"
        ))),
    );
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .unwrap()
        .insert(0, "JetBrains Mono".into());
    if cfg!(target_os = "windows") {
        let root = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:/Windows"))
            .join("Fonts");
        for (name, file) in [
            ("Segoe UI", "segoeui.ttf"),
            ("Segoe UI Semibold", "seguisb.ttf"),
        ] {
            if let Ok(bytes) = std::fs::read(root.join(file)) {
                fonts.font_data.insert(
                    name.into(),
                    std::sync::Arc::new(FontData::from_owned(bytes)),
                );
            }
        }
        if fonts.font_data.contains_key("Segoe UI") {
            fonts
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, "Segoe UI".into());
        }
    }
    for (family, face) in [("Medium", "Segoe UI"), ("Semibold", "Segoe UI Semibold")] {
        let mut fallback = fonts.families[&FontFamily::Proportional].clone();
        if fonts.font_data.contains_key(face) {
            fallback.insert(0, face.into());
        }
        fonts
            .families
            .insert(FontFamily::Name(family.into()), fallback);
    }
    ctx.set_fonts(fonts);
}
pub fn medium(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("Medium".into()))
}
