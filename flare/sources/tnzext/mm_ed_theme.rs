//! Animate's default dark UI: palette, metrics, fonts and shared panel chrome.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Response, Sense, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, vec2,
};

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

// Palette: Animate 2024's default Dark theme (docs/animate-ui.md §2).
/// The window behind everything: the gaps between docked groups and its edge lines.
pub const APP_BG: Color32 = rgb(0x252525);
pub const PANEL_BG: Color32 = rgb(0x323232);
/// App bar and document tab row.
pub const BAR: Color32 = rgb(0x2F2F2F);
/// Panel tab strips.
pub const PANEL_HEADER: Color32 = rgb(0x2F2F2F);
pub const INSET: Color32 = rgb(0x262626);
/// Hairlines: borders, row separators, section dividers, timeline grid lines.
pub const BORDER: Color32 = rgb(0x252525);
/// The vertical lines between button groups.
pub const GROUP_DIVIDER: Color32 = rgb(0x545454);
/// Outlines of fields and push buttons.
pub const OUTLINE: Color32 = rgb(0x4F4F4F);
/// Outlines of popups and floating windows.
pub const POPUP_BORDER: Color32 = rgb(0x5A5A5A);
/// Our own dark popups (tool flyouts, tool-option menus, the app bar's popovers), their hovered rows and separators.
pub const POPUP_BG: Color32 = rgb(0x1E1E1E);
pub const POPUP_HOVER: Color32 = rgb(0x2C2C2C);
pub const POPUP_SEPARATOR: Color32 = rgb(0x3F3F3F);
/// The check mark in dark popups, on a flyout's current tool.
pub const ACCENT_HOVER: Color32 = rgb(0x378EF0);
pub const TEXT: Color32 = rgb(0xEBEBEB);
pub const TEXT_DIM: Color32 = rgb(0xA5A5A5);
pub const TEXT_DISABLED: Color32 = rgb(0x707070);
/// Glyphs of tools that can't be used (not built yet), and Swap Colors while a color is none.
pub const ICON_DISABLED: Color32 = rgb(0x555555);
/// The red diagonal of a "no color" swatch.
pub const NO_COLOR: Color32 = rgb(0xE0302A);
/// The badge on tools that can't be used.
pub const WARNING: Color32 = rgb(0xF5C400);
/// Push buttons at rest (icon buttons are flat).
pub const BUTTON: Color32 = rgb(0x2E2E2E);
pub const BUTTON_HOVER: Color32 = rgb(0x3F3F3F);
/// Pressed buttons, the active tool and every toggled-on button.
pub const BUTTON_ACTIVE: Color32 = rgb(0x545454);
pub const TOGGLED: Color32 = BUTTON_ACTIVE;
pub const SLIDER_TRACK: Color32 = rgb(0x3F3F3F);
pub const ACCENT: Color32 = rgb(0x2680EB);
/// Primary buttons (New Document's Create, a dialog's OK): white text on blue.
pub const CTA: Color32 = rgb(0x1473E6);
pub const CTA_HOVER: Color32 = rgb(0x0D66D0);
pub const CTA_DOWN: Color32 = rgb(0x095ABA);
/// The window's and dialogs' close box under the pointer, as Windows 11 draws it.
pub const CLOSE_HOVER: Color32 = rgb(0xC42B1C);
/// Selected rows in lists.
pub const SELECTION: Color32 = rgb(0x40618E);
pub const PASTEBOARD: Color32 = rgb(0x252525);
pub const STAGE_OUTLINE: Color32 = rgb(0x000000);
pub const FRAME_CELL: Color32 = rgb(0x2F2F2F);
pub const FRAME_CELL_5TH: Color32 = rgb(0x3A3A3A);
/// Frames holding content, and frames after a blank keyframe.
pub const FRAME_SPAN: Color32 = rgb(0x909090);
pub const FRAME_SPAN_BLANK: Color32 = rgb(0x4A4A4A);
pub const FRAME_SPAN_EDGE: Color32 = rgb(0x252525);
pub const KEYFRAME: Color32 = rgb(0x1E1E1E);
pub const CLASSIC_TWEEN: Color32 = rgb(0x78609E);
pub const SHAPE_TWEEN: Color32 = rgb(0xAE6F42);
pub const MOTION_TWEEN: Color32 = rgb(0x9C8B43);
pub const PLAYHEAD: Color32 = rgb(0x4A80CC);
pub const LAYER_SELECTED: Color32 = rgb(0x40618E);
pub const LAYER_HOVER: Color32 = rgb(0x393939);
pub const LAYER_NAME: Color32 = rgb(0xD5D5D5);
/// Ruler numbers and seconds.
pub const RULER_TEXT: Color32 = rgb(0xA5A5A5);
/// The current-frame number in the timeline toolbar.
pub const CURRENT_FRAME_TEXT: Color32 = rgb(0x8FB8E8);
/// Selected frames: opaque on empty cells, about half over spans.
pub const FRAME_SELECTED: Color32 = rgb(0x5A8AD0);
/// Box around selected objects on the stage (every object type, until Highlight
/// color preferences exist).
pub const SELECTION_BOX: Color32 = rgb(0x3FA9F5);
/// The marquee and the lasso: 1 px black, as the spec reads Animate (§1.0),
/// on a 3 px light rim of our own so they show on the dark pasteboard too.
// ponytail: §7.1 check 4 (Animate 24 on the pasteboard) is still open; redo this after it.
pub const MARQUEE: Color32 = rgb(0x000000);
/// White at 160/255, premultiplied.
pub const MARQUEE_RIM: Color32 = Color32::from_rgba_premultiplied(160, 160, 160, 160);
/// Scrollbar thumbs: 7 px pills with no visible track.
pub const SCROLLBAR_THUMB: Color32 = rgb(0xD1D1D1);
/// Ruler guides: green by default; the hex is a placeholder.
pub const GUIDE: Color32 = rgb(0x33CC66);
/// Grid lines; the grey is a placeholder.
pub const GRID: Color32 = rgb(0x949494);
/// Onion skin colors for frames before and after the playhead.
pub const ONION_BEFORE: Color32 = rgb(0x2F6FFF);
pub const ONION_AFTER: Color32 = rgb(0x00FF00);
/// Frame label marks: names, comments and anchors.
pub const LABEL_NAME: Color32 = rgb(0xD62D2D);
pub const LABEL_COMMENT: Color32 = rgb(0x2E9B3E);
pub const LABEL_ANCHOR: Color32 = rgb(0xD8A21C);
// Properties and Library (docs/animate-ui.md §7.1).
/// The selected Tool / Object / Frame / Doc tab.
pub const CATEGORY_PILL: Color32 = rgb(0x4A4A4A);
/// The line under a scrubby number.
pub const FIELD_UNDERLINE: Color32 = rgb(0x474747);
/// Disabled dropdowns: a flat fill, no outline.
pub const DROPDOWN_DISABLED: Color32 = rgb(0x3D3D3D);
/// Section titles, semibold.
pub const SECTION_TITLE: Color32 = rgb(0xC8C8C8);
pub const CHECKBOX_BORDER: Color32 = rgb(0x9A9A9A);
pub const CHECKBOX_ON: Color32 = rgb(0xC8C8C8);
/// Slider knobs: hollow rings.
pub const SLIDER_KNOB: Color32 = rgb(0xE0E0E0);
/// A filter's card in the Filters section: a little darker than the panel, outlined.
pub const FILTER_CARD: Color32 = rgb(0x2B2B2B);
pub const FILTER_CARD_BORDER: Color32 = rgb(0x474747);
/// The red diagonal of "no color".
pub const NO_COLOR_SLASH: Color32 = rgb(0xE0302A);
/// Outline colors Animate gives layers as they're added (Layer_1 is cyan),
/// used when a layer doesn't carry its own.
pub const LAYER_COLORS: [Color32; 8] = [
    rgb(0x00FFFF),
    rgb(0x9933CC),
    rgb(0xFF800A),
    rgb(0xFF4FFF),
    rgb(0x4FFF4F),
    rgb(0x4F80FF),
    rgb(0xFF0000),
    rgb(0x669900),
];

