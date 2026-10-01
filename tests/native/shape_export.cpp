// Decode every real <DOMShape> in an extracted FLA and write each as SVG.
//
// This is the end-to-end check on the decoder: the geometry goes in as Adobe
// wrote it and comes out as something a renderer will draw. A shape whose
// contour is empty or whose point count is suspiciously low is reported, since
// a partial decode that produces a plausible-looking path is the failure mode
// worth catching.
//
// usage: shape_export <extracted-fla-dir> <out-dir> [--limit N]
#include "XFLShape.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>
#include <cstdio>

static QFileInfoList walkXml(const QDir &dir) {
    QFileInfoList out;
    // NoDotAndDotDot, not NoDot: NoDot excludes only ".", so ".." recurses
    // forever until the stack is exhausted.
    const QFileInfoList entries =
        dir.entryInfoList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot,
                          QDir::Name);
    for (const QFileInfo &fi : entries) {
        if (fi.isDir())
            out += walkXml(QDir(fi.absoluteFilePath()));
        else if (fi.suffix().compare(QLatin1String("xml"), Qt::CaseInsensitive) == 0)
            out.append(fi);
    }
    return out;
}

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc < 3) {
        fprintf(stderr, "   usage: shape_export <extracted-fla-dir> <out-dir> "
                        "[--limit N]\n");
        return 2;
    }
    const QDir root(QString::fromLocal8Bit(argv[1]));
    const QDir outDir(QString::fromLocal8Bit(argv[2]));
    outDir.mkpath(QStringLiteral("."));
    int limit = 100000;
    for (int i = 3; i + 1 < argc; ++i)
        if (QString::fromLocal8Bit(argv[i]) == QLatin1String("--limit"))
            limit = QString::fromLocal8Bit(argv[i + 1]).toInt();

    int shapes = 0, failed = 0, empty = 0;
    int totalContours = 0, totalPoints = 0, strokes = 0;

    // A manifest, so the verifier can pair each exported SVG with the exact
    // <DOMShape> it came from rather than guessing by position.
    QFile manifest(outDir.filePath("manifest.tsv"));
    manifest.open(QIODevice::WriteOnly | QIODevice::Truncate);
    manifest.write("svg\tsource\tindex\tcontours\tpoints\n");

    // Each <DOMShape> is captured as raw text and handed to the decoder, which
    // finds the <Edge> elements inside it. A regex over the raw span is
    // adequate here and avoids a second XML parse per element.
    for (const QFileInfo &fi : walkXml(root)) {
        if (shapes >= limit) break;
        QFile f(fi.absoluteFilePath());
        if (!f.open(QIODevice::ReadOnly)) continue;
        const QByteArray doc = f.readAll();
        // The ordinal within this document. The manifest index has to be
        // per-document, not the running total, or a verifier indexing into one
        // document can only resolve the first document's rows.
        int localIndex = 0;

        // Find each <DOMShape ...> and its matching </DOMShape>, then decode the
        // span. A regex over the raw text is adequate here and avoids a second
        // XML pass per element.
        static const QRegularExpression re(
            QStringLiteral("<DOMShape\\b[^>]*>(.*?)</DOMShape>"),
            QRegularExpression::DotMatchesEverythingOption);
        QRegularExpressionMatchIterator it = re.globalMatch(QString::fromUtf8(doc));
        while (it.hasNext()) {
            if (shapes >= limit) break;
            const QRegularExpressionMatch m = it.next();
            const QString inner = m.captured(1);
            if (!inner.contains(QLatin1String("edges="))) continue;

            ++shapes;
            XFL::Shape s;
            QString err;
            if (!XFL::decodeShapeXml(inner, s, err)) {
                ++failed;
                if (failed <= 3)
                    fprintf(stderr, "   decode failed: %s\n", qPrintable(err));
                continue;
            }
            if (s.contours.isEmpty()) {
                ++empty;
                continue;
            }
            totalContours += s.contours.size();
            for (const XFL::Contour &c : s.contours)
                totalPoints += c.points.size();
            strokes += s.strokedContours;

            ++localIndex;
            const QString name =
                QStringLiteral("shape_%1.svg").arg(shapes, 5, 10, QLatin1Char('0'));
            QFile o(outDir.filePath(name));
            if (o.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                o.write(XFL::toSvgDocument(s, name).toUtf8());
                o.close();
            }
            int npts = 0;
            for (const XFL::Contour &c : s.contours) npts += c.points.size();
            // .arg with five placeholders needs the chained overload for the
            // two-argument case; the rest are ints.
            QString rel = root.relativeFilePath(fi.absoluteFilePath());
            QString row = name + QLatin1Char('\t') + rel +
                          QLatin1Char('\t') + QString::number(localIndex) +
                          QLatin1Char('\t') + QString::number(s.contours.size()) +
                          QLatin1Char('\t') + QString::number(npts) +
                          QLatin1Char('\n');
            manifest.write(row.toUtf8());
        }
    }

    manifest.close();
    printf("shapes seen          : %d\n", shapes);
    printf("decode failures      : %d\n", failed);
    printf("shapes with no contour: %d\n", empty);
    printf("contours written     : %d\n", totalContours);
    printf("points written       : %d\n", totalPoints);
    printf("stroked contours     : %d\n", strokes);
    return failed ? 1 : 0;
}
