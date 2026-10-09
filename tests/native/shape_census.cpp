// Decode every real <Edge> in an extracted FLA and print a digest.
//
// This is the input to the differential test: an independent Python decoder
// computes the same figures from the same XML, and the two must agree. Running
// the C++ decoder over real geometry is the only way to know the parser works
// on the format rather than on the fixtures I wrote.
//
// usage: shape_census <extracted-fla-dir>
#include "XFLShape.h"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QXmlStreamReader>

static int gEdges = 0, gParsed = 0, gFailed = 0;
static int gContours = 0, gClosed = 0;
static double gMinX = 0, gMaxX = 0, gMinY = 0, gMaxY = 0;
static bool gAny = false;

static void visit(const QString &path) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return;
    const QByteArray bytes = f.readAll();
    QXmlStreamReader xr(bytes);
    while (!xr.atEnd()) {
        xr.readNext();
        if (xr.isStartElement() && xr.name() == QLatin1String("Edge")) {
            const QString edges =
                xr.attributes().value(QLatin1String("edges")).toString();
            if (edges.isEmpty()) continue;
            ++gEdges;
            XFL::Shape s;
            QString err;
            if (!XFL::decodeEdges(edges, false, s, err)) {
                ++gFailed;
                if (gFailed <= 3)
                    fprintf(stderr, "   parse failure: %s  (%.60s)\n",
                            qPrintable(err), qPrintable(edges));
                continue;
            }
            ++gParsed;
            for (const XFL::Contour &c : s.contours) {
                if (c.points.size() < 2) continue;
                ++gContours;
                // Contour::closed, not a reimplementation of the test with a
                // different tolerance. The differential check compares this
                // number, so recomputing it here meant the comparison was
                // measuring the census's own arithmetic rather than the field it
                // appeared to cover.
                if (c.closed) ++gClosed;
                for (const QPointF &p : c.points) {
                    if (!gAny) {
                        gMinX = gMaxX = p.x();
                        gMinY = gMaxY = p.y();
                        gAny = true;
                    } else {
                        gMinX = qMin(gMinX, p.x());
                        gMaxX = qMax(gMaxX, p.x());
                        gMinY = qMin(gMinY, p.y());
                        gMaxY = qMax(gMaxY, p.y());
                    }
                }
            }
        }
    }
}

// Every *.xml at or below `dir`. QDir::entryInfoList with a recursive filter is
// not available in Qt5, and entryInfoList("*.xml", Recursive) does not recurse
// either -- hence the explicit walk.
static QFileInfoList walkXml(const QDir &dir) {
    QFileInfoList out;
    // NoDotAndDotDot, not NoDot: in Qt5 NoDot excludes only ".", so ".."
    // comes back as a directory entry and the recursion walks up out of the
    // tree and then back down forever until the stack gives out.
    const QFileInfoList entries =
        dir.entryInfoList(QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot,
                          QDir::Name);
    for (const QFileInfo &fi : entries) {
        if (fi.isDir()) {
            out += walkXml(QDir(fi.absoluteFilePath()));
        } else if (fi.suffix().compare(QLatin1String("xml"),
                                       Qt::CaseInsensitive) == 0) {
            out.append(fi);
        }
    }
    return out;
}

int main(int argc, char **argv) {
    QCoreApplication app(argc, argv);
    if (argc < 2) {
        fprintf(stderr, "   usage: shape_census <extracted-fla-dir>\n");
        return 2;
    }
    // A real FLA keeps its symbols several levels down (LIBRARY/Body Parts/
    // Characters/<name>/<name>.xml), so the walk has to be recursive. A
    // one-level walk finds 83 of the document's 1990 edges, which is the kind
    // of partial result that reads as a parse failure rather than as a short
    // search.
    const QDir root(QString::fromLocal8Bit(argv[1]));
    for (const QFileInfo &fi : walkXml(root)) visit(fi.absoluteFilePath());

    // Printed in a stable, greppable form so the Python side can be compared
    // field by field.
    printf("edges=%d\n", gEdges);
    printf("parsed=%d\n", gParsed);
    printf("failed=%d\n", gFailed);
    printf("contours=%d\n", gContours);
    printf("closed=%d\n", gClosed);
    if (gAny) {
        printf("minx=%.2f\n", gMinX);
        printf("maxx=%.2f\n", gMaxX);
        printf("miny=%.2f\n", gMinY);
        printf("maxy=%.2f\n", gMaxY);
    }
    return 0;
}
