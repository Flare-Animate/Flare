//! The frame's leftover textures.

use super::*;

#[test]
fn textures_a_skipped_frame_left_to_free_are_freed_and_the_output_can_be_dropped() {
    let mut textures = egui::TexturesDelta::default();
    textures.free.insert(egui::TextureId::Managed(3));
    textures.free.insert(egui::TextureId::Managed(7));
    let mut freed = Vec::new();
    free_left_over(&mut textures, |id| freed.push(id));
    freed.sort_by_key(|id| format!("{id:?}"));
    assert_eq!(
        freed,
        vec![egui::TextureId::Managed(3), egui::TextureId::Managed(7)]
    );
    // egui panics (in debug builds) if an output is dropped with deltas left; this must not.
    assert!(textures.is_empty());
    drop(textures);
    // Nothing left means nothing freed.
    let mut again = egui::TexturesDelta::default();
    let mut none = 0;
    free_left_over(&mut again, |_| none += 1);
    assert_eq!(none, 0);
}