// Metrics, in points.
pub const APP_BAR_H: f32 = 30.0;
pub const DOC_TABS_H: f32 = 26.0;
pub const TAB_STRIP_H: f32 = 22.0;
/// The Tools column inside its 1 px outer edge line.
pub const TOOLS_W: f32 = 48.0;
/// Tool buttons, on a 30 px pitch.
pub const TOOL_BUTTON: Vec2 = vec2(32.0, 26.0);
pub const ICON_BUTTON: f32 = 22.0;

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // Animate's own UI font isn't ours to ship; Segoe UI is the closest stock Windows face.
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\segoeui.ttf")) {
        fonts.font_data.insert("ui".into(), FontData::from_owned(bytes).into());
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "ui".into());
    }
    // Semibold for headings and tile names; without Segoe UI it falls back to regular.
    let mut semibold = fonts.families[&FontFamily::Proportional].clone();
    if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\seguisb.ttf")) {
        fonts.font_data.insert("ui-semibold".into(), FontData::from_owned(bytes).into());
        semibold.insert(0, "ui-semibold".into());
    }
    fonts.families.insert(FontFamily::Name("semibold".into()), semibold);
    ctx.set_fonts(fonts);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(10.5)),
            (TextStyle::Body, FontId::proportional(12.0)),
            (TextStyle::Button, FontId::proportional(12.0)),
            (TextStyle::Heading, FontId::proportional(13.0)),
            (TextStyle::Monospace, FontId::monospace(11.5)),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = vec2(6.0, 4.0);
        s.button_padding = vec2(6.0, 2.0);
        s.interact_size = vec2(36.0, 20.0);
        s.menu_margin = Margin::same(4);

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL_BG;
        v.window_fill = PANEL_BG;
        v.window_stroke = Stroke::new(1.0, POPUP_BORDER);
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(4);
        v.window_shadow = egui::Shadow { offset: [0, 1], blur: 4, spread: 0, color: Color32::from_black_alpha(128) };
        v.popup_shadow = v.window_shadow;
        v.extreme_bg_color = INSET;
        v.text_edit_bg_color = Some(INSET);
        v.faint_bg_color = rgb(0x2D2D2D);
        v.override_text_color = None;
        v.weak_text_color = Some(TEXT_DIM);
        v.selection.bg_fill = SELECTION;
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        v.hyperlink_color = ACCENT;
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = PANEL_BG;
        w.noninteractive.weak_bg_fill = PANEL_BG;
        w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        // Buttons and fields: flat, 1 px outlined, 4 px corners; brighter while hovered or pressed.
        let states = [(&mut w.inactive, BUTTON, OUTLINE), (&mut w.hovered, BUTTON_HOVER, rgb(0x707070)), (&mut w.active, BUTTON_ACTIVE, rgb(0x707070)), (&mut w.open, BUTTON_ACTIVE, rgb(0x707070))];
        for (wv, fill, outline) in states {
            wv.bg_fill = INSET;
            wv.weak_bg_fill = fill;
            wv.bg_stroke = Stroke::new(1.0, outline);
            wv.fg_stroke = Stroke::new(1.0, if fill == BUTTON { TEXT } else { Color32::WHITE });
            wv.corner_radius = CornerRadius::same(4);
            wv.expansion = 0.0;
        }
    });
}

