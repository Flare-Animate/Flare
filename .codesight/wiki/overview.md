# Flare — Overview

> **Navigation aid.** This article shows WHERE things live (routes, models, files). Read actual source files before implementing new features or making changes.

**Flare** is a rust project built with raw-http.

## Scale

5 database models · 45 library files · 7 middleware layers · 21 environment variables

**Database:** unknown, 5 models — see [database.md](./database.md)

**Libraries:** 45 files — see [libraries.md](./libraries.md)

## High-Impact Files

Changes to these files have the widest blast radius across the codebase:

- `original/tests/native/rw3_golden.py` — imported by **5** files
- `original/tools/flash/ts/html5-import.ts` — imported by **3** files
- `crates/foundation/flare-document/src/tests.rs` — imported by **2** files
- `crates/services/flare-runtime/src/tests.rs` — imported by **2** files
- `docs/temp/Flare-Architecture-Pack/scripts/validate_pack.py` — imported by **2** files
- `docs/temp/Flare-Architecture-Pack/scripts/compat_report.py` — imported by **2** files

## Required Environment Variables

- `CARGO_TARGET_DIR` — `original/tools/flash/xfl_handler.py`
- `CXX` — `original/tests/native/gen_ikjacobian_cases.py`
- `DISPLAY` — `original/scripts/dev/qa/qa_driver.py`
- `FLARE_BIN` — `original/scripts/dev/qa/qa_driver.py`
- `FLARE_CLI` — `original/tools/flash/xfl_handler.py`
- `FLARE_MOHO_FIXTURES` — `original/tests/native/mutation_check.py`
- `HOMEBREW_PREFIX` — `original/tests/native/run_tests.py`
- `QA_CHROMIUM` — `original/scripts/dev/qa/html5_check.js`
- `QT_DIR` — `original/tools/flash/swfshape_oracle.py`
- `USERPROFILE` — `original/tests/run_rust.py`

---
_Back to [index.md](./index.md) · Generated 2026-10-29_