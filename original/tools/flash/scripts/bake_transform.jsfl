// bake_transform.jsfl
// JSFL Script: Bake Transform
//
// Applies the transform of the currently selected symbol instance to the
// symbol's artwork, then applies the reverse transform to all instances of
// the symbol across all timelines so that nothing visually changes.
//
// Requested in: https://github.com/Flare-Animate/Flare/issues/11
//
// Behaviour (matching the original description):
//   The target frame range depends on the looping behaviour of the selected
//   instance:
//     - "single frame"  → apply only to that one frame in the symbol.
//     - "play once"     → apply to all frames >= the instance's firstFrame.
//     - "loop"          → apply to all frames in the symbol.
//   A dialog is shown to allow the user to override this selection.

var doc = fl.getDocumentDOM();
if (!doc) {
    fl.trace("Bake Transform: No document open.");
} else {
    bakeTransform();
}

/**
 * Main entry point.
 */
function bakeTransform() {
    var doc = fl.getDocumentDOM();
    var sel = doc.selection;
    if (!sel || sel.length === 0) {
        fl.trace("Bake Transform: Nothing selected.");
        return;
    }

    var inst = sel[0];
    if (inst.elementType !== "instance" || !inst.libraryItem) {
        fl.trace("Bake Transform: Selected element is not a symbol instance.");
        return;
    }

    // Determine the frame scope from the instance's looping mode (or ask the user)
    var loopMode = inst.loop;  // "loop", "play once", "single frame"
    var choices = ["Loop (all frames)", "Play Once (frame >= firstFrame)", "Single Frame"];
    var defaultChoice;
    if (loopMode === "loop")              defaultChoice = 0;
    else if (loopMode === "play once")    defaultChoice = 1;
    else                                  defaultChoice = 2;

    var choiceStr = fl.ask(
        "Frame scope for bake:\n  0 = Loop (all frames)\n  1 = Play Once\n  2 = Single Frame",
        String(defaultChoice)
    );
    if (choiceStr === null) { fl.trace("Cancelled."); return; }
    var choice = parseInt(choiceStr, 10);
    if (isNaN(choice) || choice < 0 || choice > 2) choice = defaultChoice;

    var symbolName = inst.libraryItem.name;

    // Capture the current transform matrix
    var mat = inst.matrix;
    var scaleX = mat.a;
    var scaleY = mat.d;
    var skewX  = mat.b;
    var skewY  = mat.c;
    var tx     = mat.tx;
    var ty     = mat.ty;

    // Enter the symbol and apply the transform to artwork frames
    doc.library.editItem(symbolName);
    var symTimeline = doc.getTimeline();

    var firstFr = (choice === 1) ? inst.firstFrame : 0;
    var lastFr  = (choice === 2) ? inst.firstFrame + 1 : symTimeline.layers[0].frameCount;

    for (var l = 0; l < symTimeline.layers.length; l++) {
        var layer = symTimeline.layers[l];
        for (var f = firstFr; f < lastFr && f < layer.frames.length; f++) {
            var frame = layer.frames[f];
            for (var e = 0; e < frame.elements.length; e++) {
                var el = frame.elements[e];
                el.matrix = {
                    a:  scaleX, b: skewX,
                    c:  skewY,  d: scaleY,
                    tx: el.matrix.tx + tx,
                    ty: el.matrix.ty + ty
                };
            }
        }
    }

    doc.exitEditMode();

    // Apply the INVERSE transform to ALL instances of this symbol
    var invA  =  scaleY / (scaleX * scaleY - skewX * skewY);
    var invD  =  scaleX / (scaleX * scaleY - skewX * skewY);
    var invB  = -skewX  / (scaleX * scaleY - skewX * skewY);
    var invC  = -skewY  / (scaleX * scaleY - skewX * skewY);
    var invTx = -(invA * tx + invB * ty);
    var invTy = -(invC * tx + invD * ty);

    var totalUpdated = 0;
    for (var t = 0; t < doc.timelines.length; t++) {
        var timeline = doc.timelines[t];
        for (var ll = 0; ll < timeline.layers.length; ll++) {
            var tlLayer = timeline.layers[ll];
            for (var ff = 0; ff < tlLayer.frames.length; ff++) {
                var tlFrame = tlLayer.frames[ff];
                for (var ee = 0; ee < tlFrame.elements.length; ee++) {
                    var tlEl = tlFrame.elements[ee];
                    if (tlEl.elementType === "instance" &&
                        tlEl.libraryItem && tlEl.libraryItem.name === symbolName) {
                        var m = tlEl.matrix;
                        tlEl.matrix = {
                            a:  m.a * invA  + m.b * invC,
                            b:  m.a * invB  + m.b * invD,
                            c:  m.c * invA  + m.d * invC,
                            d:  m.c * invB  + m.d * invD,
                            tx: m.a * invTx + m.b * invTy + m.tx,
                            ty: m.c * invTx + m.d * invTy + m.ty
                        };
                        totalUpdated++;
                    }
                }
            }
        }
    }

    fl.trace("Bake Transform: applied transform to '" + symbolName +
             "' artwork, updated " + totalUpdated + " instance(s).");
}