/// Semibold UI text at `size`.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

// Menus: light, native-style popups on Windows even in the dark theme (docs/animate-ui.md §2.3).
pub const MENU_BG: Color32 = rgb(0xF2F2F2);
pub const MENU_BORDER: Color32 = rgb(0xCCCCCC);
pub const MENU_HOVER: Color32 = rgb(0x91C9F7);
pub const MENU_SEPARATOR: Color32 = rgb(0xD7D7D7);
/// Menu titles on the app bar.
pub const MENUBAR_TEXT: Color32 = rgb(0xC0C0C0);

/// The style of every dropdown, context and panel menu: 22 px rows, black
/// labels and shortcuts, a blue hover row, grey disabled items.
pub fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(24.0, 3.0);
    style.spacing.item_spacing.y = 0.0;
    style.spacing.interact_size.y = 22.0;
    // Between a label and its shortcut: two of these, 24 px as native menus leave at least.
    style.spacing.icon_spacing = 12.0;
    style.spacing.menu_margin = Margin::symmetric(2, 3);
    let v = &mut style.visuals;
    v.window_fill = MENU_BG;
    v.panel_fill = MENU_BG;
    v.window_stroke = Stroke::new(1.0, MENU_BORDER);
    v.weak_text_color = Some(Color32::BLACK);
    // Disabled items fade to #6D6D6D over the menu background.
    v.disabled_alpha = 0.55;
    v.widgets.noninteractive.weak_bg_fill = MENU_BG;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, MENU_SEPARATOR);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.fg_stroke.color = Color32::BLACK;
        w.corner_radius = CornerRadius::ZERO;
    }
    for w in [&mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.weak_bg_fill = MENU_HOVER;
        w.bg_fill = MENU_HOVER;
    }
}

