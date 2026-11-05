// Reports a pen's pressure from the canvas's pointer events to the workspace.
//
// eframe's web backend turns a pen into mouse events, which carry no pressure. Pointer events do
// carry it, so this listens to them on the canvas and passes each one to `report(pointerType,
// pressure)`, which is `flarePenReport` in the Rust module. The workspace reads the last report
// while it draws, so only the pen's value matters; any other device reports as no pen.
//
// Pointer events fire before the mouse events eframe reads, so a report is in place before the
// frame that draws the sample. A pen that leaves the canvas during a stroke keeps sending events
// to the canvas (the browser captures a pen while it touches), so leaving it clears the feed.

// The pressure of the newest sample in this event: browsers may merge several moves into one
// event, and the last of them is the one the pen is at now.
function newestPressure(event) {
  const samples =
    event.type === "pointermove" && typeof event.getCoalescedEvents === "function"
      ? event.getCoalescedEvents()
      : [];
  const newest = samples.length > 0 ? samples[samples.length - 1] : event;
  // A pen that does not say how hard it presses is reported as not a number, which the workspace
  // reads as full pressure rather than a thin line.
  return typeof newest.pressure === "number" ? newest.pressure : Number.NaN;
}

// Starts reporting. Returns a function that stops it, for a page that tears the canvas down.
export function installPenPressure(canvas, report) {
  const forward = (event) => report(event.pointerType ?? "", newestPressure(event));
  const clear = () => report("", 0);
  const events = ["pointerdown", "pointermove", "pointerup"];
  for (const type of events) canvas.addEventListener(type, forward);
  canvas.addEventListener("pointerleave", clear);
  canvas.addEventListener("pointercancel", clear);
  return () => {
    for (const type of events) canvas.removeEventListener(type, forward);
    canvas.removeEventListener("pointerleave", clear);
    canvas.removeEventListener("pointercancel", clear);
  };
}
