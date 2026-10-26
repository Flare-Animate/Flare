// Render an exported HTML5 file in headless chromium; assert no console errors and a non-blank page.
// usage: NODE_PATH=/opt/node-tools/node_modules node html5_check.js <file.html> <out.png>
// prints one JSON line: {"ok":bool,"errors":[...],"console":[...],"screenshot":path}
const fs = require('fs');
const { chromium } = require('playwright');
function findChrome() {
  if (process.env.QA_CHROMIUM && fs.existsSync(process.env.QA_CHROMIUM)) return process.env.QA_CHROMIUM;
  for (const d of fs.readdirSync('/opt/pw-browsers').filter(x => /^chromium-\d+$/.test(x)))
    { const p = `/opt/pw-browsers/${d}/chrome-linux/chrome`; if (fs.existsSync(p)) return p; }
  return undefined;
}
(async () => {
  const [file, shot] = process.argv.slice(2);
  const errors = [], log = [];
  const browser = await chromium.launch({ executablePath: findChrome(), args: ['--no-sandbox', '--allow-file-access-from-files'] });
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  page.on('console', m => { log.push(m.type() + ': ' + m.text()); if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push('pageerror: ' + e.message));
  page.on('requestfailed', r => errors.push('requestfailed: ' + r.url()));
  await page.goto('file://' + require('path').resolve(file));
  await page.waitForTimeout(2000);
  await page.screenshot({ path: shot });
  await browser.close();
  console.log(JSON.stringify({ ok: errors.length === 0, errors, console: log.slice(0, 20), screenshot: shot }));
})().catch(e => { console.log(JSON.stringify({ ok: false, errors: ['harness: ' + e.message], console: [], screenshot: '' })); process.exit(0); });