/// The app bar's menu titles: grey text, a rounded hover chip.
fn menubar_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(7.0, 3.0);
    let w = &mut style.visuals.widgets;
    w.inactive.fg_stroke.color = MENUBAR_TEXT;
    w.hovered.weak_bg_fill = BUTTON_HOVER;
    w.open.weak_bg_fill = BUTTON_ACTIVE;
    w.active.weak_bg_fill = BUTTON_ACTIVE;
}

/// A menu bar whose dropdowns use `menu_style`.
pub fn menu_bar() -> egui::MenuBar {
    egui::MenuBar::new().style(menubar_style).config(egui::containers::menu::MenuConfig::new().style(menu_style))
}

/// `resp.context_menu`, in the light menu style.
pub fn context_menu(resp: &Response, add: impl FnOnce(&mut Ui)) {
    egui::Popup::context_menu(resp).style(menu_style).show(add);
}

/// A menu item with its shortcut, if any, at the right.
pub fn menu_item(ui: &mut Ui, enabled: bool, label: &str, keys: &str) -> Response {
    let button = egui::Button::new(label);
    // An empty shortcut would still end the item's accessible name with a space.
    ui.add_enabled(enabled, if keys.is_empty() { button } else { button.shortcut_text(keys) })
}

/// A checkable (or radio) menu item, with `menu_check`'s mark when `checked`.
pub fn menu_toggle(ui: &mut Ui, enabled: bool, checked: bool, label: &str) -> Response {
    let resp = menu_item(ui, enabled, label, "");
    if checked {
        menu_check(ui, &resp);
    }
    resp
}

/// The check mark of a checked menu item: a tick on a light-blue square in the
/// gutter, as Windows 10 draws them.
pub fn menu_check(ui: &Ui, item: &Response) {
    let c = egui::pos2(item.rect.left() + 12.0, item.rect.center().y);
    let p = ui.painter();
    p.rect(egui::Rect::from_center_size(c, vec2(16.0, 16.0)), 0.0, MENU_HOVER, Stroke::new(1.0, ACCENT), StrokeKind::Inside);
    let ink = if item.enabled() { Color32::BLACK } else { Color32::from_gray(0x6D) };
    p.line(vec![c + vec2(-4.0, 0.0), c + vec2(-1.5, 2.5), c + vec2(4.0, -3.0)], Stroke::new(1.5, ink));
}

/// `Popup::menu`, drawn as one of our dark popovers. It stays open while
/// clicked inside, so it can hold fields and pages.
pub fn dark_popup(button: &Response) -> egui::Popup<'_> {
    let frame = Frame::popup(&button.ctx.global_style()).fill(POPUP_BG).stroke(Stroke::new(1.0, POPUP_BORDER));
    egui::Popup::menu(button).frame(frame).style(dark_popup_style).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
}

/// A dark popover's style: egui's menu style, with the dark-menu style's
/// #3F3F3F separators (docs/animate-ui.md §4.2 item 7).
fn dark_popup_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, POPUP_SEPARATOR);
}

