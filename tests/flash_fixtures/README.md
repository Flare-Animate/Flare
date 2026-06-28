# Flash import test fixtures

Minimal-but-valid sample files for every Flash/Animate format Flare imports, plus
two checkers. These are deterministic inputs for exercising the Flash import
pipeline (`flare/sources/flare/flashimport.cpp`, `common/flash/XFLReader`).

| Fixture | Format | What it proves |
|---------|--------|----------------|
| `sample.swf` | Uncompressed FWS SWF (550×400 @ 24fps) | `readSwfHeader()` RECT/frameRate decode |
| `sample.flv` | FLV header (video+audio) | `readFlvHeader()` magic + flags |
| `sample.f4v` | ISO-BMFF `ftyp` box (`f4v `) | `readF4vHeader()` brand detection |
| `sample.as` | ActionScript 3 source | `.as` text import path |
| `sample_xfl/` | XFL directory + `DOMDocument.xml` | `XFLReader` directory parse |
| `sample.fla` | ZIP wrapping the XFL document | FLA = ZIP → `extractZip` → XFL parse |
| `sample.swc` | ZIP (`catalog.xml` + `library.swf`) | SWC catalog + embedded-SWF bitmap path |

## Running the checks

```sh
# (re)generate the fixtures
python generate_fixtures.py

# verify each fixture meets the importer's format contract (exit 0 = all pass)
python verify_fixtures.py
```

The matching **C++ parser unit tests** live in
`flare/sources/flare/test_flashimport.cpp` (compile standalone against Qt5Core);
they cover the same header/RECT/XML/ZipSlip/JSFL logic in-process.

> These fixtures are intentionally tiny. They validate that each format is
> recognised and its metadata parsed — not full visual fidelity, which is the
> ongoing Flash-import work (see `doc/FLASH_SUPPORT.md`).
