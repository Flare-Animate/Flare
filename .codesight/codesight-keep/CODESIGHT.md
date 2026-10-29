# Flare — AI Context Map

> **Stack:** raw-http | none | unknown | rust

> 0 routes | 5 models | 0 components | 45 lib files | 21 env vars | 7 middleware | 38 events | 100% test coverage
> **Token savings:** this file is ~5,100 tokens. Without it, AI exploration would cost ~34,700 tokens. **Saves ~29,600 tokens per conversation.**
> **Last scanned:** 2026-10-29 10:45 — re-run after significant changes

---

# Schema

### project
- singleton: integer (pk)
- project_id: text (required, fk)
- schema_version: integer (required)
- saved_revision: integer (required)
- title: text (required)
- settings_json: text (required)
- created_utc: text (required)
- saved_utc: text (required)
- kind: text (required)
- payload_version: integer (required)
- payload_json: text (required)

### entity_reference
- owner_id: text (required, fk)
- field_path: text (required)
- target_id: text (required, fk)

### blob
- digest: bytes (pk)
- codec: text (required)
- logical_bytes: integer (required)
- stored_bytes: integer (required)
- payload: bytes (required)

### asset
- id: text (pk)
- media_type: text (required)
- embedded_digest: bytes (fk)
- linked_uri: text
- metadata_json: text (required)

### source_attachment
- id: text (pk)
- source_format: text (required)
- source_profile: text (required)
- anchor_entity_id: text (fk)
- source_path: text (required)
- content_digest: bytes (required)
- preservation_mode: text (required)
- validity: text (required)
- dependency_manifest_json: text (required)

---

# Libraries

- `docs/temp/Flare-Architecture-Pack/scripts/bootstrap.py` — function main: () -> int
- `docs/temp/Flare-Architecture-Pack/scripts/check_commit.py` — function valid_subject: (subject) -> bool, function main: () -> int
- `docs/temp/Flare-Architecture-Pack/scripts/compat_report.py` — function render: (root) -> str, function main: () -> int
- `docs/temp/Flare-Architecture-Pack/scripts/package_pack.py` — function files: (root), function main: () -> int
- `docs/temp/Flare-Architecture-Pack/scripts/validate_pack.py`
  - function load_json: (root, path)
  - function duplicate_errors: (items, key, label) -> list[str]
  - function dag_errors: (nodes, key, edges, label) -> list[str]
  - function safe_path: (path) -> bool
  - function overlaps: (left, right) -> bool
  - function claim_errors: (claims) -> list[str]
  - _...4 more_
- `original/scripts/animatecraft/ui_drive.py`
  - function mv: (x, y)
  - function click: (x, y, b)
  - function key: (name)
  - function drag: (x1, y1, x2, y2, steps)
  - function combo: (spec)
  - function dclick: (x, y)
  - _...1 more_
- `original/scripts/atlas_check.py`
  - function fail: (m)
  - function note: (m)
  - function tree_files: ()
  - function strip_fences: (t)
  - function main: ()
  - function parse_facts: ()
  - _...1 more_
- `original/scripts/check_cmake_sources.py` — function cmake_files: (), function main: () -> int
- `original/scripts/check_header_defs.py` — function main: () -> int
- `original/scripts/check_layout_profiles.py`
  - function read_registry: () -> dict
  - function menubar_ids: (path) -> tuple[set[str], list[str]]
  - function parse_room: (path) -> dict
  - function hierarchy_tree: (tokens)
  - function geometry_problems: (info) -> list[str]
  - function read_ini_keys: (path) -> set[str]
  - _...8 more_
- `original/scripts/check_preferences_ids.py` — function strip: (t)
- `original/scripts/check_theme_layout_separation.py`
  - function dirs: (path) -> list[str]
  - function stylesheet: (theme) -> Path
  - function selectors: (text) -> set[str]
  - function imports: (text) -> list[str]
  - function theme_dir: (spec) -> str
  - function effective: (theme, seen) -> set[str]
  - _...1 more_
