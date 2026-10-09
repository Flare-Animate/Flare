// As3Bridge.cpp - client for the optional flare-as3 helper
// Copyright (c) 2026 Flare Project
//
// See As3Bridge.h. The helper is an optional sidecar: every entry point
// degrades to a clear "unavailable" answer rather than throwing, so a build
// with no Python installed behaves exactly like a build where AS3 was never
// part of the plan.

#include "As3Bridge.h"

#include "tsystem.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QProcess>
#include <QStandardPaths>
#include <QTimer>

namespace As3Bridge {

namespace {

// Where the helper lives relative to the running executable, so a packaged
// build finds it without an install step. Checked in order.
QStringList candidateScriptPaths() {
    QStringList out;
    // 1. Next to the executable: tools/flash/next2flash/flare_as3_bridge.py
    //    for an in-tree run, and the same relative path in a portable bundle.
    const QString appDir = QCoreApplication::applicationDirPath();
    out << appDir + "/tools/flash/next2flash/flare_as3_bridge.py"
        << appDir + "/../tools/flash/next2flash/flare_as3_bridge.py"
        << appDir + "/../Resources/tools/flash/next2flash/flare_as3_bridge.py";
    // 2. An explicit override, then FLAREROOT (the variable Flare itself uses
    //    to locate its installation, cf. issue #65), then PATH.
    const QString env = qEnvironmentVariable("FLARE_AS3_BRIDGE");
    if (!env.isEmpty()) out.prepend(env);
    const QString root = qEnvironmentVariable("FLAREROOT");
    if (!root.isEmpty())
        out << root + "/tools/flash/next2flash/flare_as3_bridge.py";

    // 3. Walk up from the executable. A development build puts Flare.exe in
    //    build_local/RelWithDebInfo, so the script is four levels above it and
    //    none of the fixed candidates resolve -- the bridge reported unavailable
    //    with the helper sitting in the tree, and every SWF import silently lost
    //    its ActionScript while the dialog claimed the helper was not installed.
    //    A portable bundle happened to work, because there the script really is
    //    beside the executable, which is why this went unnoticed.
    //
    //    Bounded at six levels: enough for build/<config> and
    //    build_local/<config> with a subdirectory in between, and it cannot walk
    //    off the top of a drive.
    const QString relative =
        "/tools/flash/next2flash/flare_as3_bridge.py";
    QDir dir(appDir);
    for (int up = 0; up < 6; ++up) {
        const QString here = dir.absolutePath() + relative;
        if (!out.contains(here)) out << here;
        if (!dir.cdUp()) break;
    }

    out << "flare_as3_bridge.py";
    return out;
}

QString findScript() {
    for (const QString &p : candidateScriptPaths()) {
        const QFileInfo fi(p);
        if (fi.isFile() && fi.isReadable()) return fi.absoluteFilePath();
    }
    return QString();
}

QString findPython() {
    for (const QString &exe : {QStringLiteral("python3"),
                               QStringLiteral("python")}) {
        const QString found =
            QStandardPaths::findExecutable(exe);
        if (!found.isEmpty()) return found;
    }
    return QString();
}

// Run the helper and parse its single JSON object.
// `error` is always set when the result is not ok, so callers never have to
// guess why nothing came back.
Result invoke(const QStringList &args, QString &error) {
    Result r;

    const QString script = findScript();
    if (script.isEmpty()) {
        error = QObject::tr("the flare-as3 helper is not installed");
        r.error = error;
        return r;
    }
    const QString python = findPython();
    if (python.isEmpty()) {
        error = QObject::tr("no Python interpreter was found (needed only for "
                            "the optional AS3 helper)");
        r.error = error;
        return r;
    }

    QProcess proc;
    proc.setProgram(python);
    proc.setArguments(QStringList() << script << args);
    proc.setWorkingDirectory(QFileInfo(script).absolutePath());

    // A decompiler run on a large SWF can take a while, but this is a hard
    // ceiling: a wedged helper must not hang the UI.
    proc.start();
    if (!proc.waitForStarted(5000)) {
        error = QObject::tr("the flare-as3 helper could not be started");
        r.error = error;
        return r;
    }
    if (!proc.waitForFinished(120000)) {
        proc.kill();
        proc.waitForFinished(2000);
        error = QObject::tr("the flare-as3 helper timed out");
        r.error = error;
        return r;
    }

    const QByteArray out = proc.readAllStandardOutput();
    if (out.trimmed().isEmpty()) {
        error = QObject::tr("the flare-as3 helper produced no output (%1)")
                    .arg(QString::fromLocal8Bit(proc.readAllStandardError())
                             .trimmed());
        r.error = error;
        return r;
    }

    QJsonParseError perr{};
    const QJsonDocument doc = QJsonDocument::fromJson(out, &perr);
    if (perr.error != QJsonParseError::NoError || !doc.isObject()) {
        error = QObject::tr("the flare-as3 helper returned malformed JSON: %1")
                    .arg(perr.errorString());
        r.error = error;
        return r;
    }

    const QJsonObject obj = doc.object();
    r.ok = obj.value("available").toBool(false) ||
           obj.value("ok").toBool(false);
    if (obj.contains("error") && !obj.value("error").isNull()) {
        const QString e = obj.value("error").toString();
        if (!e.isEmpty()) r.error = e;
    }
    if (obj.contains("replaced")) r.replaced = obj.value("replaced").toInt();
    if (obj.contains("blocks"))   r.blocks   = obj.value("blocks").toInt();

    // decompile: the helper lists the .as files it wrote, one per class.
    const QJsonArray classes = obj.value("classes").toArray();
    for (const QJsonValue &v : classes) {
        const QString entry = v.toString();
        if (entry.isEmpty()) continue;
        ActionScriptClass c;
        // The helper formats entries as "<block> (N class(es), in <dir>/)" or a
        // bare path. Keep the whole string as the display name and derive the
        // block from the directory when it is obvious.
        c.name  = entry.section(QLatin1Char('('), 0, 0).trimmed();
        c.block = entry.section(QLatin1String("in "), 1, 1)
                      .section(QLatin1Char('/'), 0, 0);
        if (c.block.isEmpty()) c.block = entry;
        r.classes.append(c);
    }
    return r;
}

bool       g_probed   = false;
bool       g_available = false;
QString    g_version;
QString    g_reason;

// Run `status` and return the answer as JSON (empty on any failure).
QJsonObject probeStatus(QString &error) {
    const QString script = findScript();
    if (script.isEmpty()) {
        error = QObject::tr("the flare-as3 helper is not installed");
        return {};
    }
    const QString python = findPython();
    if (python.isEmpty()) {
        error = QObject::tr("no Python interpreter was found (needed only for "
                            "the optional AS3 helper)");
        return {};
    }
    QProcess p;
    p.start(python, {script, QStringLiteral("status")});
    if (!p.waitForFinished(10000)) {
        error = QObject::tr("the flare-as3 helper did not respond");
        return {};
    }
    const QJsonObject o =
        QJsonDocument::fromJson(p.readAllStandardOutput()).object();
    if (o.isEmpty())
        error = QObject::tr("the flare-as3 helper returned no status");
    return o;
}

}  // namespace

// ---------------------------------------------------------------------------

bool isAvailable() {
    if (!g_probed) {
        g_probed = true;
        QString err;
        const QJsonObject o = probeStatus(err);
        g_available = o.value("available").toBool(false);
        g_version   = o.value("version").toString();
        if (!g_available) {
            g_reason = err;
            qDebug() << "[AS3] helper unavailable:" << err;
        }
    }
    return g_available;
}

QString version() { return isAvailable() ? g_version : QString(); }

QString unavailableReason() {
    isAvailable();
    return g_reason;
}

Result decompile(const TFilePath &swf, const TFilePath &outDir) {
    QString err;
    Result r = invoke({"decompile", swf.getQString(), outDir.getQString()}, err);
    if (!r.ok && r.error.isEmpty()) r.error = err;
    return r;
}

Result patchStrings(const TFilePath &swf, const TFilePath &patchJson,
                    const TFilePath &outSwf) {
    QString err;
    Result r = invoke({"patch", swf.getQString(), patchJson.getQString(),
                       outSwf.getQString()}, err);
    if (!r.ok && r.error.isEmpty()) r.error = err;
    return r;
}

Result compile(const TFilePath &sourceDir, const TFilePath &outSwf) {
    QString err;
    Result r = invoke({"compile", sourceDir.getQString(), outSwf.getQString()}, err);
    if (!r.ok && r.error.isEmpty()) r.error = err;
    return r;
}

}  // namespace As3Bridge
