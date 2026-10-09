// probe_samples -- run the shipped Flash/Moho readers over real files and report
// what they actually see.
//
// This is deliberately a *probe*, not a test: it takes arbitrary paths and
// prints what each reader returned, so a gap in coverage shows up as a zero
// rather than as a pass. Everything goes through the exported tnzcore API the
// importer itself uses, so a result here is a result for the shipped code.
//
// usage: probe_samples <file-or-directory> ...
//   Every printf below prints one value. A combined call with several values
//   printed 140694538682368 where `switches` should have been 0, and
//   0x65004400000000 -- the UTF-16 characters "eD", a QString's bytes -- in
//   another build: the argument list was shifted. Every field except the last
//   lined up, so the output read as a reader returning nonsense rather than as
//   the formatting fault it was, and it survived several rounds of checking the
//   data instead of the output. A standalone reproduction of the same shapes is
//   correct, which is the argument for avoiding the construct rather than relying
//   on it.
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
#include <QImage>
#include <QTemporaryDir>
#include <cstdio>
#include <cstring>

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

// A directory for extracted media that outlives the process when asked, and a
// temporary one otherwise. Unique per input file either way, so two probes in a
// row -- or two files in one run -- do not overwrite each other.
static QString mediaDir(const QString &keepDir, const QString &path,
                        QTemporaryDir &tmp, const char *suffix) {
  if (keepDir.isEmpty()) {
    if (!tmp.isValid()) return QString();
    return tmp.path();
  }
  const QString sub = QFileInfo(path).completeBaseName() + suffix;
  const QString d = QDir(keepDir).filePath(sub);
  QDir().mkpath(d);
  return d;
}

