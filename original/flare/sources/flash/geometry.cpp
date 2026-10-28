#include "geometry.h"

#include <cmath>

namespace Flash {
namespace {
bool digit(QChar value) { return value >= '0' && value <= '9'; }
bool hexDigit(QChar value) {
  return digit(value) || (value >= 'a' && value <= 'f') ||
         (value >= 'A' && value <= 'F');
}

// Returns the end of a decimal or '#'-prefixed hex coordinate token beginning
// at position; equal to position when no token characters are present.
int scanCoordinate(const QString &data, int position) {
  const auto run = [&](bool (*accept)(QChar)) {
    while (position < data.size() && accept(data[position])) ++position;
  };
  const auto take = [&](QChar a, QChar b) {
    if (position >= data.size() || (data[position] != a && data[position] != b))
      return false;
    ++position;
    return true;
  };
  if (position < data.size() && data[position] == '#') {
    ++position;
    run(hexDigit);
    if (take('.', '.')) run(hexDigit);
  } else {
    take('-', '+');
    run(digit);
    if (take('.', '.')) run(digit);
    if (take('e', 'E')) {
      take('-', '+');
      run(digit);
    }
  }
  return position;
}

class EdgeDecoder {
  const QString &m_data;
  int m_position = 0;

public:
  explicit EdgeDecoder(const QString &data) : m_data(data) {}
  void spaces() {
    while (m_position < m_data.size() && m_data[m_position].isSpace())
      ++m_position;
  }
  bool done() {
    spaces();
    return m_position == m_data.size();
  }
  QChar command() {
    spaces();
    if (m_position == m_data.size()) throw Error("Missing XFL edge command.");
    return m_data[m_position++];
  }
  double coordinate() {
    spaces();
    const int start = m_position;
    m_position      = scanCoordinate(m_data, m_position);
    if (m_position == start) throw Error("Missing XFL edge coordinate.");
    return edgeCoordinate(m_data.mid(start, m_position - start));
  }
  Point point() {
    const double x = coordinate();
    if (m_position >= m_data.size() || !m_data[m_position].isSpace())
      throw Error("XFL point coordinates require a separator.");
    const double y = coordinate();
    return {x, y};
  }
  unsigned selection() {
    spaces();
    if (m_position == m_data.size() || m_data[m_position] != 'S') return 0;
    ++m_position;
    const int start = m_position;
    while (m_position < m_data.size() && digit(m_data[m_position]))
      ++m_position;
    bool ok              = false;
    const unsigned value = m_data.mid(start, m_position - start).toUInt(&ok);
    if (!ok || value > 7) throw Error("Invalid XFL edge selection flags.");
    return value;
  }
};

}  // namespace

double edgeCoordinate(const QString &token) {
  double twips = 0;
  bool ok      = false;
  if (token.startsWith('#')) {
    const QString encoded  = token.mid(1);
    const int dot          = encoded.indexOf('.');
    const QString integer  = dot < 0 ? encoded : encoded.left(dot);
    const QString fraction = dot < 0 ? QString() : encoded.mid(dot + 1);
    if (integer.isEmpty() || integer.size() > 6 ||
        (dot >= 0 && (fraction.isEmpty() || fraction.size() > 2)))
      throw Error("Invalid XFL hexadecimal coordinate: " + token);
    for (QChar value : integer + fraction)
      if (!hexDigit(value))
        throw Error("Invalid XFL hexadecimal coordinate: " + token);
    const quint32 encodedInteger = integer.toUInt(&ok, 16);
    if (!ok) throw Error("Invalid XFL hexadecimal coordinate: " + token);
    const qint32 signedInteger = encodedInteger & 0x800000
                                     ? qint32(encodedInteger) - 0x1000000
                                     : qint32(encodedInteger);
    unsigned fractionalByte    = 0;
    if (!fraction.isEmpty()) {
      fractionalByte = fraction.toUInt(&ok, 16);
      if (!ok) throw Error("Invalid XFL hexadecimal fraction: " + token);
      if (fraction.size() == 1) fractionalByte *= 16;
    }
    twips = signedInteger + fractionalByte / 256.0;
  } else {
    twips = token.toDouble(&ok);
    if (!ok) throw Error("Invalid XFL decimal coordinate: " + token);
  }
  if (!std::isfinite(twips) || std::abs(twips) > 1000000000)
    throw Error("XFL coordinate is outside the supported range.");
  return twips / 20.0;
}

QVector<Segment> decodeEdges(const QString &data, quint64 maxSegments,
                             const std::function<bool()> &cancelled) {
  EdgeDecoder decoder(data);
  QVector<Segment> result;
  Point current;
  bool moved = false, startsContour = true;
  quint64 commands = 0;
  while (!decoder.done()) {
    if ((commands++ % 1024) == 0 && cancelled && cancelled())
      throw Error("Geometry reading cancelled.");
    const QChar command = decoder.command();
    if (command == '!') {
      const Point point = decoder.point();
      startsContour     = result.isEmpty() || !(point == current);
      current           = point;
      decoder.selection();  // validated; editor selection state is not kept
      moved = true;
      continue;
    }
    if (!moved) throw Error("XFL edges must start with a move command.");
    if (quint64(result.size()) >= maxSegments)
      throw Error("XFL geometry segment budget exceeded.");
    Segment segment;
    segment.start         = current;
    segment.startsContour = startsContour;
    if (command == '[') {
      segment.quadratic = true;
      segment.control   = decoder.point();
      segment.end       = decoder.point();
    } else if (command == '|' || command == '/') {
      segment.end     = decoder.point();
      segment.control = {(segment.start.x + segment.end.x) / 2,
                         (segment.start.y + segment.end.y) / 2};
    } else {
      throw Error("Unsupported XFL edge command: " + QString(command));
    }
    result.push_back(segment);
    current       = segment.end;
    startsContour = false;
  }
  return result;
}

Point Affine::apply(Point point) const {
  return {a * point.x + c * point.y + tx, b * point.x + d * point.y + ty};
}
Affine Affine::operator*(const Affine &child) const {
  return {a * child.a + c * child.b,        b * child.a + d * child.b,
          a * child.c + c * child.d,        b * child.c + d * child.d,
          a * child.tx + c * child.ty + tx, b * child.tx + d * child.ty + ty};
}
void Affine::validate() const {
  for (double value : {a, b, c, d, tx, ty})
    if (!std::isfinite(value) || std::abs(value) > 1000000000)
      throw Error("Invalid XFL transformation matrix.");
}
}  // namespace Flash