/// A dark popover's row, 24 high: hover fill, the label, and a blue check
/// left of it when `checked`.
pub fn popup_row(ui: &mut Ui, label: &str, checked: bool, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(rect, 0.0, POPUP_HOVER);
    }
    if checked {
        let c = egui::pos2(rect.left() + 12.0, rect.center().y);
        let points = vec![c + vec2(-4.0, 0.0), c + vec2(-1.5, 3.0), c + vec2(4.0, -3.5)];
        p.add(egui::Shape::line(points, Stroke::new(1.5, ACCENT_HOVER)));
    }
    let color = if !enabled { TEXT_DISABLED } else if resp.hovered() { Color32::WHITE } else { TEXT };
    p.text(egui::pos2(rect.left() + 26.0, rect.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(12.0), color);
    resp
}

/// Width of the gaps between docked groups, where the window's dark frame shows.
pub const GAP: i8 = 3;

/// A docked group's frame, keeping a `GAP` toward the document column (or the
/// stage, for the Timeline).
pub fn dock_frame(left: i8, right: i8, top: i8) -> Frame {
    Frame::NONE.fill(PANEL_BG).outer_margin(Margin { left, right, top, bottom: 0 })
}

/// The 11 px title strip on top of a dock column and the 1 px line under it,
/// with its collapse or expand chevrons at the end they point to: « at the
/// left, » at the right.
pub fn dock_strip(ui: &mut Ui, chevron: crate::icons::Icon) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 12.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, PANEL_BG);
    p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, BORDER));
    let x = if chevron == crate::icons::Icon::ChevronsLeft { rect.left() + 9.0 } else { rect.right() - 9.0 };
    let color = if resp.hovered() { Color32::WHITE } else { TEXT_DIM };
    crate::icons::paint(p, egui::Rect::from_center_size(egui::pos2(x, rect.top() + 5.5), vec2(9.0, 8.0)), chevron, color);
}

/// A docked panel's tab strip; returns the clicked tab, if any. The active tab
/// joins the panel below it; the panel menu (≡) at the right end opens `menu`,
/// in the light menu style.
pub fn tab_strip_menu(ui: &mut Ui, tabs: &[&str], active: usize, menu: Option<impl FnOnce(&mut Ui)>) -> Option<usize> {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), TAB_STRIP_H), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, PANEL_HEADER);
    let mut clicked = None;
    let mut x = rect.left();
    // An empty label is a panel closed out of its dock: no tab.
    for (i, label) in tabs.iter().enumerate().filter(|(_, label)| !label.is_empty()) {
        // Uncolored, so `painter.galley` paints it in the tab's color.
        let font = if i == active { semibold(12.0) } else { FontId::proportional(12.0) };
        let galley = painter.layout_no_wrap(label.to_string(), font, Color32::PLACEHOLDER);
        let tab = egui::Rect::from_min_size(egui::pos2(x, rect.top()), vec2(galley.size().x + 22.0, TAB_STRIP_H));
        let resp = ui.interact(tab, ui.id().with(("tab", label)), Sense::click());
        // A lone tab (a flyout's) is a title, not a choice.
        if tabs.len() > 1 {
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, i == active, *label));
        }
        if i == active {
            painter.rect_filled(tab, 0.0, PANEL_BG);
        }
        let color = if i == active {
            rgb(0xF3F3F3)
        } else if resp.hovered() {
            TEXT
        } else {
            rgb(0x818181)
        };
        painter.galley(tab.center() - galley.size() / 2.0, galley, color);
        if resp.clicked() {
            clicked = Some(i);
        }
        x = tab.right();
    }
    // Panel menu: four short lines at the right end, in a 24 px hit box.
    let hit = egui::Rect::from_center_size(egui::pos2(rect.right() - 14.0, rect.center().y), vec2(24.0, TAB_STRIP_H));
    let hot = match menu {
        Some(menu) => {
            let resp = ui.interact(hit, ui.id().with("panel-menu"), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{} panel menu", tabs.get(active).unwrap_or(&""))));
            let popup = egui::Popup::menu(&resp).style(menu_style).align(egui::RectAlign::BOTTOM_END).show(|ui| {
                ui.set_min_width(200.0);
                menu(ui);
            });
            resp.hovered() || popup.is_some()
        }
        None => ui.rect_contains_pointer(hit),
    };
    let glyph = egui::Rect::from_center_size(hit.center(), vec2(10.0, 7.0));
    for i in 0..4 {
        painter.hline(glyph.x_range(), glyph.top() + i as f32 * 2.0, Stroke::new(1.0, if hot { TEXT } else { TEXT_DIM }));
    }
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, BORDER));
    clicked
}

