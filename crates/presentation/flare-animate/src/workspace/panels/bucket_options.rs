//! The Paint Bucket's option: how wide a gap in the lines it still fills across, Animate's Gap
//! Size. The sizes are `flare_tools::paint::GapSize`, measured on the screen.

use egui::Ui;
use flare_tools::paint::GapSize;
use flare_ui::chrome;

use crate::workspace::Workspace;

impl Workspace {
    pub(super) fn bucket_options(&mut self, ui: &mut Ui) {
        let current = self.gap_label(self.paint.gap);
        chrome::row(ui, &tr!(self, "opt-gap-size"), |ui| {
            egui::ComboBox::from_id_salt("gap_size_menu")
                .width(140.0)
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for size in GapSize::ALL {
                        let label = self.gap_label(size);
                        ui.selectable_value(&mut self.paint.gap, size, label);
                    }
                });
        });
    }

    pub(crate) fn gap_label(&self, size: GapSize) -> String {
        self.tr(match size {
            GapSize::DontClose => "opt-gap-none",
            GapSize::Small => "opt-gap-small",
            GapSize::Medium => "opt-gap-medium",
            GapSize::Large => "opt-gap-large",
        })
    }
}
