// Loads a built site at a phone size and a desktop size in headless Chromium and fails if either
// does not come up cleanly: no page error, no failed request, the page does not scroll sideways,
// the canvas fills the viewport, and the page drew more than one colour.
// Usage: node tests/mobile_layout.mjs <url>
// Needs `playwright` (npx playwright install chromium) to be resolvable from the working directory.
import { createRequire } from "node:module";
const require = createRequire(process.cwd() + "/");
const { chromium } = require("playwright");

const url = process.argv[2];
if (!url) {
  console.error("usage: node mobile_layout.mjs <url>");
  process.exit(2);
}
const sizes = [
  { name: "phone", width: 390, height: 844, query: "?ui=mobile" },
  { name: "desktop", width: 1280, height: 800, query: "?ui=desktop" },
];
const problems = [];
const browser = await chromium.launch();
try {
  for (const size of sizes) {
    const context = await browser.newContext({
      viewport: { width: size.width, height: size.height },
      deviceScaleFactor: 1,
    });
    const page = await context.newPage();
    const here = `${size.name} ${size.width}x${size.height}`;
    page.on("pageerror", (e) => problems.push(`${here}: page error: ${e}`));
    page.on("console", (m) => {
      if (m.type() === "error") problems.push(`${here}: console error: ${m.text().slice(0, 300)}`);
    });
    page.on("requestfailed", (r) => problems.push(`${here}: request failed: ${r.url()}`));
    await page.goto(url + size.query, { waitUntil: "load" });
    await page.waitForSelector("html[data-flare=started], #flare-error", { timeout: 60000 }).catch(() =>
      problems.push(`${here}: startup marker never appeared`));
    await page.waitForTimeout(15000); // eframe starts after init(); software GL is slow
    if (await page.$("#flare-error")) {
      problems.push(`${here}: error panel: ${await (await page.$("#flare-error")).innerText()}`);
    }
    const geometry = await page.evaluate(() => {
      const canvas = document.getElementById("flare-canvas");
      const box = canvas ? canvas.getBoundingClientRect() : null;
      return {
        scrollWidth: document.documentElement.scrollWidth,
        innerWidth: window.innerWidth,
        canvasWidth: box ? box.width : 0,
        canvasHeight: box ? box.height : 0,
      };
    });
    if (geometry.scrollWidth > geometry.innerWidth) {
      problems.push(`${here}: the page scrolls sideways (${geometry.scrollWidth} > ${geometry.innerWidth})`);
    }
    if (Math.abs(geometry.canvasWidth - size.width) > 1 || Math.abs(geometry.canvasHeight - size.height) > 1) {
      problems.push(`${here}: the canvas is ${geometry.canvasWidth}x${geometry.canvasHeight}, not the viewport`);
    }
    const png = await page.screenshot({ path: undefined });
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
    if (colours < 16) problems.push(`${here}: the page drew only ${colours} distinct colours`);
    console.log(`${here}: canvas ${geometry.canvasWidth}x${geometry.canvasHeight}, scroll width ${geometry.scrollWidth}, ${colours}+ colours`);
    await context.close();
  }
} finally {
  await browser.close();
}
if (problems.length) {
  console.error(`mobile layout check FAILED for ${url}:\n- ${problems.join("\n- ")}`);
  process.exit(1);
}
console.log(`mobile layout check ok: ${url}`);
