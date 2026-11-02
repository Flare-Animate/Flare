"""Regenerate/check flare/sources/common/tiio/tiio_jpg_exif_cases.txt from the original C++."""
import rw3_golden as g

raise SystemExit(g.run(
    "jpgexif", "common/tiio/tiio_jpg_exif_cases.txt",
    ["flare/sources/common/tiio/tiio_jpg_exif.cpp", "flare/sources/include/tiio_jpg_exif.h"],
))
