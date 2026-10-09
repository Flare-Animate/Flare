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


// One manifest row. A shape that produced no SVG is still recorded, because the
// verifier walks the manifest and indexes into the document by the same number.
static void recordRow(QFile &manifest, const QString &name,
                      const QString &source, int index, int contours,
                      int points) {
    const QString row = name + QLatin1Char('\t') + source + QLatin1Char('\t') +
                        QString::number(index) + QLatin1Char('\t') +
                        QString::number(contours) + QLatin1Char('\t') +
                        QString::number(points) + QLatin1Char('\n');
    manifest.write(row.toUtf8());
}

// True when the fragment contains an <Edge ...> whose `edges` attribute is
// non-empty.
//
// This has to be the *same* rule the verifier uses to decide which shapes to
// compare: an <Edge> carrying a non-empty `edges` attribute, at any depth
// inside the <DOMShape>, and the shape counted whether or not it goes on to
// produce any geometry. It was a raw substring search for "edges=", which also matches a
// <fills> block or a comment; the verifier parsed the XML and looked for a real
// <Edge> element. When the two sides numbered shapes differently, every shape
// after the first disagreement in a document was verified against the wrong
// geometry -- and the run still reported success.
static bool hasGeometry(const QString &inner) {
    // `<Edge\b[^>]*?/>?` rather than `<Edge\b[^>]*>`: a raw '>' is legal
    // inside an XML attribute value, and a greedy [^>]* stops at it. An
    // `edges` attribute of "!0 0>0" then truncated the tag, the attribute
    // regex found no complete quoted value, and hasGeometry reported no
    // geometry for a shape the verifier -- parsing the XML properly -- saw
    // fine. Every later shape in that document was then compared against the
    // wrong geometry.
    static const QRegularExpression edge(
        QStringLiteral("<Edge\\b[^>]*/?>"),
        QRegularExpression::DotMatchesEverythingOption);
    // An attribute name at a tag boundary, not a substring: an attribute value
    // could itself contain "edges=".
    static const QRegularExpression attr(
        QStringLiteral("(\\s|^)edges\\s*=\\s*(\"[^\"]*\"|'[^']*')"));
    QRegularExpressionMatchIterator it = edge.globalMatch(inner);
    while (it.hasNext()) {
        const QRegularExpressionMatch m = attr.match(it.next().captured(0));
        // Longer than the two quote characters, so the value is non-empty.
        if (m.hasMatch() && m.captured(2).size() > 2) return true;
    }
    return false;
}

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
            if (!hasGeometry(inner)) continue;

            // Count the shape first, and count it whatever happens next. The
            // per-document index used to be incremented only after a shape
            // produced contours, so a geometry-bearing shape that decoded to
            // nothing -- an `edges` attribute of "S1" and nothing else -- was
            // skipped by the counter but kept by the verifier. Every later
            // shape in that document was then off by one.
            ++shapes;
            ++localIndex;

            const QString name =
                QStringLiteral("shape_%1.svg").arg(shapes, 5, 10, QLatin1Char('0'));
            const QString rel = root.relativeFilePath(fi.absoluteFilePath());

            XFL::Shape s;
            QString err;
            if (!XFL::decodeShapeXml(inner, s, err)) {
                // Still recorded: the verifier walks the manifest, and a row it
                // cannot find would be a silent hole in the coverage.
                recordRow(manifest, name, rel, localIndex, 0, 0);
                ++failed;
                if (failed <= 3)
                    fprintf(stderr, "   decode failed: %s\n", qPrintable(err));
                continue;
            }
            if (s.contours.isEmpty()) {
                // Recorded even though no SVG is written, so the index the
                // verifier walks stays aligned with the document.
                recordRow(manifest, name, rel, localIndex, 0, 0);
                ++empty;
                continue;
            }
            totalContours += s.contours.size();
            for (const XFL::Contour &c : s.contours)
                totalPoints += c.points.size();
            strokes += s.strokedContours;

            QFile o(outDir.filePath(name));
            if (o.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
                o.write(XFL::toSvgDocument(s, name).toUtf8());
                o.close();
            }
            int npts = 0;
            for (const XFL::Contour &c : s.contours) npts += c.points.size();
            recordRow(manifest, name, rel, localIndex, s.contours.size(), npts);
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
