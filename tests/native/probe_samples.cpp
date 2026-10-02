// probe_samples -- run the shipped Flash/Moho readers over real files and report
// what they actually see.
//
// This is deliberately a *probe*, not a test: it takes arbitrary paths and
// prints what each reader returned, so a gap in coverage shows up as a zero
// rather than as a pass. Everything goes through the exported tnzcore API the
// importer itself uses, so a result here is a result for the shipped code.
//
// usage: probe_samples <file-or-directory> ...
#include "MohoReader.h"
#include "tfilepath.h"
#include "SWFAssets.h"
#include "XFLReader.h"
#include "ZipArchive.h"

#include <QCoreApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QTemporaryDir>
#include <cstdio>

static QString fmtName(FlashAssets::Format f) {
  switch (f) {
  case FlashAssets::Format::Swf:     return "SWF";
  case FlashAssets::Format::Zip:     return "ZIP (FLA/XFL/SWC)";
  case FlashAssets::Format::Ole2Fla: return "OLE2/CFBF (binary FLA)";
  case FlashAssets::Format::IsoBmff: return "ISO-BMFF (F4V)";
  case FlashAssets::Format::Unknown: return "unknown";
  }
  return "?";
}

static QString containerName(Moho::Container c) {
  switch (c) {
  case Moho::Container::Zip:     return "zip";
  case Moho::Container::RawJson: return "raw-json";
  case Moho::Container::Legacy:  return "legacy .anme";
  case Moho::Container::Unknown: return "unknown";
  }
  return "?";
}

