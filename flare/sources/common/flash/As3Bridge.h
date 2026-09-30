// As3Bridge.h - client for the optional flare-as3 helper
// Copyright (c) 2026 Flare Project
//
// Flare's built-in readers cover every Flash container except ActionScript:
// pulling AS3 source out of an ABC block needs a real AVM2 decompiler. The
// Next2Flash merge vendors one (MIT) behind a small JSON CLI, tools/flash/
// next2flash/flare_as3_bridge.py.
//
// It is deliberately an *optional* sidecar, modelled on how Flare already
// treats FFmpeg: the binary is probed once at startup and, if it is missing or
// reports itself unavailable, the AS3 menu entries grey out and everything else
// -- FLA/XFL/SWF import, bitmaps, the timeline -- works with no Python at all.
//
// Protocol: one JSON object on stdout per invocation. See
// doc/NEXT2FLASH_INTEGRATION.md and tools/flash/next2flash/README.md.

#ifndef AS3BRIDGE_H_
#define AS3BRIDGE_H_

#include "tcommon.h"
#include "tfilepath.h"
#include <QString>
#include <QStringList>
#include <QVector>

namespace As3Bridge {

// One class recovered from an ABC block.
struct ActionScriptClass {
    QString name;        // e.g. "Main"
    QString block;       // e.g. "block_0" (DoABC blocks within one SWF)
    QString source;      // decompiled .as, empty when not requested
};

// Result of a bridge call. `ok` means the call itself succeeded; the payload
// fields are only meaningful then.
struct Result {
    bool ok = false;
    QString error;
    QVector<ActionScriptClass> classes;
    int replaced = 0;    // patch: constant strings rewritten
    int blocks = 0;      // patch/decompile: ABC blocks seen
};

// Is the helper usable? Probed once and cached; safe to call from the UI thread
// after the first call. Never blocks for longer than a short process launch.
bool isAvailable();

// Version string reported by the helper, empty when unavailable.
QString version();

// Decompile every AS3 class in `swf` to ActionScript under `outDir`.
// Returns ok=false with `error` set when the helper is unavailable, so callers
// can distinguish "no AS3 support installed" from "this file has no AS3".
Result decompile(const TFilePath &swf, const TFilePath &outDir);

// Rewrite AS3 constant strings in `swf` using `patchJson`
// ({"strings": {"old": "new", ...}}) and write the result to `outSwf`.
// Every other tag is re-emitted byte-identical, so this is safe to run on a
// published movie.
Result patchStrings(const TFilePath &swf, const TFilePath &patchJson,
                    const TFilePath &outSwf);

// Recompile ActionScript back into an SWF. Needs the Flex SDK toolchain, which
// is not part of the vendored slice, so this reports itself unsupported.
Result compile(const TFilePath &sourceDir, const TFilePath &outSwf);

}  // namespace As3Bridge

#endif  // AS3BRIDGE_H_
