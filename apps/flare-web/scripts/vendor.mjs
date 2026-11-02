// Copies the WebAssembly libraries the page needs (SQLite and zstd for .flare files, and Ruffle for
// Test Movie, from node_modules) into the folder the page is served from, ready to load with no
// bundler.
//
//   node scripts/vendor.mjs OUT_DIR        (after `npm ci`)
//
// SQLite's module loads as it is. zstd's browser build was written for a bundler: its modules import
// each other without ".js", which a browser will not load, so the specifiers are completed here.
// Every relative import is then checked to name a file that exists, and the script fails if one
// does not, so a changed package breaks the build and not the page.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const modules = join(here, "..", "node_modules");
const out = process.argv[2];
if (!out) {
  console.error("usage: vendor.mjs OUT_DIR");
  process.exit(2);
}

const read = (path) => JSON.parse(readFileSync(path, "utf8"));
const sqlite = join(modules, "@sqlite.org", "sqlite-wasm");
const zstd = join(modules, "@bokuweb", "zstd-wasm");
for (const dir of [sqlite, zstd]) {
  if (!existsSync(dir)) {
    console.error(`${dir} is missing: run \`npm ci\` in apps/flare-web first`);
    process.exit(1);
  }
}

// SQLite: the ES module and the wasm it loads from beside itself.
mkdirSync(join(out, "sqlite"), { recursive: true });
copyFileSync(join(sqlite, "dist", "index.mjs"), join(out, "sqlite", "index.mjs"));
copyFileSync(join(sqlite, "dist", "sqlite3.wasm"), join(out, "sqlite", "sqlite3.wasm"));

// zstd: the ES module build, specifiers completed.
const zstdOut = join(out, "zstd");
const SPECIFIER = /((?:from|import)\s*\(?\s*['"])(\.{1,2}\/[^'"]+?)(['"])/g;

function complete(file, root) {
  const text = readFileSync(file, "utf8").replace(SPECIFIER, (match, before, spec, after) => {
    const target = resolve(dirname(file), spec);
    if (/\.(js|mjs|wasm)$/.test(spec)) return match;
    if (existsSync(`${target}.js`)) return `${before}${spec}.js${after}`;
    if (existsSync(join(target, "index.js"))) return `${before}${spec}/index.js${after}`;
    throw new Error(`${relative(root, file)} imports ${spec}, which is not there`);
  });
  writeFileSync(file, text);
}

function copyTree(from, to) {
  mkdirSync(to, { recursive: true });
  for (const name of readdirSync(from)) {
    const source = join(from, name);
    if (statSync(source).isDirectory()) {
      copyTree(source, join(to, name));
    } else if (name.endsWith(".js") || name.endsWith(".wasm")) {
      copyFileSync(source, join(to, name));
    }
  }
}

function check(dir, root) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      check(path, root);
    } else if (name.endsWith(".js")) {
      const text = readFileSync(path, "utf8");
      for (const match of text.matchAll(SPECIFIER)) {
        if (!existsSync(resolve(dirname(path), match[2]))) {
          throw new Error(`${relative(root, path)} imports ${match[2]}, which is not there`);
        }
      }
    }
  }
}

copyTree(join(zstd, "dist", "esm"), zstdOut);

// Ruffle: its script, the chunks it loads beside itself, the WebAssembly, and the licence files
// (MIT OR Apache-2.0 asks for them to travel with the code). Source maps are left out.
const ruffle = join(modules, "@ruffle-rs", "ruffle");
if (!existsSync(ruffle)) {
  console.error(`${ruffle} is missing: run \`npm ci\` in apps/flare-web first`);
  process.exit(1);
}
mkdirSync(join(out, "ruffle"), { recursive: true });
for (const name of readdirSync(ruffle)) {
  if (/\.(js|wasm)$/.test(name) || name.startsWith("LICENSE")) {
    copyFileSync(join(ruffle, name), join(out, "ruffle", name));
  }
}
if (!existsSync(join(out, "ruffle", "ruffle.js"))) throw new Error("ruffle.js is not in the Ruffle package");
const files = [];
(function walk(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (name.endsWith(".js")) files.push(path);
  }
})(zstdOut);
files.forEach((file) => complete(file, out));
check(zstdOut, out);

// The notice the two licences ask for, from what the packages say about themselves.
const notice = [sqlite, zstd]
  .map((dir) => {
    const pkg = read(join(dir, "package.json"));
    const repo = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url ?? "";
    return `${pkg.name} ${pkg.version}\n  licence: ${pkg.license}\n  source: ${repo}`;
  })
  .join("\n\n");
writeFileSync(
  join(out, "LICENSES.txt"),
  `Libraries Flare's browser build loads (from npm), unchanged except that zstd's module paths are completed with ".js".\n\n${notice}\n\n` +
    "SQLite itself is in the public domain (https://www.sqlite.org/copyright.html).\n" +
    "zstd itself is used under its BSD-3-Clause licence (https://github.com/facebook/zstd/blob/dev/LICENSE).\n",
);
console.log(`vendored into ${out}`);
