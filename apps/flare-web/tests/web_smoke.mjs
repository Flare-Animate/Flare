// Loads a built site in headless Chromium and fails if it does not come up: no page error, no
// Rust panic, no failed request, no error panel, and a stage that drew more than one colour.
// Usage: node tests/web_smoke.mjs <url> [screenshot.png]
// Needs `playwright` (npx playwright install chromium) to be resolvable from the working directory.
import { createRequire } from "node:module";
const require = createRequire(process.cwd() + "/");
const { chromium } = require("playwright");

const url = process.argv[2];
if (!url) {
  console.error("usage: node web_smoke.mjs <url> [screenshot.png]");
  process.exit(2);
}
const shot = process.argv[3];
const problems = [];
const browser = await chromium.launch({
  args: [
    "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-webgl",
    "--ignore-gpu-blocklist", "--use-gl=angle",
  ],
});
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.on("pageerror", (e) => problems.push(`page error: ${e}`));
  page.on("console", (m) => {
    if (m.type() === "error") problems.push(`console error: ${m.text().slice(0, 300)}`);
  });
  page.on("requestfailed", (r) => problems.push(`request failed: ${r.url()}`));
  page.on("response", (r) => {
    if (r.status() >= 400) problems.push(`HTTP ${r.status()}: ${r.url()}`);
  });
  await page.goto(url, { waitUntil: "load" });
  await page.waitForSelector("html[data-flare=started], #flare-error", { timeout: 60000 }).catch(() =>
    problems.push("startup marker (html[data-flare=started]) never appeared"));
  await page.waitForTimeout(15000); // eframe starts asynchronously after init() resolves; software GL is slow
  const panel = await page.$("#flare-error");
  if (panel) problems.push(`error panel: ${await panel.innerText()}`);
  const png = await page.screenshot({ path: shot });
  const colours = await page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = new OffscreenCanvas(img.width, img.height);
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, img.width, img.height).data;
    const seen = new Set();
    for (let i = 0; i < d.length && seen.size < 64; i += 4) seen.add((d[i] << 16) | (d[i + 1] << 8) | d[i + 2]);
    return seen.size;
  }, png.toString("base64"));
  if (colours < 16) problems.push(`the page drew only ${colours} distinct colours; the canvas looks blank`);
} finally {
  await browser.close();
}
if (problems.length) {
  console.error(`web smoke FAILED for ${url}:\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log(`web smoke ok: ${url}`);
