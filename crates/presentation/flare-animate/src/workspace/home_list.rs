//! The recent projects as a list with a heading per column, as Animate's Home lists them: a click
//! on a heading orders the list by it.

use egui::{Align2, FontId, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use flare_ui::theme;
use std::path::PathBuf;
use std::time::SystemTime;

use super::*;
use flare_recent::SortBy;

const ROW_HEIGHT: f32 = 30.0;
const MODIFIED_WIDTH: f32 = 150.0;
const SIZE_WIDTH: f32 = 90.0;
const KIND_WIDTH: f32 = 150.0;
const CELL_PAD: f32 = 10.0;

impl Workspace {
    /// Draws `paths` in the order the page is sorted by. Returns the one chosen to open.
    pub(super) fn recent_list(&mut self, ui: &mut Ui, paths: &[PathBuf]) -> Option<PathBuf> {
        let width = ui.available_width();
        let name_width = (width - MODIFIED_WIDTH - SIZE_WIDTH - KIND_WIDTH).max(120.0);
        let columns = [
            (SortBy::Name, "start-col-name", name_width),
            (SortBy::Modified, "start-col-modified", MODIFIED_WIDTH),
            (SortBy::Size, "start-col-size", SIZE_WIDTH),
            (SortBy::Kind, "start-col-kind", KIND_WIDTH),
        ];
        let (head, _) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
        let mut x = head.left();
        for (column, key, column_width) in columns {
            let label = self.tr(key);
            let label = label.as_str();
            let cell = Rect::from_min_size(pos2(x, head.top()), vec2(column_width, ROW_HEIGHT));
            x += column_width;
            let response = ui
                .interact(
                    cell,
                    ui.make_persistent_id(("recent-heading", key)),
                    Sense::click(),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.hovered() {
                ui.painter().rect_filled(cell, 0.0, theme::hover());
            }
            let active = self.home.sort.by == column;
            let ink = if active {
                theme::text()
            } else {
                theme::text_dim()
            };
            ui.painter().text(
                pos2(cell.left() + CELL_PAD, cell.center().y),
                Align2::LEFT_CENTER,
                label,
                FontId::proportional(12.0),
                ink,
            );
            if active {
                let arrow = pos2(
                    cell.left() + CELL_PAD + label_width(ui, label) + 10.0,
                    cell.center().y,
                );
                paint_arrow(ui, arrow, self.home.sort.descending, ink);
            }
            if response.clicked() {
                self.home.sort = self.home.sort.click(column);
            }
        }
        ui.painter().hline(
            head.x_range(),
            head.bottom(),
            Stroke::new(1.0, theme::border()),
        );

        let now = SystemTime::now();
        let mut open = None;
        for path in paths {
            let (row, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
            if response.hovered() {
                ui.painter().rect_filled(row, 3.0, theme::hover());
            }
            let info = self.home.info.get(path);
            let (name, folder) = super::home::name_and_folder(path);
            // Until the thread has answered, the cells say so instead of guessing.
            let (modified, size) = match info {
                None => (self.tr("start-reading"), String::new()),
                Some(info) => (
                    info.modified
                        .map_or_else(|| self.tr("start-not-found"), |t| self.ago(t, now)),
                    info.size.map(flare_recent::format_size).unwrap_or_default(),
                ),
            };
            let kind = self.file_kind_label(path);
            let cells = [
                (name_width, name.as_str(), true),
                (MODIFIED_WIDTH, modified.as_str(), false),
                (SIZE_WIDTH, size.as_str(), false),
                (KIND_WIDTH, kind.as_str(), false),
            ];
            let mut x = row.left();
            for (cell_width, text, primary) in cells {
                let cell = Rect::from_min_size(pos2(x, row.top()), vec2(cell_width, ROW_HEIGHT));
                x += cell_width;
                let painter = ui
                    .painter()
                    .with_clip_rect(cell.shrink2(vec2(CELL_PAD, 0.0)));
                let at = pos2(cell.left() + CELL_PAD, cell.center().y);
                let font = FontId::proportional(13.0);
                if primary {
                    let drawn = painter.text(at, Align2::LEFT_CENTER, text, font, theme::text());
                    painter.text(
                        pos2(drawn.right() + 10.0, at.y),
                        Align2::LEFT_CENTER,
                        &folder,
                        FontId::proportional(11.0),
                        theme::text_dim(),
                    );
                } else {
                    painter.text(at, Align2::LEFT_CENTER, text, font, theme::text_dim());
                }
            }
            if response
                .on_hover_text(path.display().to_string())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                open = Some(path.clone());
            }
        }
        open
    }
}

impl Workspace {
    /// How long ago `then` was, in the interface's language: flare-recent only has English.
    fn ago(&self, then: SystemTime, now: SystemTime) -> String {
        const DAY: u64 = 86_400;
        let seconds = now.duration_since(then).unwrap_or_default().as_secs();
        let (unit, n) = match seconds {
            0..=59 => return self.tr("start-just-now"),
            60..=3599 => ("minute", seconds / 60),
            3600..=86_399 => ("hour", seconds / 3600),
            86_400..=2_591_999 => ("day", seconds / DAY),
            2_592_000..=31_535_999 => ("month", seconds / (30 * DAY)),
            _ => ("year", seconds / (365 * DAY)),
        };
        self.tr_count(&format!("start-ago-{unit}"), n)
    }

    /// What kind of file a recent project is, going by its name.
    fn file_kind_label(&self, path: &std::path::Path) -> String {
        let key = match flare_recent::kind(path) {
            "Flare project" => "start-kind-flare",
            "Animate document" => "start-kind-animate-document",
            "Animate project" => "start-kind-animate-project",
            _ => "start-kind-file",
        };
        self.tr(key)
    }

    /// The name of a size preset on Home.
    pub(super) fn preset_title(&self, name: &str) -> String {
        let key = match name {
            "Character Animation" => "start-preset-character",
            "Social" => "start-preset-social",
            "Portrait" => "start-preset-portrait",
            "Web" => "start-preset-web",
            _ => "start-preset-classic",
        };
        self.tr(key)
    }
}

fn label_width(ui: &Ui, label: &str) -> f32 {
    ui.painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(12.0), theme::text())
        .size()
        .x
}

/// A small triangle: pointing up for a list that runs from the smallest, down for the other way.
fn paint_arrow(ui: &Ui, center: egui::Pos2, descending: bool, ink: egui::Color32) {
    let half = 4.0;
    let tip = if descending { half } else { -half };
    ui.painter().add(Shape::convex_polygon(
        vec![
            pos2(center.x - half, center.y - tip / 2.0),
            pos2(center.x + half, center.y - tip / 2.0),
            pos2(center.x, center.y + tip / 2.0),
        ],
        ink,
        Stroke::NONE,
    ));
}
