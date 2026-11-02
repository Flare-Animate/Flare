// Theme completeness check for stuff/config/qss.
//
// Bug this exists to prevent: LiquidGlass.qss shipped as a 33-line fragment
// while every other theme was a standalone ~3356-line stylesheet covering all
// 715 selectors. Selecting it silently left most of the UI unstyled -- there
// was no error, just a broken-looking application.
//
// A theme is complete if it either covers the full selector set itself, or
// @imports a theme that does. Checked with node, no dependencies, so it can run
// in the same suite as the rest of the tooling.
//
//   node tools/themes/check-themes.mjs
//
// Exits non-zero if any theme is incomplete.

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const QSS = resolve(HERE, "..", "..", "stuff", "config", "qss");

/** Selectors are the lines that open a rule block: `Foo {` or `Foo:hover {`. */
const selectorCount = (text) => (text.match(/\{/g) || []).length;

/** The set of selectors a stylesheet actually declares. */
const selectorsOf = (text) => {
  const out = new Set();
  for (const line of text.split(/\r?\n/)) {
    const m = line.match(/^([^{}@][^{}]*?)\s*\{/);
    if (m) out.add(m[1].trim());
  }
  return out;
};

/** @import specifiers, relative ones only. */
const importsOf = (text) =>
  [...text.matchAll(/@import\s+(['"])(?!https?:|\/|data:)([^'"]+)\1/g)].map((m) => m[2]);

/** "../Dark/Dark.qss" -> "Dark": themes are directories named after themselves. */
const themeDirOf = (spec) => {
  const parts = spec.replace(/\\/g, "/").split("/").filter(Boolean);
  const last = parts[parts.length - 1];
  return last.replace(/\.qss$/i, "");
};

const themeDirs = existsSync(QSS)
  ? readdirSync(QSS, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .map((d) => d.name)
      .sort()
  : [];

if (themeDirs.length === 0) {
  console.error(`no themes found under ${QSS}`);
  process.exit(1);
}

const cache = new Map();
function load(dir) {
  if (cache.has(dir)) return cache.get(dir);
  const file = join(QSS, dir, `${dir}.qss`);
  if (!existsSync(file)) {
    cache.set(dir, null);
    return null;
  }
  const text = readFileSync(file, "utf8");
  const info = { dir, text, file };
  cache.set(dir, info);
  return info;
}

/** Selectors a theme declares after following its @import chain. */
function effectiveSelectors(dir, seen = new Set()) {
  const info = load(dir);
  if (!info) return new Set();
  const out = new Set(selectorsOf(info.text));
  for (const spec of importsOf(info.text)) {
    const target = themeDirOf(spec);
    if (!target || seen.has(target)) continue; // guard a cycle
    seen.add(target);
    for (const sel of effectiveSelectors(target, seen)) out.add(sel);
  }
  return out;
}

// Reference = the union across standalone themes, so an outlier theme cannot
// define "complete". Synthwave carries 2 rules the others lack; that is extra,
// not a requirement.
const required = new Set();
for (const dir of themeDirs) {
  const info = load(dir);
  if (!info) continue;
  for (const sel of selectorsOf(info.text)) required.add(sel);
}

console.log(`themes under ${QSS}`);
console.log(`reference coverage: ${required.size} distinct selectors\n`);

let bad = 0;
for (const dir of themeDirs) {
  const info = load(dir);
  if (!info) {
    console.log(`FAIL ${dir.padEnd(16)} no ${dir}.qss`);
    bad++;
    continue;
  }
  const own = selectorCount(info.text);
  const via = importsOf(info.text);
  const have = effectiveSelectors(dir);
  const missing = [...required].filter((s) => !have.has(s));
  const ok = missing.length === 0;
  const note = via.length
    ? `${own} own + ${via.length} import(s) -> ${have.size}`
    : `${own}`;
  console.log(`${ok ? "PASS" : "FAIL"} ${dir.padEnd(16)} ${note}`);
  if (!ok) {
    console.log(
      `     -> ${missing.length} of ${required.size} selectors uncovered` +
        (missing.length <= 6 ? `: ${missing.map((m) => m.slice(0, 40)).join(", ")}` : ""),
    );
    bad++;
  }
}

console.log(`\n${themeDirs.length - bad}/${themeDirs.length} themes complete`);
process.exit(bad ? 1 : 0);