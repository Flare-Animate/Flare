//! What a layer's folders say about it: whether one hides it, and how see-through they make it.

/// Whether a folder above `layers[index]` that holds it (directly or through other folders) is
/// hidden. Parents are always earlier layers, so one pass up the list finds them all.
pub(crate) fn hidden_by_folder(layers: &[flare_document::Layer], index: usize) -> bool {
    let mut parent = layers[index].parent();
    while let Some(id) = parent {
        let Some(folder) = layers[..index].iter().find(|l| l.id() == id) else {
            return false;
        };
        if !folder.is_visible() {
            return true;
        }
        parent = folder.parent();
    }
    false
}

/// How opaque the stage draws `layers[index]` while it is edited: its own view opacity times that
/// of every folder it is in, from 0 to 1. Bounded by the number of layers, so a damaged list of
/// parents cannot loop.
pub(crate) fn view_alpha_of(layers: &[flare_document::Layer], index: usize) -> f32 {
    let percent = |l: &flare_document::Layer| f32::from(l.view_opacity()) / 100.0;
    let mut alpha = percent(&layers[index]);
    let mut parent = layers[index].parent();
    for _ in 0..index {
        let Some(folder) = parent.and_then(|id| layers[..index].iter().find(|l| l.id() == id))
        else {
            break;
        };
        alpha *= percent(folder);
        parent = folder.parent();
    }
    alpha
}
