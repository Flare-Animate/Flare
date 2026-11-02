// Writes frames made by the browser's zstd (@bokuweb/zstd-wasm, level 3) of a known pattern, for the
// Rust test that checks Flare reads what a browser writes. The content of the frame named N.zst is
// the N bytes i % 251. Run from apps/flare-web after `npm ci`:
//   node scripts/make_zstd_fixtures.mjs ../../crates/io/flare-storage/tests/fixtures/zstd-wasm
import { init, compress } from "@bokuweb/zstd-wasm";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const out = process.argv[2];
if (!out) {
  console.error("usage: make_zstd_fixtures.mjs OUT_DIR");
  process.exit(2);
}
await init();
mkdirSync(out, { recursive: true });
for (const n of [100, 5000, 300000, 3000000]) {
  const raw = Uint8Array.from({ length: n }, (_, i) => i % 251);
  writeFileSync(join(out, `${n}.zst`), compress(raw, 3));
}
