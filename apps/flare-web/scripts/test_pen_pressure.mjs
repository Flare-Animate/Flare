// Checks the page's pointer-event shim with a stand-in canvas: a pen's pressure reaches the
// workspace, any other device clears it, the newest coalesced sample wins, a pen without a
// pressure is not thin, and stopping the shim stops the reports.
//
// Run with: node apps/flare-web/scripts/test_pen_pressure.mjs
import assert from "node:assert/strict";
import { installPenPressure } from "../src/lib/pen-pressure.js";

// A pointer event as a test can build it: the fields a browser gives, plus the coalesced samples.
class FakePointerEvent extends Event {
  constructor(type, fields = {}, coalesced) {
    super(type);
    Object.assign(this, fields);
    if (coalesced) this.coalesced = coalesced;
  }
  getCoalescedEvents() {
    return this.coalesced ?? [];
  }
}

function setup() {
  const canvas = new EventTarget();
  const reports = [];
  const stop = installPenPressure(canvas, (type, pressure) => reports.push([type, pressure]));
  return { canvas, reports, stop };
}

// A pen move reports its own pressure.
{
  const { canvas, reports } = setup();
  canvas.dispatchEvent(new FakePointerEvent("pointermove", { pointerType: "pen", pressure: 0.42 }));
  assert.deepEqual(reports, [["pen", 0.42]]);
}

// Only the newest coalesced sample is reported, since the stroke is where the pen is now.
{
  const { canvas, reports } = setup();
  const older = { pointerType: "pen", pressure: 0.2 };
  const newer = { pointerType: "pen", pressure: 0.8 };
  canvas.dispatchEvent(
    new FakePointerEvent("pointermove", { pointerType: "pen", pressure: 0.2 }, [older, newer]),
  );
  assert.deepEqual(reports, [["pen", 0.8]]);
}

// A mouse and a touch report their type, so the workspace clears the pen.
{
  const { canvas, reports } = setup();
  canvas.dispatchEvent(new FakePointerEvent("pointermove", { pointerType: "mouse", pressure: 0.5 }));
  canvas.dispatchEvent(new FakePointerEvent("pointerdown", { pointerType: "touch", pressure: 0.5 }));
  assert.deepEqual(reports, [
    ["mouse", 0.5],
    ["touch", 0.5],
  ]);
}

// A pen with no pressure figure is reported as not a number, never as zero.
{
  const { canvas, reports } = setup();
  canvas.dispatchEvent(new FakePointerEvent("pointerdown", { pointerType: "pen" }));
  assert.equal(reports.length, 1);
  assert.equal(reports[0][0], "pen");
  assert.ok(Number.isNaN(reports[0][1]));
}

// Leaving the canvas or a cancelled pointer clears the pen.
{
  const { canvas, reports } = setup();
  canvas.dispatchEvent(new FakePointerEvent("pointerleave", { pointerType: "pen", pressure: 0 }));
  canvas.dispatchEvent(new FakePointerEvent("pointercancel", { pointerType: "pen", pressure: 0 }));
  assert.deepEqual(reports, [
    ["", 0],
    ["", 0],
  ]);
}

// After stopping, the canvas reports nothing.
{
  const { canvas, reports, stop } = setup();
  stop();
  canvas.dispatchEvent(new FakePointerEvent("pointermove", { pointerType: "pen", pressure: 0.5 }));
  canvas.dispatchEvent(new FakePointerEvent("pointerleave", {}));
  assert.deepEqual(reports, []);
}

console.log("pen pressure shim: all checks passed");
