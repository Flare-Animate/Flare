// add_instance_to_symbol.jsfl
// JSFL Script: Add Instance to Symbol
//
// Adds the currently selected group/content on the stage as a frame in a
// library symbol, preserving the pivot point relationship.
// Requested in: https://github.com/Flare-Animate/Flare/issues/11
//
// Behaviour (matching the original description):
//   - If the text input is left empty or at the default (""),  the selected
//     group is added as a new frame at the END of the currently selected
//     library item.
//   - If a symbol name is given and it exists in the library, the group is
//     added there.
//   - If the name does not exist, a new graphic symbol is created.
//   In all cases the group's pivot point is aligned to the target symbol's
//   registration point (origin).

var doc = fl.getDocumentDOM();
if (!doc) {
    fl.trace("Add Instance to Symbol: No document open.");
} else {
    addInstanceToSymbol();
}

/**
 * Main entry point.
 */
function addInstanceToSymbol() {
    var doc = fl.getDocumentDOM();
    var sel = doc.selection;
    if (!sel || sel.length === 0) {
        fl.trace("Add Instance to Symbol: Nothing selected.");
        return;
    }

    var symbolName = fl.ask("Symbol name (leave empty to use currently selected library item):", "");

    var targetItem = null;

    if (!symbolName || symbolName === "") {
        // Use the currently selected library item
        targetItem = doc.library.getSelectedItems()[0] || null;
        if (!targetItem) {
            fl.trace("Add Instance to Symbol: No library item selected and no name given.");
            return;
        }
        symbolName = targetItem.name;
    } else {
        // Check if it exists
        targetItem = _findLibraryItem(doc, symbolName);
        if (!targetItem) {
            // Create a new graphic symbol
            doc.library.addNewItem("graphic", symbolName);
            targetItem = _findLibraryItem(doc, symbolName);
            fl.trace("Add Instance to Symbol: Created new graphic symbol '" + symbolName + "'.");
        }
    }

    if (!targetItem) {
        fl.trace("Add Instance to Symbol: Failed to locate or create symbol '" + symbolName + "'.");
        return;
    }

    // Record the group's current position (pivot offset)
    var pivotX = sel[0].x;
    var pivotY = sel[0].y;

    // Copy selection into the symbol's timeline
    doc.clipCopy();
    doc.library.editItem(symbolName);
    var symTimeline = doc.getTimeline();
    var lastFrame = symTimeline.layers[0].frameCount;
    symTimeline.insertBlankKeyframe(lastFrame);
    symTimeline.currentFrame = lastFrame;
    doc.clipPaste(true);

    // Reposition pasted content so that the group's pivot maps to the symbol's origin
    var pastedSel = doc.selection;
    if (pastedSel && pastedSel.length > 0) {
        pastedSel[0].x = -pivotX;
        pastedSel[0].y = -pivotY;
    }

    doc.exitEditMode();
    fl.trace("Add Instance to Symbol: Added group to '" + symbolName + "' at frame " + (lastFrame + 1) + ".");
}

/**
 * Find a library item by name; returns null if not found.
 */
function _findLibraryItem(doc, name) {
    var items = doc.library.items;
    for (var i = 0; i < items.length; i++) {
        if (items[i].name === name) return items[i];
    }
    return null;
}
