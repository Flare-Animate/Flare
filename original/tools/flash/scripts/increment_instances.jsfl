// increment_instances.jsfl
// JSFL Script: Increment Instances
//
// Shifts frame instances of the selected symbol on all timelines.
// Requested in: https://github.com/Flare-Animate/Flare/issues/11
//
// Usage:
//   1. Select a symbol instance on the stage.
//   2. Run this script via Commands > Run Command.
//   3. Enter the start frame and the shift amount when prompted.
//
// Behaviour:
//   For every timeline in the document, every instance of the same symbol
//   whose current frame number is >= startFrame is shifted by `shift` frames.

var doc = fl.getDocumentDOM();
if (!doc) {
    fl.trace("Increment Instances: No document open.");
} else {
    incrementInstances();
}

/**
 * Main entry point.
 */
function incrementInstances() {
    var startFrame = parseInt(fl.ask("Start frame (frames >= this will be shifted):", "1"), 10);
    if (isNaN(startFrame)) { fl.trace("Cancelled."); return; }

    var shift = parseInt(fl.ask("Shift amount (frames to add; negative to subtract):", "1"), 10);
    if (isNaN(shift)) { fl.trace("Cancelled."); return; }

    var doc = fl.getDocumentDOM();
    var totalChanged = 0;

    for (var t = 0; t < doc.timelines.length; t++) {
        var timeline = doc.timelines[t];
        for (var l = 0; l < timeline.layers.length; l++) {
            var layer = timeline.layers[l];
            for (var f = 0; f < layer.frames.length; f++) {
                var frame = layer.frames[f];
                for (var e = 0; e < frame.elements.length; e++) {
                    var el = frame.elements[e];
                    if (el.elementType === "instance" && el.libraryItem) {
                        if (f >= startFrame - 1) {
                            el.firstFrame = Math.max(0, el.firstFrame + shift);
                            totalChanged++;
                        }
                    }
                }
            }
        }
    }

    fl.trace("Increment Instances: shifted " + totalChanged + " instance(s) by " + shift + " frame(s).");
}
