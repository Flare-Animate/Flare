// SWFAssets.h - binary Flash container readers and asset extractors
// Copyright (c) 2026 Flare Project
//
// Pure format logic for the import commands in flare/sources/flare/flashimport.cpp:
// container sniffing, the SWF/FLV/F4V header readers, the SWF tag-stream bitmap
// extractor, legacy binary-FLA (OLE2/CFBF) carving, and ZIP-based FLA media
// detection.
//
// It lives in common/flash (not in the flare application) so that it has no
// dependency on the Toonz scene/xsheet layer and can be exercised directly by
// tests. flashimport.cpp keeps only the dialog, dispatch and scene-import glue.
//
// Format knowledge (see also doc/FLASH_SUPPORT.md):
//   - Adobe SWF specification, tag codes 6/8/20/21/22/23/24/35/36/90
//   - Ruffle (MIT/Apache-2.0)  - tag reference numbers and RECT bit layout
//   - [MS-CFB]                - Compound File Binary layout used by legacy FLA
//   - FLV public spec / ISO BMFF - container headers
//   - Apache Flex SDK (Apache-2.0) - not used here, referenced for SWC layout

#ifndef SWFASSETS_H_
#define SWFASSETS_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <QByteArray>
#include <QString>
#include <QStringList>

#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace FlashAssets {

// Container formats, identified from the leading bytes rather than the
// extension: Flash payloads in the wild are routinely mislabeled (".ssf" holds
// a plain SWF, FLAs get re-zipped as ".zip").
//
// The sniffer is deliberately authoritative - the extension is only consulted
// for formats with no reliable magic number, and only when the bytes tell us
// nothing. That is what lets a SWF named ".fla", or an FLA re-zipped as
// ".zip", import correctly.
enum class Format {
    Unknown,
    Swf,      // FWS / CWS / ZWS  (also .swz, .ksk: same header)
    Ole2Fla,  // legacy binary FLA (Flash CS4 and earlier); also .fls
    Zip,      // ZIP-backed XFL: .fla (CS5+), .swc, .zxp, .mxp, .ane, .air,
              // .oam, .sol (Flash Shared Library)
    IsoBmff   // .f4v / .m4v / .mp4 - ftyp box
};

DVAPI Format detectFormat(const QString &path);

// Every extension the Flash import command advertises. Kept beside the
// sniffer so the file dialog and the dispatch cannot drift apart.
DVAPI QStringList supportedExtensions();

struct SwfInfo {
    bool valid = false;
    bool compressed = false;  // zlib-compressed (SWF6+) or LZMA (SWF13+)
    int  version   = 0;
    int  width     = 0;
    int  height    = 0;
    int  frameRate = 0;
    int  frameCount = 0;
};
DVAPI SwfInfo readSwfHeader(const QString &path);

struct FlvInfo {
    bool valid    = false;
    int  version  = 0;
    bool hasVideo = false;
    bool hasAudio = false;
};
DVAPI FlvInfo readFlvHeader(const QString &path);

struct F4vInfo {
    bool    valid        = false;
    QString majorBrand;
    QString compatBrands;
};
DVAPI F4vInfo readF4vHeader(const QString &path);

// Expand a CWS (zlib) SWF into a plain FWS body, patched to uncompressed.
DVAPI QByteArray decompressCwsSwf(const QByteArray &swfData);

// FLA/XFL archives keep raw bitmap media in bin/*.dat with no wrapper; sniff the
// image type from its magic bytes and give each a real extension.
// Returns the file names written into `outDir`.
DVAPI QStringList extractFLABinaryMedia(const QString &outDir);

// True when `path` is an OLE2 / Compound File Binary document, i.e. a legacy
// binary .fla written by Flash CS4 or earlier.
DVAPI bool isOle2CompoundFile(const QString &path);

// Carve every embedded JPEG/PNG out of a legacy binary FLA. Carves per
// OLE2/CFBF stream (bitmaps live in per-symbol streams) and falls back to a
// whole-file scan; every candidate is validated by decoding it, so a false
// positive inside entropy data never yields a broken image.
DVAPI QStringList extractLegacyFlaBitmaps(const QByteArray &data,
                                          const QString &outDir);

// Walk a (decompressed) SWF tag stream and write out every embedded bitmap.
// Returns the file names written into `outDir`.
DVAPI QStringList extractSwfBitmaps(const QByteArray &swfData,
                                    const QString &outDir);

// What a SWF actually contains. The importer reports this so content it cannot
// convert is *named* rather than silently dropped -- a SWF full of vector art
// used to import as a completely empty scene with no explanation.
struct SwfContent {
    int bitmaps = 0;   // DefineBits / JPEG / lossless image tags
    int audio   = 0;   // DefineSound
    int streams = 0;   // SoundStreamHead/Block (streaming audio)
    int shapes  = 0;   // DefineShape / Shape3 / Shape4 / MorphShape
    int texts   = 0;   // DefineText / DefineText2
    int fonts   = 0;   // DefineFont / Font2 / Font3
    int sprites = 0;   // DefineSprite (nested timelines)
    int actions = 0;   // DoAction / DoInitAction (ActionScript 1/2 bytecode)
    int abc     = 0;   // DoABC / DoABC2 (ActionScript 3 bytecode)
    int video   = 0;   // DefineVideoStream
    int binary  = 0;   // DefineBinaryData

    bool isEmpty() const {
        return !(bitmaps || audio || streams || shapes || texts || fonts ||
                 sprites || actions || abc || video || binary);
    }
    // True when the movie carries art that the bitmap extractor cannot turn
    // into a level, so the user should be told why the import looks empty.
    bool hasNonBitmapArt() const {
        return shapes || texts || fonts || sprites || video;
    }
};

// Census a SWF tag stream (recursing into DefineSprite).
DVAPI SwfContent censusSwf(const QByteArray &swfData);

// Extract embedded audio. DefineSound tags become one file each; streaming
// audio (SoundStreamHead + SoundStreamBlock) is concatenated per stream.
// MP3 is written as .mp3, uncompressed 16-bit PCM as .wav, ADPCM and the
// proprietary codecs as raw .raw (they cannot be decoded without a codec we do
// not ship). Returns the file names written into `outDir`.
DVAPI QStringList extractSwfAudio(const QByteArray &swfData,
                                  const QString &outDir);

}  // namespace FlashAssets

#endif  // SWFASSETS_H_