/// A flat square button holding a painted glyph, Animate-style: no fill at
/// rest, a hover fill, and a lighter chip behind the glyph while toggled on,
/// or pressed, or while the menu it opens is open.
pub fn icon_button(ui: &mut Ui, size: f32, selected: bool, enabled: bool, tooltip: &str, paint: impl FnOnce(&egui::Painter, egui::Rect, Color32)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let painter = ui.painter_at(rect);
    let radius = if size < 24.0 { 2.0 } else { 4.0 };
    if enabled && (resp.is_pointer_button_down_on() || open) {
        painter.rect_filled(rect, radius, BUTTON_ACTIVE);
    } else if selected {
        painter.rect_filled(rect, radius, TOGGLED);
    } else if enabled && resp.hovered() {
        painter.rect_filled(rect, radius, BUTTON_HOVER);
    }
    if resp.has_focus() {
        painter.rect_stroke(rect.expand(1.0), radius, Stroke::new(2.0, ACCENT), StrokeKind::Outside);
    }
    let color = if !enabled { TEXT_DISABLED } else if selected || open || resp.hovered() { Color32::WHITE } else { TEXT };
    paint(&painter, rect, color);
    resp.on_hover_text(tooltip)
}

/// A dialog's main button, OK say: white text on the blue call to action,
/// darker while hovered and pressed (docs/animate-ui.md §4.7).
pub fn cta_button(ui: &mut Ui, text: &str) -> Response {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        for (state, fill) in [(&mut w.inactive, CTA), (&mut w.hovered, CTA_HOVER), (&mut w.active, CTA_DOWN)] {
            (state.weak_bg_fill, state.bg_stroke, state.fg_stroke.color) = (fill, Stroke::NONE, Color32::WHITE);
        }
        ui.button(text)
    })
    .inner
}

/// A collapsible Properties section, open at first: a 2 px divider, a 32 px
/// header (chevron, semibold title) that a click opens or closes, then its
/// rows, 16 px in from both sides. Draw it in a Ui as wide as the panel.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    section_with(ui, title, true, add)
}

/// `section`, closed at first unless `open`.
pub fn section_with(ui: &mut Ui, title: &str, open: bool, add: impl FnOnce(&mut Ui)) {
    let id = ui.make_persistent_id(("section", title));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, open);
    let (divider, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
    ui.painter().rect_filled(divider, 0.0, BORDER);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    if resp.clicked() {
        state.toggle(ui);
    }
    let open = state.is_open();
    state.store(ui.ctx());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, title));
    let p = ui.painter();
    let color = if resp.hovered() { TEXT } else { SECTION_TITLE };
    let (x, y) = (rect.left() + 16.0, rect.center().y);
    let chevron = if open {
        vec![egui::pos2(x, y - 2.0), egui::pos2(x + 4.0, y + 2.0), egui::pos2(x + 8.0, y - 2.0)]
    } else {
        vec![egui::pos2(x + 2.0, y - 4.0), egui::pos2(x + 6.0, y), egui::pos2(x + 2.0, y + 4.0)]
    };
    p.add(egui::Shape::line(chevron, Stroke::new(1.3, color)));
    p.text(egui::pos2(rect.left() + 36.0, y), egui::Align2::LEFT_CENTER, title, semibold(12.0), color);
    if open {
        Frame::NONE.inner_margin(Margin { left: 16, right: 16, top: 0, bottom: 10 }).show(ui, add);
    }
}
