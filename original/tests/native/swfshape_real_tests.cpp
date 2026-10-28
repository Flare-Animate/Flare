// Run the SWF shape extractor over a whole movie, and report what came out.
//
// The extract tests build small movies by hand, which cannot show what happens on a
// real one: hundreds of shapes, sprites nested several deep, and bitmaps and audio
// interleaved between them. This drives the shipped extractor over a file and prints
// the result, so the wiring is measured rather than assumed.
//
// usage: swfshape_real <file.swf|ssf> <out-dir>

#include "SWFAssets.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QXmlStreamReader>

#include <cstdio>

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc < 3) {
    fprintf(stderr, "usage: swfshape_real <file.swf> <out-dir>\n");
    return 2;
  }
  const QString src = QString::fromLocal8Bit(argv[1]);
  const QString out = QString::fromLocal8Bit(argv[2]);

  QFile f(src);
  if (!f.open(QIODevice::ReadOnly)) {
    fprintf(stderr, "cannot open %s\n", qPrintable(src));
    return 1;
  }
  const QByteArray raw = f.readAll();
  f.close();

  const QByteArray plain = FlashAssets::decompressCwsSwf(raw);
  const QByteArray &data = plain.isEmpty() ? raw : plain;

  const FlashAssets::SwfContent c = FlashAssets::censusSwf(data);

  int skipped = 0;
  const QStringList names = FlashAssets::extractSwfShapes(data, out, &skipped);

  // What actually landed on disk, and whether a level loader could read it. Counted
  // rather than assumed: a file written and never parsed is a file that does not load.
  int present = 0, parseable = 0, withPath = 0, bytes = 0, nested = 0;
  int widest = 0;
  for (const QString &n : names) {
    const QString p = QDir(out).filePath(n);
    if (!QFile::exists(p)) continue;
    ++present;
    if (n.contains(QLatin1String("sprite"))) ++nested;
    QFile f2(p);
    if (!f2.open(QIODevice::ReadOnly)) continue;
    const QByteArray svg = f2.readAll();
    f2.close();
    bytes += svg.size();
    if (svg.contains(" d=\"") && svg.contains("<path")) ++withPath;
    widest = qMax(widest, svg.size());
    QXmlStreamReader xml(svg);
    while (!xml.atEnd()) xml.readNext();
    if (!xml.hasError()) ++parseable;
  }

  fprintf(stderr, "\n-- %s --\n", qPrintable(QFileInfo(src).fileName()));
  fprintf(stderr, "  census              : %d shape tag(s)\n", c.shapes);
  fprintf(stderr, "  written             : %d file(s)\n", names.size());
  fprintf(stderr, "  refused             : %d\n", skipped);
  fprintf(stderr, "  present on disk     : %d\n", present);
  fprintf(stderr, "  inside a sprite     : %d\n", nested);
  fprintf(stderr, "  with a path         : %d\n", withPath);
  fprintf(stderr, "  parse as XML        : %d of %d\n", parseable, present);
  fprintf(stderr, "  total size          : %d bytes (largest %d)\n", bytes, widest);
  if (c.shapes != names.size())
    fprintf(stderr, "  NOTE: the census counts DefineMorphShape (46), which is not\n"
                    "        decoded, so a difference here is expected on a movie\n"
                    "        with morph shapes and a fault otherwise.\n");

  // Fail if anything was written that cannot be read back.
  if (present != names.size() || parseable != present || withPath != present)
    return 1;
  return 0;
}