static void probeFile(const QString &path) {
  const QFileInfo fi(path);
  printf("\n=== %s  (%.1f KB) ===\n", qPrintable(fi.fileName()),
         fi.size() / 1024.0);

  const FlashAssets::Format fmt = FlashAssets::detectFormat(path);
  printf("  flash detected : %s\n", qPrintable(fmtName(fmt)));

  QFile f(path);
  if (!f.open(QIODevice::ReadOnly)) {
    printf("  cannot open    : %s\n", qPrintable(f.errorString()));
    return;
  }
  const QByteArray bytes = f.readAll();
  f.close();

  if (fmt == FlashAssets::Format::Swf) {
    const FlashAssets::SwfInfo si = FlashAssets::readSwfHeader(path);
    printf("  swf header     : %s", si.valid ? "valid" : "INVALID");
    if (si.valid)
      printf("  version=%d  %dx%d  %d fps", si.version, si.width, si.height,
             si.frameRate);
    printf("\n");
    const FlashAssets::SwfContent c = FlashAssets::censusSwf(bytes);
    printf("  swf census     : bitmap=%d shape=%d text=%d font=%d video=%d "
           "audio=%d stream=%d sprite=%d action=%d abc=%d binary=%d\n",
           c.bitmaps, c.shapes, c.texts, c.fonts, c.video, c.audio, c.streams,
           c.sprites, c.actions, c.abc, c.binary);
    QTemporaryDir dir;
    const QStringList wrote = FlashAssets::extractSwfBitmaps(bytes, dir.path());
    const QStringList audio = FlashAssets::extractSwfAudio(bytes, dir.path());
    printf("  swf extracted  : %d bitmap(s), %d audio\n", wrote.size(),
           audio.size());
    for (const QString &n : wrote) printf("      %s\n", qPrintable(n));
  }

  if (fmt == FlashAssets::Format::Ole2Fla) {
    printf("  OLE2 binary FLA (the legacy CFBF container)\n");
    QTemporaryDir dir;
    const QStringList wrote =
        FlashAssets::extractLegacyFlaBitmaps(bytes, dir.path());
    printf("  carved bitmaps : %d\n", wrote.size());
    for (const QString &n : wrote) printf("      %s\n", qPrintable(n));
  }

  if (fmt == FlashAssets::Format::Zip) {
    std::string detail;
    QTemporaryDir dir;
    const bool ok = FlareZip::extract(TFilePath(path),
                                     TFilePath(dir.path()), detail);
    printf("  zip extract    : %s", ok ? "ok" : "FAILED");
    if (!ok) printf("  (%s)", detail.c_str());
    printf("\n");
    if (ok) {
      int png = 0, jpg = 0, xml = 0, swf = 0, other = 0, dirs = 0;
      qint64 bytesOut = 0;
      QDirIterator it(dir.path(), QDir::Files | QDir::Dirs,
                      QDirIterator::Subdirectories);
      while (it.hasNext()) {
        it.next();
        const QFileInfo f2(it.filePath());
        if (f2.isDir()) { ++dirs; continue; }
        bytesOut += f2.size();
        const QString n = f2.fileName().toLower();
        if (n.endsWith(".png")) ++png;
        else if (n.endsWith(".jpg") || n.endsWith(".jpeg")) ++jpg;
        else if (n.endsWith(".xml")) ++xml;
        else if (n.endsWith(".swf")) ++swf;
        else ++other;
      }
      printf("  contents       : %d png, %d jpg, %d swf, %d xml, %d other, "
             "%d dir(s), %.1f MB out\n",
             png, jpg, swf, xml, other, dirs, bytesOut / 1048576.0);

      // The document census: what the XFL says, which is what the import
      // dialog reports to the user. Driven through the same Reader the
      // importer uses, on the original file rather than the extracted copy, so
      // the number here is the number a user would see.
      // Braces, not parentheses: `XFL::Reader xr(TFilePath(path));` is a
      // function declaration -- the most vexing parse -- so xr would have no
      // members at all.
      XFL::Reader xr{TFilePath(path)};
      if (xr.read()) {
        const XFL::ContentCensus &c = xr.getDocument().census;
        printf("  xfl census     : shapes=%d shapeText=%d texts=%d morphs=%d "
               "bitmaps=%d symbols=%d components=%d sounds=%d videos=%d\n",
               c.shapes, c.shapeText, c.texts, c.morphs, c.bitmaps, c.symbols,
               c.components, c.sounds, c.videos);
        printf("  xfl document   : %dx%d @ %.2f fps, %zu timeline(s), %zu "
               "bitmap item(s)\n",
               xr.getDocument().width, xr.getDocument().height,
               xr.getDocument().frameRate,
               xr.getDocument().timelines.size(),
               xr.getDocument().bitmaps.size());
      } else {
        printf("  xfl census     : reader failed: %s\n",
               xr.getError().c_str());
      }
    }
  }

  // Moho: by content, not by extension. A .ssf is a Moho project and nothing
  // else, so this is the only reader that applies -- but the sniffer decides
  // that, so the answer is worth printing either way.
  Moho::Document d;
  const bool mohoOk = Moho::read(TFilePath(path), d);
  printf("  moho read      : %s", mohoOk ? "valid" : "rejected");
  if (mohoOk) {
    printf("  container=%s entry=%s version=%d  %dx%d  %d-%d @ %.2f fps\n",
           qPrintable(containerName(d.container)),
           qPrintable(d.containerEntry), d.version, d.width, d.height,
           d.startFrame, d.endFrame, d.fps);
    printf("  moho rig       : %d top-level, %d total layers, %zu bones, "
           "%zu switches, %zu keyframe tracks, %d keyframes\n",
           d.layers.size(), d.totalLayers, d.bones.size(), d.switches.size(),
           (size_t)d.keyframeTracks, d.keyframes);
    printf("  moho content   : %d shapes, %d mesh points, %d styles, %d "
           "actions\n",
           d.shapes, d.meshPoints, d.styles, d.actions);
    printf("  moho artwork   : %d referenced, %d found, %d missing\n",
           d.referencedImages.size(), d.imagesFound, d.imagesMissing);
    if (!d.referencedImages.isEmpty()) {
      printf("      first      : %s\n",
             qPrintable(d.referencedImages.first()));
      printf("      last       : %s\n",
             qPrintable(d.referencedImages.last()));
    }
    for (const Moho::Switch &s : d.switches)
      printf("      switch %s: %zu alt, rest=%s end=%s\n", qPrintable(s.name),
             s.alternatives.size(), qPrintable(s.childAtRest),
             qPrintable(s.childAtEnd));
  } else {
    printf("  (%s)\n", qPrintable(d.error));
  }
}

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc < 2) {
    fprintf(stderr, "usage: probe_samples <file-or-directory> ...\n");
    return 2;
  }
  for (int i = 1; i < argc; ++i) {
    const QString arg = QString::fromLocal8Bit(argv[i]);
    const QFileInfo fi(arg);
    if (fi.isDir()) {
      QDirIterator it(arg, QDir::Files, QDirIterator::Subdirectories);
      while (it.hasNext()) {
        it.next();
        if (!it.filePath().endsWith(".lnk", Qt::CaseInsensitive))
          probeFile(it.filePath());
      }
    } else if (fi.isFile()) {
      probeFile(arg);
    } else {
      printf("\n=== %s : no such file ===\n", qPrintable(arg));
    }
  }
  return 0;
}
