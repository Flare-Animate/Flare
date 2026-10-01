# Native reader tests

C++ tests for the format readers in `tnzcore`
(`flare/sources/common/flash/`, `flare/sources/common/moho/`).

## These call the real code

Each test links the actual module and calls its real entry points. The Moho
reader is resolved from the built `tnzcore.dll` at run time with
`GetProcAddress`, so the test binary carries no link-time dependency on the
module under test and picks up a rebuilt DLL without relinking.

This is deliberate. A test that re-implements the function it is testing passes
when the shipped code is broken, and goes stale silently.
`flare/sources/flare/test_flashimport.cpp` is the anti-example: it copies the
parsers out of `flashimport.cpp` into the test, so it would still pass if
`flashimport.cpp` were emptied.

## Building

The tests are not part of the Flare build. They are built separately so a test
failure can never break a release, and so they can be run without building the
whole application.

```sh
# from tests/native
cmake -B build \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DCMAKE_PREFIX_PATH=/path/to/qt/5.15.2/msvc2019_64
cmake --build build --config RelWithDebInfo
```

`flash_reader_tests` and `moho_reader_tests` need a built `tnzcore` first:

```sh
cmake --build <build_local> --config RelWithDebInfo --target tnzcore
```

Set `QT_BIN` in the environment before running. Qt5Core is not beside the test
binary, so without it the process fails to start at all — no output, exit code
0xC0000135. The readers also need `tnzcore.dll` and Flare's own runtime
dependencies on `PATH`; `run_tests.py` sets all of that.

Set these in the cache if your tree differs from the defaults:

| Variable | Default |
|----------|---------|
| `FLASH_ROOT` | `<repo root>` |
| `BUILD` | `<repo root>/build_local` |
| `ZLIB_IMPORT_LIB` | `<repo root>/vcpkg/installed/x64-windows/lib/zlib.lib` |

## Running

```sh
# generate the fixtures first
python ../flash_fixtures/generate_fixtures.py
python ../flash_fixtures/generate_trailer_fixtures.py
python ../flash_fixtures/generate_moho_fixtures.py /tmp/mohofx

# flash_reader_tests <flash fixture dir>
./build/RelWithDebInfo/flash_reader_tests ../flash_fixtures

# moho_reader_tests <moho fixture dir>
./build/RelWithDebInfo/moho_reader_tests /tmp/mohofx
```

Each prints one line per check and exits non-zero on any failure. The readers
need Qt's image plugins on the path, so set
`QT_PLUGIN_PATH=<qt>/plugins` and put `<qt>/bin` on `PATH` or JPEG decoding
silently fails and the bitmap tests report nothing extracted.

## What is covered

| Binary | Covers |
|--------|--------|
| `flash_reader_tests` | format sniffing (including deliberately misnamed files), SWF header metadata, the advertised extension list, ZIP extraction, the stale-trailer repair from issue #70, Zip-Slip refusal, malformed-input rejection, AS3 bridge degradation |
| `moho_reader_tests` | Moho container detection, equivalence of the two container forms, the minimal rig, switch layers, the older 1021 format generation, nesting and bone binding, a dangling bone parent, every rejection path, the manifest |
| `xfl_shape_tests` | the XFL `<DOMShape>` decoder |
| `shape_census` | decodes every real `<Edge>` in an extracted FLA and prints a digest |
| `shape_export` | decodes every real `<DOMShape>` and writes each as an SVG |

## Adding a test

Put it next to the code it covers, not here, when the code is a static function.
Everything reachable through a reader's public API belongs here; anything
private belongs in a test that lives in the same translation unit's directory so
it can be moved into an anonymous namespace if needed.

## The shape tools

`xfl_shape_tests` is self-contained: the geometry is written into the test, so
it needs no fixture directory. The other two take an extracted FLA.

```sh
# an extracted FLA: unzip one, or use the copy in your temp dir
unzip "Grandfather Clock and Metronome.fla" -d /tmp/fla

# decode every real edge, print a digest
./build/RelWithDebInfo/shape_census /tmp/fla

# decode every real shape and write it out as SVG
./build/RelWithDebInfo/shape_export /tmp/fla /tmp/svg

# the two independent checks over that output
python differential_shape.py /tmp/fla
python verify_shapes_svg.py /tmp/fla /tmp/svg
```

`differential_shape.py` and `verify_shapes_svg.py` contain a second, independent
Python decoder written from the format description. A C++ test that compares the
C++ decoder against itself passes when both are wrong the same way; these compare
against an implementation that was written separately, so they can actually
disagree.

This matters more than usual for this format, because the most common ways to
misread it all produce *plausible* geometry rather than an outright failure:

* treating `/` as a close-path opcode, when it is a `lineTo`;
* decoding the `cubics` attribute, which on real documents describes a different
  outline from `edges`;
* reading a straight segment as contributing two points instead of one, which
  shifts every control point after it;
* recording a restated `moveTo` as a point, which desynchronises the point list
  from the segment list.

Each of those was found by one of these two checks, not by the C++ tests.