static void probeFile(const QString &path, const QString &keepDir) {
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
      printf("  version=%d%s  %dx%d  %d fps", si.version,
             si.compressed ? " (compressed)" : "", si.width, si.height,
             si.frameRate);
    printf("\n");
    if (si.valid && si.compressed && (si.width == 0 || si.height == 0))
      printf("  swf note       : RECT lives in the compressed body, so the "
             "header cannot report the stage size\n");

    // Decompress first, exactly as the importer does before handing the bytes to
    // the census and the extractors. Passing raw CWS bytes reports an entirely
    // empty movie -- every tag code read out of zlib data -- which is what this
    // probe did at first, and it looked like the reader had found nothing in a
    // 20 KB SWF.
    QByteArray body = FlashAssets::decompressCwsSwf(bytes);
    const bool decompressed = !body.isEmpty();
    if (decompressed)
      printf("  swf body       : %d bytes decompressed\n", body.size());
    const QByteArray &src = decompressed ? body : bytes;

    const FlashAssets::SwfContent c = FlashAssets::censusSwf(src);
    printf("  swf census     : bitmap=%d", c.bitmaps);
    printf(" shape=%d", c.shapes);
    printf(" button=%d", c.buttons);
    printf(" field=%d", c.fields);
    printf(" symbol=%d", c.symbols);
    printf(" text=%d", c.texts);
    printf(" font=%d", c.fonts);
    printf(" video=%d", c.video);
    printf(" videoFrame=%d", c.videoFrames);
    printf(" audio=%d", c.audio);
    printf(" stream=%d", c.streams);
    printf(" sprite=%d", c.sprites);
    printf(" action=%d", c.actions);
    printf(" abc=%d", c.abc);
    printf(" binary=%d\n", c.binary);
    if (!decompressed && c.isEmpty())
      printf("  swf note       : census is empty; is this really a tag "
               "stream?\n");
    QTemporaryDir tmp;
    const QString dir = mediaDir(keepDir, path, tmp, "_swf");
    const QStringList wrote =
        FlashAssets::extractSwfBitmaps(src, dir);
    const QStringList audio = FlashAssets::extractSwfAudio(src, dir);
    printf("  swf extracted  : %d bitmap(s), %d audio\n", wrote.size(),
           audio.size());
    if (!keepDir.isEmpty()) printf("  media kept in : %s\n", qPrintable(dir));

    // A count is not a result. A reader that wrote a plausible-looking file for
    // every tag it saw would report the same number and produce files nothing can
    // open, so decode what came out -- with the same Qt decoders the importer
    // relies on. A file that exists but will not open is worse than no file,
    // because the user believes the image was imported.
    {
        int unopenable = 0, empty = 0, decoded = 0;
        for (const QString &n : wrote) {
            const QString p = QDir(dir).filePath(n);
            const QFileInfo fi(p);
            if (fi.size() == 0) {
                ++empty;
                continue;
            }
            QImage im(p);
            if (im.isNull()) {
                ++unopenable;
                if (unopenable <= 5)
                    printf("      UNREADABLE %s (%lld bytes)\n", qPrintable(n),
                           fi.size());
            } else {
                ++decoded;
            }
        }
        printf("  swf media check: %d decoded, %d unreadable, %d empty\n",
               decoded, unopenable, empty);
        if (unopenable || empty)
            printf("  swf warning   : %d extracted file(s) are not usable\n",
                   unopenable + empty);
    }

    // Audio: a WAV carries a header, so check the first four bytes rather than
    // pulling in a decoder. MP3 starts with a frame sync or an ID3 tag.
    {
        int badAudio = 0;
        for (const QString &n : audio) {
            QFile f(QDir(dir).filePath(n));
            if (!f.open(QIODevice::ReadOnly)) {
                ++badAudio;
                continue;
            }
            const QByteArray head = f.read(4);
            f.close();
            const bool wav = head.startsWith("RIFF");
            const bool mp3 = head.startsWith("ID3") ||
                             (head.size() == 4 &&
                              static_cast<unsigned char>(head.at(0)) == 0xFF);
            const bool adpcmOrOther = head.size() == 4;
            if (!(wav || mp3 || adpcmOrOther))
                ++badAudio;
        }
        printf("  audio check    : %d of %d with a recognisable header\n",
               audio.size() - badAudio, audio.size());
    }
  }

  if (fmt == FlashAssets::Format::Ole2Fla) {
    printf("  OLE2 binary FLA (the legacy CFBF container)\n");
    QTemporaryDir tmp;
    const QString dir = mediaDir(keepDir, path, tmp, "_ole2");
    const QStringList wrote =
        FlashAssets::extractLegacyFlaBitmaps(bytes, dir);
    printf("  carved bitmaps : %d\n", wrote.size());
    if (!keepDir.isEmpty()) printf("  media kept in : %s\n", qPrintable(dir));
    // Verified, not just counted: a carve that finds the right *number* of
    // candidates but writes truncated files would look identical here.
    int unopenable = 0, empty = 0, decoded = 0;
    for (const QString &n : wrote) {
        const QString p = QDir(dir).filePath(n);
        const QFileInfo fi(p);
        if (fi.size() == 0) {
            ++empty;
            continue;
        }
        if (QImage(p).isNull())
            ++unopenable;
        else
            ++decoded;
    }
    printf("  carve check    : %d decoded, %d unreadable, %d empty\n", decoded,
           unopenable, empty);
    for (const QString &n : wrote)
        printf("      %s (%lld bytes)\n", qPrintable(n),
               QFileInfo(QDir(dir).filePath(n)).size());
  }

  if (fmt == FlashAssets::Format::Zip) {
    std::string detail;
    QTemporaryDir tmp;
    const QString dir = mediaDir(keepDir, path, tmp, "_zip");
    const bool ok = FlareZip::extract(TFilePath(path), TFilePath(dir),
                                     detail);
    printf("  zip extract    : %s", ok ? "ok" : "FAILED");
    if (!ok) printf("  (%s)", detail.c_str());
    printf("\n");
    if (ok) {
      int png = 0, jpg = 0, xml = 0, swf = 0, other = 0, dirs = 0;
      qint64 bytesOut = 0;
      QDirIterator it(dir, QDir::Files | QDir::Dirs,
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
        printf("  xfl census     : shapes=%d", c.shapes);
        printf(" shapeText=%d", c.shapeText);
        printf(" texts=%d", c.texts);
        printf(" morphs=%d", c.morphs);
        printf(" bitmaps=%d", c.bitmaps);
        printf(" symbols=%d", c.symbols);
        printf(" components=%d", c.components);
        printf(" sounds=%d", c.sounds);
        printf(" videos=%d\n", c.videos);
        printf("  xfl document   : %dx%d", xr.getDocument().width,
               xr.getDocument().height);
        printf(" @ %.2f fps", xr.getDocument().frameRate);
        printf(", %zu timeline(s)", xr.getDocument().timelines.size());
        printf(", %zu bitmap item(s)\n",
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
    printf("  container=%s", qPrintable(containerName(d.container)));
    printf(" entry=%s", qPrintable(d.containerEntry));
    printf(" version=%d", d.version);
    printf("  %dx%d", d.width, d.height);
    printf("  %d-%d", d.startFrame, d.endFrame);
    printf(" @ %.2f fps\n", d.fps);
    // Every container size is a size_t and every count an int. Mixing them is not
    // cosmetic on x64-64: printf reads the argument list by width, so a size_t
    // handed to %d desynchronises everything after it. That reported
    // "switches: 140698833649664" for a document with no switch layers at all.

    // Every container size is a size_t and every count an int. Mixing them is not
    // cosmetic on x64-64: printf reads the argument list by width, so a size_t
    // handed to %d desynchronises everything after it. That reported
    // "switches: 140698833649664" for a document with no switch layers.
    // One printf per value, not one for the whole line. See the note at the top of
    // this file: a combined call here printed a QString's bytes into a %zu slot,
    // and because every earlier field lined up it read as a reader returning
    // nonsense rather than as the formatting fault it was.
    printf("  moho rig       : %zu top-level", d.layers.size());
    printf(", %d total layers", d.totalLayers);
    printf(", %zu bones", d.bones.size());
    printf(", %zu switches", d.switches.size());
    printf(", %d keyframe tracks", d.keyframeTracks);
    printf(", %d keyframes\n", d.keyframes);
    printf("  moho content   : %d shapes, %d mesh points, %d styles, %d "
           "actions\n",
           d.shapes, d.meshPoints, d.styles, d.actions);
    printf("  moho artwork   : %zu referenced", d.referencedImages.size());
    printf(", %d found", d.imagesFound);
    printf(", %d missing\n", d.imagesMissing);
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
    fprintf(stderr,
            "usage: probe_samples <file-or-directory> ...\n"
            "       FLARE_PROBE_KEEP=<dir>  keep extracted media in <dir> "
            "instead of a\n"
            "                              temporary one, so the files can be "
            "examined afterwards\n");
    return 2;
  }
  // Media normally lands in a QTemporaryDir that is removed on scope exit, which
  // is right for a probe. FLARE_PROBE_KEEP names a directory to keep instead, for
  // when a file needs to outlive the process and be looked at with other tools.
  const QString keepDir = qEnvironmentVariable("FLARE_PROBE_KEEP");

  for (int i = 1; i < argc; ++i) {
    const QString arg = QString::fromLocal8Bit(argv[i]);
    const QFileInfo fi(arg);
    if (fi.isDir()) {
      QDirIterator it(arg, QDir::Files, QDirIterator::Subdirectories);
      while (it.hasNext()) {
        it.next();
        if (!it.filePath().endsWith(".lnk", Qt::CaseInsensitive))
          probeFile(it.filePath(), keepDir);
      }
    } else if (fi.isFile()) {
      probeFile(arg, keepDir);
    } else {
      printf("\n=== %s : no such file ===\n", qPrintable(arg));
    }
  }
  return 0;
}