- `original/scripts/dep_skew_check.py` — function resolved_versions: (lock_text) -> dict[str, list[str]], function main: () -> int
- `original/scripts/dev/contrast_check.py` — function lum: (h), function cr: (a, b)
- `original/scripts/dev/gen_liquidglass.py` — function ramp: (l), function remap: (m)
- `original/scripts/dev/qa/qa_driver.py`
  - function sh: (cmd, **kw)
  - function xdo: (*a)
  - function shortcut_key: (app, cmd)
  - function accept_dialog: (app, key)
  - function make_vector_level: (app)
  - function crop: (path, rect)
  - _...17 more_
- `original/scripts/log_watcher.py` — function main: (), class LogHandler
- `original/scripts/port_batch.py`
  - function is_skipped_dir: (path)
  - function rel: (path)
  - function has_sibling_rs: (cpp_path)
  - function iter_cpp: ()
  - function line_count: (path)
  - function looks_risky: (path)
  - _...7 more_
- `original/stuff/profiles/layouts/shortcuts/mkpresets.py`
  - function rd: (f)
  - function wr: (f, d)
  - function derive: (ov)
- `original/thirdparty/Lz4/Lz4_131/versionsTest/test-lz4-versions.py`
  - function proc: (cmd_args, pipe, dummy)
  - function make: (args, pipe)
  - function git: (args, pipe)
  - function get_git_tags: ()
  - function sha1_of_file: (filepath)
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/ChecksumImpl.cs`
  - class ChecksumGeneratorBase
  - class CRC32Checksum
  - class AdlerChecksum
  - function Reset
  - function Update
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/CircularBuffer.cs` — function Put, function Get
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/CodecBase.cs`
  - class CodecBase
  - function Add
  - function Finish
  - function Dispose
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/Deflater.cs`
  - class Deflater
  - function Add
  - function Finish
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/DotZLib.cs`
  - enum CompressLevel
  - class ZLibException
  - interface ChecksumGenerator
  - interface Codec
  - class Info
  - function DataAvailableHandler
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/GZipStream.cs`
  - class GZipStream
  - function Dispose
  - function Read
  - function ReadByte
  - function Write
  - function WriteByte
  - _...3 more_
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/Inflater.cs`
  - class Inflater
  - function Add
  - function Finish
- `original/thirdparty/zlib-1.2.8/contrib/dotzlib/DotZLib/UnitTests.cs`
  - class CircBufferTests
  - class ChecksumTests
  - class InfoTests
  - class DeflateInflateTests
  - class GZipStreamTests
  - function byteArrEqual
  - _...13 more_
- `original/tools/flash/cfbf_reader.py`
  - function is_cfbf: (head) -> bool
  - function read_streams: (buf) -> list
  - function carve_images: (data, limit) -> list
  - class CfbfError
- `original/tools/flash/decompile_flash.py`
  - function find_decompiler: (explicit)
  - function try_run: (cmd)
  - function export_with_jpexs: (exe, swf, outdir)
  - function main: ()
- `original/tools/flash/game_export.py`
  - function as3_to_js: (src)
  - function export: (scene)
  - function scene_from_import: (outdir)
  - function load_scene: (path)
- `original/tools/flash/import_container.py`
  - function run_decompiler_on_swf: (swf_path, outdir, decompiler) -> list
  - function copy_tree: (src, dst, patterns) -> list
  - function extract_jsfl_functions: (path) -> list
  - function detect_jsfl_api_calls: (path) -> list
  - function lint_script_file: (path) -> list
  - function handle_swc: (path, outdir, decompiler) -> list
  - _...12 more_
- `original/tools/flash/lint_repo_scripts.py` — function lint_file: (path) -> List[Dict[str, str]], function main: () -> int
- `original/tools/flash/next2flash/ts/fcp-import.ts`
  - function importFcp: (parse, data) => FcpSummary
  - interface FcpSummary
  - type FcpParse
- `original/tools/flash/next2flash/ts/swf-import.ts`
  - function unzip: (b) => Map<string, Uint8Array>
  - function msgpackDecode: (b) => unknown
  - function parseN2DBlob: (b) => Parsed
  - function route
  - function swfSignature
  - function isSwf
  - _...2 more_
- `original/tools/flash/swfshape_oracle.py`
  - function build_env: (root)
  - function default_probe: ()
  - function read_tags: (buf)
  - function decompress: (path)
  - function jpexs_shapes: (jar, swf, tmp)
  - function jpexs_points: (shape)
  - _...3 more_
- `original/tools/flash/ts/html5-import.ts`
  - function parseHtml5: (js) => Html5Manifest
  - function scriptSrcs
  - interface Html5Symbol
  - interface Html5Manifest
- `original/tools/flash/webapp/app.ts` — function render: (js) => string
- `original/tools/flash/webapp/build.ts`
  - function toPlainJs: (source, file) => string
  - function bundle: () => string
  - function build: (distDir) => string[]
  - const WEBAPP_DIR
  - const REPO_ROOT
  - const IMPORT_TS
  - _...4 more_
- `original/tools/flash/xfl_handler.py`
  - function read_swf_header: (swf_path) -> Optional[Dict]
  - function find_flare_cli: () -> Optional[str]
  - function convert_swf_to_xfl: (swf_path, xfl_output, use_jpexs, stage_only, use_native) -> bool
  - class XFLFilter
  - class XFLSymbol
  - class XFLDocument
  - _...2 more_
- `original/tools/icon/extract-ico.mjs` — function pngFromIco: (buf) => void, function decodePng: (buf) => void
- `original/tools/layout_tool.py`
  - function validate: (d)
  - function export: (d, out)
  - function imp: (bundle, dest)
- `original/tools/rebrand/rename_to_flare.py`
  - function should_skip: (filepath) -> bool
  - function replace_in_file: (filepath, replacements) -> int
  - function get_replacements_for_file: (filepath) -> list
  - function main: ()
- `original/tools/rebrand/replace_openToonz_in_source.py`
  - function line_has_url: (line) -> bool
  - function process_file: (path) -> int
  - function replace_in_tr_strings: (text)
- `original/tools/sync/upstream_agent.py`
  - function map_upstream_path: (rel_path) -> str
  - function is_cmake_file: (path) -> bool
  - function run: (cmd, check, capture, cwd) -> subprocess.CompletedProcess
  - function git: (args, check, capture) -> subprocess.CompletedProcess
  - function load_state: () -> dict
  - function save_state: (state) -> None
  - _...17 more_

---

# Config

## Environment Variables

- `BUILD_DIR` (has default) — original/scripts/dev/qa/qa_driver.py
- `CARGO_TARGET_DIR` **required** — original/tools/flash/xfl_handler.py
- `CMAKE_PREFIX_PATH` (has default) — original/tests/native/run_tests.py
- `CXX` **required** — original/tests/native/gen_ikjacobian_cases.py
- `DISPLAY` **required** — original/scripts/dev/qa/qa_driver.py
- `FLARE_BIN` **required** — original/scripts/dev/qa/qa_driver.py
- `FLARE_BUILD` (has default) — original/tests/native/mutation_check.py
- `FLARE_CLI` **required** — original/tools/flash/xfl_handler.py
- `FLARE_MOHO_FIXTURES` **required** — original/tests/native/mutation_check.py
- `FLARE_SAMPLES` (has default) — original/tools/flash/tests/test_import_formats.py
- `HOMEBREW_PREFIX` **required** — original/tests/native/run_tests.py
- `KISSFFT_DATATYPE` (has default) — original/thirdparty/kiss_fft/test/testkiss.py
- `KISSFFT_OPENMP` (has default) — original/thirdparty/kiss_fft/test/testkiss.py
- `NEXT2FLASH_DIR` (has default) — original/tests/native/gen_as3_cases.py
- `PORT_C_REF` (has default) — original/tests/native/gen_avl_cases.py
- `QA_CHROMIUM` **required** — original/scripts/dev/qa/html5_check.js
- `QT_BIN` (has default) — original/tests/native/mutation_check.py
- `QT_DIR` **required** — original/tools/flash/swfshape_oracle.py
- `QTDIR` (has default) — original/tests/native/run_tests.py
- `TEMP` (has default) — original/tests/native/run_tests.py
- `USERPROFILE` **required** — original/tests/run_rust.py

## Config Files

- `Cargo.toml`

---

# Middleware

## validation
- crate-catalog — `docs/temp/Flare-Architecture-Pack/docs/architecture/crate-catalog.md`

## custom
- trepetitionguard — `original/flare/sources/flareqt/trepetitionguard.rs`
- generate_fixtures — `original/tests/flash_fixtures/generate_fixtures.py`
- generate_moho_fixtures — `original/tests/flash_fixtures/generate_moho_fixtures.py`
- generate_trailer_fixtures — `original/tests/flash_fixtures/generate_trailer_fixtures.py`
- generate_variants — `original/tests/flash_fixtures/generate_variants.py`
- generated_swf_shapes — `original/tests/flash_fixtures/generated_swf_shapes.py`

---

# Dependency Graph

## Most Imported Files (change these carefully)

- `original/tests/native/rw3_golden.py` — imported by **5** files
- `original/tools/flash/ts/html5-import.ts` — imported by **3** files
- `docs/temp/Flare-Architecture-Pack/scripts/validate_pack.py` — imported by **2** files
- `docs/temp/Flare-Architecture-Pack/scripts/compat_report.py` — imported by **2** files
- `original/flare/sources/common/flash/tests.rs` — imported by **2** files
- `original/flare/sources/common/tvectorimage/pathops/tests/common.rs` — imported by **2** files
- `original/tools/flash/cfbf_reader.py` — imported by **2** files
- `original/tools/flash/xfl_handler.py` — imported by **2** files
- `original/tools/flash/game_export.py` — imported by **2** files
- `apps/flare-cli/src/project.rs` — imported by **1** files
- `apps/flare-desktop/src/menu.rs` — imported by **1** files
- `apps/flare-desktop/src/paths.rs` — imported by **1** files
- `apps/flare-desktop/src/recovery.rs` — imported by **1** files
- `crates/automation/flare-mcp/src/tools.rs` — imported by **1** files
- `crates/document/flare-document/src/attachment.rs` — imported by **1** files
- `crates/document/flare-document/src/shape.rs` — imported by **1** files
- `crates/document/flare-document/src/sound.rs` — imported by **1** files
- `crates/document/flare-document/src/stage.rs` — imported by **1** files
- `crates/document/flare-document/src/timeline.rs` — imported by **1** files
- `crates/document/flare-document/src/validate.rs` — imported by **1** files

## Import Map (who imports what)

- `original/tests/native/rw3_golden.py` ← `original/tests/native/gen_jpgexif_cases.py`, `original/tests/native/gen_namebuilder_cases.py`, `original/tests/native/gen_perlinnoise_cases.py`, `original/tests/native/gen_potentials_cases.py`, `original/tests/native/gen_tgeometry_cases.py`
- `original/tools/flash/ts/html5-import.ts` ← `original/tools/flash/ts/html5-import.test.ts`, `original/tools/flash/webapp/app.ts`, `original/tools/flash/webapp/build.test.ts`
- `docs/temp/Flare-Architecture-Pack/scripts/validate_pack.py` ← `docs/temp/Flare-Architecture-Pack/scripts/package_pack.py`, `docs/temp/Flare-Architecture-Pack/scripts/tests/test_pack.py`
- `docs/temp/Flare-Architecture-Pack/scripts/compat_report.py` ← `docs/temp/Flare-Architecture-Pack/scripts/tests/test_pack.py`, `docs/temp/Flare-Architecture-Pack/scripts/validate_pack.py`
- `original/flare/sources/common/flash/tests.rs` ← `original/flare/sources/common/flash/ac_bridge.rs`, `original/flare/sources/common/flash/ac_xfl.rs`
- `original/flare/sources/common/tvectorimage/pathops/tests/common.rs` ← `original/flare/sources/common/tvectorimage/pathops/tests/prop_pathops.rs`, `original/flare/sources/common/tvectorimage/pathops/tests/props.rs`
- `original/tools/flash/cfbf_reader.py` ← `original/tools/flash/import_container.py`, `original/tools/flash/tests/test_flash_import_fixes.py`
- `original/tools/flash/xfl_handler.py` ← `original/tools/flash/tests/test_flash_import_fixes.py`, `original/tools/flash/tests/test_import_container.py`
- `original/tools/flash/game_export.py` ← `original/tools/flash/tests/test_flash_import_fixes.py`, `original/tools/flash/tests/test_game_export.py`
- `apps/flare-cli/src/project.rs` ← `apps/flare-cli/src/main.rs`

---

# Events & Queues

## eventemitter

- `pushbyte` [event] — `original/tests/native/gen_as3_cases.py`
- `pushshort` [event] — `original/tests/native/gen_as3_cases.py`
- `pushint` [event] — `original/tests/native/gen_as3_cases.py`
- `pushuint` [event] — `original/tests/native/gen_as3_cases.py`
- `pushstring` [event] — `original/tests/native/gen_as3_cases.py`
- `pushdouble` [event] — `original/tests/native/gen_as3_cases.py`
- `getlocal` [event] — `original/tests/native/gen_as3_cases.py`
- `getlocal_0` [event] — `original/tests/native/gen_as3_cases.py`
- `getproperty` [event] — `original/tests/native/gen_as3_cases.py`
- `getlex` [event] — `original/tests/native/gen_as3_cases.py`
- `getslot` [event] — `original/tests/native/gen_as3_cases.py`
- `findpropstrict` [event] — `original/tests/native/gen_as3_cases.py`
- `callproperty` [event] — `original/tests/native/gen_as3_cases.py`
- `newarray` [event] — `original/tests/native/gen_as3_cases.py`
- `newobject` [event] — `original/tests/native/gen_as3_cases.py`
- `jump` [event] — `original/tests/native/gen_as3_cases.py`
- `dup` [event] — `original/tests/native/gen_as3_cases.py`
- `pop` [event] — `original/tests/native/gen_as3_cases.py`
- `constructprop` [event] — `original/tests/native/gen_as3_cases.py`
- `coerce` [event] — `original/tests/native/gen_as3_cases.py`
- `setproperty` [event] — `original/tests/native/gen_as3_cases.py`
- `callpropvoid` [event] — `original/tests/native/gen_as3_cases.py`
- `label` [event] — `original/tests/native/gen_as3_cases.py`
- `iftrue` [event] — `original/tests/native/gen_as3_cases.py`
- `setlocal` [event] — `original/tests/native/gen_as3_cases.py`
- `increment` [event] — `original/tests/native/gen_as3_cases.py`
- `convert_i` [event] — `original/tests/native/gen_as3_cases.py`
- `hasnext2` [event] — `original/tests/native/gen_as3_cases.py`
- `ifstrictne` [event] — `original/tests/native/gen_as3_cases.py`
- `kill` [event] — `original/tests/native/gen_as3_cases.py`
- `lookupswitch` [event] — `original/tests/native/gen_as3_cases.py`
- `newfunction` [event] — `original/tests/native/gen_as3_cases.py`
- `pushscope` [event] — `original/tests/native/gen_as3_cases.py`
- `returnvalue` [event] — `original/tests/native/gen_as3_cases.py`
- `returnvoid` [event] — `original/tests/native/gen_as3_cases.py`
- `constructsuper` [event] — `original/tests/native/gen_as3_cases.py`
- `getlocal_1` [event] — `original/tests/native/gen_as3_cases.py`

## bullmq

- `resizable` [queue] — `original/thirdparty/libjpeg-turbo/libjpeg-turbo-2.0.6/doc/html/jquery.js`

---

# Test Coverage

> **100%** of routes and models are covered by tests
> 131 test files found

## Covered Models

- project
- entity_reference
- blob
- asset
- source_attachment

---

_Generated by [codesight](https://github.com/Houseofmvps/codesight) — see your codebase clearly_