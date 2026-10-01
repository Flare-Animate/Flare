# Tests

```sh
python tests/run_all.py
```

Runs every suite and exits non-zero if any fail. Set `QT_BIN` to your Qt
`bin` directory first, or the C++ tests cannot find Qt's image plugins and will
report that no bitmaps decoded.

| Suite | What it covers |
|-------|----------------|
| `tools/flash/tests`, `tools/sync/tests` (pytest) | the `flare-as3` bridge's JSON contract and AS3 patch pass; the upstream sync agent's path mapping |
| `tests/moho/test_moho_menu.py` | the Moho import command's menu wiring |
| `tests/native/flash_reader_tests.cpp` | format sniffing, SWF header, ZIP extraction and trailer repair, Zip-Slip refusal, malformed input, AS3 bridge degradation |
| `tests/native/moho_reader_tests.cpp` | the whole Moho reader surface |
| `tests/flash_fixtures/verify_fixtures.py` | each fixture meets the importer's format contract |

## The rules these follow

**Test the shipped code, not a copy of it.** Every test calls the real entry
points. A test that re-implements the function it is testing passes when the
shipped code is broken, and goes stale silently --
`flare/sources/flare/test_flashimport.cpp` is the example of that failure mode
and is marked superseded rather than extended.

**A bug fix gets a fixture.** Every parser bug fixed so far has a fixture that
fails without the fix: the stale ZIP trailer from issue #70, a misnamed SWF, a
foreign MIME type, a dangling bone parent, a truncated document.

**Rejections carry a reason.** A parser that cannot read something must say why.
"Failed" with no detail is the failure mode these tests exist to prevent.

**Generation is reproducible.** The fixture generators pin every archive entry
to a fixed timestamp, so regenerating produces byte-identical files and a diff
means the content actually changed.

## Layout

```
tests/
  run_all.py              run everything
  flash_fixtures/         committed fixtures + their generators
  moho/                   static checks that don't need a build
  native/                 C++ tests against tnzcore (see native/README.md)
```

`tests/native` builds separately from the application, on purpose: a test
failure must never break a release, and the tests need a built `tnzcore` to
link against. See `tests/native/README.md` for the build and run commands.

## Adding a test

Put it where it can reach the code. Anything reachable through a reader's public
API belongs in `tests/native`. A private static function belongs in a test in
the same directory as the code under test, so it can be moved into an anonymous
namespace rather than exported purely for testing.
