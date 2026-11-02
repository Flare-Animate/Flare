// ZipArchive.h - ZIP container handling for the Flash importers
// Copyright (c) 2026 Flare Project
//
// Two jobs, both of which used to be duplicated (and diverging) between
// XFLReader.cpp and flashimport.cpp:
//
// 1. **Trailer salvage.** Adobe Animate and Flash CS5+ write ".fla" (and
//    ".swc", ".zxp", ".oam", ...) as ZIP archives. A number of real FLAs in
//    the wild carry an end-of-central-directory (EOCD) record whose size/offset
//    fields do not describe the central directory that is actually in the file
//    — typically because the writer emitted a duplicate local file header (the
//    ODF-style "mimetype" entry Adobe also writes) and computed the EOCD from a
//    stale entry count. The central directory itself is intact; only the
//    22-byte trailer lies. Both Python's zipfile and the bundled minizip reject
//    such an archive outright (minizip's unzReadEndOfCentralDirRecord() bails
//    with UNZ_BADZIPFILE when eocdOffset < cdOffset + cdSize), which is what
//    produced the "Failed to extract archive (invalid/corrupt ZIP)" error on
//    perfectly valid FLA files (issue #70).
//
// 2. **Safe extraction.** A single Zip-Slip-hardened entry-point for unpacking
//    an archive, so the traversal checks live in exactly one place.
//
// Nothing is guessed during salvage: a recovered central directory is only
// accepted when a full walk of its records lands exactly on the existing EOCD
// *and* every record's back-pointer resolves to a real local file header.
//
// Format references: PKWARE APPNOTE.TXT (6.3.x) for the EOCD and central
// directory record layouts.

#ifndef ZIPARCHIVE_H_
#define ZIPARCHIVE_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <string>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_IMPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace FlareZip {

// Outcome of inspecting (and possibly repairing) an archive's trailer.
enum class Status {
    Ok,           // Trailer is self-consistent; the original path is usable.
    Repaired,     // Trailer was wrong; a corrected copy now exists at outPath.
    NotAZip,      // No end-of-central-directory record: not a ZIP at all.
    Zip64,        // ZIP64 archive: left untouched (minizip already handles it).
    Unrepairable  // Looks like a ZIP but the central directory is unusable.
};

// Inspect `zipPath` and, if — and only if — its end-of-central-directory record
// disagrees with the central directory actually present, write a corrected copy
// to `outPath`. The payload bytes are copied verbatim; only the trailer is
// rewritten. `detail` receives a human-readable description for the log.
//
// A well-formed archive costs one 64 KiB tail read and writes nothing.
DVAPI Status normalize(const TFilePath &zipPath, const TFilePath &outPath,
                       std::string &detail);

// Extract every entry of `zipPath` into `outDir`, repairing the trailer first
// when necessary. Rejects absolute member paths and ".." traversal (Zip Slip).
//
// Returns false only when the archive cannot be opened at all, or when no
// member could be written. Individual unreadable members are skipped, matching
// the historical behaviour of the per-caller extractors.
DVAPI bool extract(const TFilePath &zipPath, const TFilePath &outDir,
                   std::string &detail);

}  // namespace FlareZip

#endif  // ZIPARCHIVE_H_
