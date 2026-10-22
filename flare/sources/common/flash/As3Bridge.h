// As3Bridge.h - AS3 (ActionScript 3 / ABC) support, native
// Copyright (c) 2026 Flare Project
//
// The ABC parser, decompiler and patcher are Rust (common/flash/ac_as3_*.rs,
// derived from Next2Flash, MIT; see THIRD-PARTY-NOTICES.md) and are called
// directly through the flare_as3_* C ABI in flare_formats.h. There is no
// Python, no helper process and no JSON-over-stdout hop any more; only
// compile() needs an external tool (the Flex SDK's mxmlc), and only that call
// reports "unavailable" when it is missing.
//
// The public signatures are unchanged from the former sidecar client.

#ifndef AS3BRIDGE_H_
#define AS3BRIDGE_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <QString>
#include <QStringList>
#include <QVector>

// Same export dance as every other header in this directory: the bodies live in
// tnzcore, which is built with TFLASH_EXPORTS, and callers in the Flare
// executable need the import side.
#undef DVAPI
#undef DVVAR
#ifdef TFLASH_EXPORTS
#define DVAPI DV_EXPORT_API
#define DVVAR DV_EXPORT_VAR
#else
#define DVAPI DV_IMPORT_API
#define DVVAR DV_IMPORT_VAR
#endif

namespace As3Bridge {

// One class recovered from an ABC block.
struct ActionScriptClass {
    QString name;        // fully qualified, e.g. "com.example.Main"
    QString block;       // e.g. "block_0" (DoABC blocks within one SWF)
    QString source;      // decompiled .as
};

// Result of a call. `ok` means the call itself succeeded; the payload fields
// are only meaningful then.
struct Result {
    bool ok = false;
    QString error;
    QVector<ActionScriptClass> classes;
    int replaced = 0;    // patch: constants rewritten
    int blocks = 0;      // patch/decompile: ABC blocks seen
};

// The native core is linked in, so this is true whenever it answers.
DVAPI bool isAvailable();

// Core version string ("flare-as3-native/1"), empty when unavailable.
DVAPI QString version();

// Why AS3 support is unavailable; empty when available.
DVAPI QString unavailableReason();

// Decompile every AS3 class in `swf` (a SWF or a bare .abc block) to
// ActionScript under `outDir`, one block_N/ folder per DoABC block.
DVAPI Result decompile(const TFilePath &swf, const TFilePath &outDir);

// Rewrite AS3 constant strings in `swf` using `patchJson`
// ({"strings": {"old": "new", ...}}) and write the result to `outSwf`.
// Every other tag is re-emitted byte-identical.
DVAPI Result patchStrings(const TFilePath &swf, const TFilePath &patchJson,
                          const TFilePath &outSwf);

// Write an edited class back into the SWF without a compiler. Only edits to
// string literals are expressible this way; anything else reports an error
// starting with "needs compiler".
DVAPI Result patchClassSource(const TFilePath &swf, int block, int classIndex,
                              const QString &editedSource,
                              const TFilePath &outSwf);

// Recompile ActionScript sources into an SWF with the Flex SDK's mxmlc
// (FLARE_FLEX_SDK / FLEX_SDK / PATH). The only call that needs an external tool.
DVAPI Result compile(const TFilePath &sourceDir, const TFilePath &outSwf);

}  // namespace As3Bridge

#endif  // AS3BRIDGE_H_
