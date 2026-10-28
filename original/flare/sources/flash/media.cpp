#include "media.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QProcess>
#include <QTemporaryFile>
#include <QUrl>
#include <algorithm>
#include <cmath>
#include <limits>
#include <zlib.h>

namespace Flash {
namespace {

quint8 byte(const QByteArray &bytes, int at) { return quint8(bytes.at(at)); }

quint16 big16(const QByteArray &bytes, int at) {
  return (quint16(byte(bytes, at)) << 8) | byte(bytes, at + 1);
}

quint32 big32(const QByteArray &bytes, int at) {
  return (quint32(big16(bytes, at)) << 16) | big16(bytes, at + 2);
}

quint16 little16(const QByteArray &bytes, int at) {
  return quint16(byte(bytes, at)) | (quint16(byte(bytes, at + 1)) << 8);
}

void dimensions(BitmapPayload &result, quint64 maxPixels) {
  if (result.width <= 0 || result.height <= 0 || result.width > 32768 ||
      result.height > 32768 ||
      quint64(result.width) * quint64(result.height) > maxPixels)
    throw Error("Bitmap dimensions exceed the native import limit: " +
                result.member);
}

void jpegHeader(Package &package, BitmapPayload &result, quint64 maxPixels) {
  const auto &bytes = result.bytes;
  if (bytes.size() < 8 || byte(bytes, 0) != 0xff || byte(bytes, 1) != 0xd8)
    throw Error("Invalid JPEG bitmap header: " + result.member);
  int pos        = 2;
  bool frameSeen = false, scanSeen = false;
  unsigned markers = 0;
  while (pos < bytes.size()) {
    if ((++markers & 255u) == 0) package.checkCancelled();
    if (byte(bytes, pos++) != 0xff)
      throw Error("Invalid JPEG marker layout: " + result.member);
    while (pos < bytes.size() && byte(bytes, pos) == 0xff) ++pos;
    if (pos >= bytes.size()) break;
    const quint8 marker = byte(bytes, pos++);
    if (marker == 0xd9) break;
    if (marker == 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    if (pos > bytes.size() - 2)
      throw Error("Truncated JPEG marker: " + result.member);
    const int length = big16(bytes, pos);
    if (length < 2 || length > bytes.size() - pos)
      throw Error("Invalid JPEG marker length: " + result.member);
    if (marker == 0xda) {
      if (length < 6) throw Error("Invalid JPEG scan header: " + result.member);
      pos += length;
      scanSeen = true;
      break;
    }
    const bool frame = (marker >= 0xc0 && marker <= 0xc3) ||
                       (marker >= 0xc5 && marker <= 0xc7) ||
                       (marker >= 0xc9 && marker <= 0xcb) ||
                       (marker >= 0xcd && marker <= 0xcf);
    if (frame) {
      if (frameSeen || length < 8)
        throw Error("Invalid JPEG frame header: " + result.member);
      frameSeen     = true;
      result.height = big16(bytes, pos + 3);
      result.width  = big16(bytes, pos + 5);
      dimensions(result, maxPixels);
    }
    pos += length;
  }
  const int end = bytes.lastIndexOf(QByteArray::fromHex("ffd9"));
  // Adobe's JPEG DAT can append a small opaque trailer after the JPEG EOI.
  if (!frameSeen || !scanSeen || end < pos || bytes.size() - end - 2 > 32)
    throw Error("Incomplete JPEG bitmap payload: " + result.member);
}

void pngHeader(BitmapPayload &result, quint64 maxPixels) {
  const auto &bytes = result.bytes;
  if (bytes.size() < 33 ||
      !bytes.startsWith(QByteArray::fromHex("89504e470d0a1a0a")) ||
      big32(bytes, 8) != 13 || bytes.mid(12, 4) != "IHDR")
    throw Error("Invalid PNG bitmap header: " + result.member);
  result.width  = int(big32(bytes, 16));
  result.height = int(big32(bytes, 20));
  dimensions(result, maxPixels);
}

void losslessDat(Package &package, BitmapPayload &result, quint64 maxPixels) {
  const QByteArray &bytes = result.bytes;
  const auto invalid      = [&](const QString &detail) {
    throw Error("Invalid XFL lossless bitmap DAT " + detail + ": " +
                     result.member);
  };
  // Known CS5+ 32-bit bitmap record: 03 05, LE row/size, four LE frame
  // coordinates, alpha flag, and chunked zlib variant 1. Other versions and
  // variants require separate evidence and are rejected explicitly.
  if (bytes.size() < 30 || byte(bytes, 0) != 3 || byte(bytes, 1) != 5)
    invalid("header");
  const quint16 rowSize = little16(bytes, 2);
  result.width          = little16(bytes, 4);
  result.height         = little16(bytes, 6);
  dimensions(result, maxPixels);
  if (quint64(result.width) * 4 != rowSize) invalid("row size");
  const quint8 flags   = byte(bytes, 24);
  const quint8 variant = byte(bytes, 25);
  if (flags > 1 || variant != 1) invalid("flags or compression variant");
  const quint64 outputSize = quint64(rowSize) * quint64(result.height);
  if (outputSize >= quint64(std::numeric_limits<int>::max()))
    invalid("decoded size exceeds native limit");

  // The header size is untrusted until the stream decodes to exactly it.
  // Grow the output as data arrives instead of allocating it up front; the
  // extra byte detects streams that decode beyond the declared size.
  const quint64 capacity = outputSize + 1;
  const auto ensureSpace = [&](quint64 used) {
    const quint64 size = quint64(result.straightRgba.size());
    if (used < size) return;
    result.straightRgba.resize(int(std::min<quint64>(
        capacity, std::max<quint64>(size * 2, 4ull * 1024 * 1024))));
  };
  z_stream stream = {};
  if (inflateInit(&stream) != Z_OK) invalid("zlib initialization");
  struct EndInflater {
    z_stream *stream;
    ~EndInflater() { inflateEnd(stream); }
  } endInflater{&stream};

  int pos          = 26;
  bool streamEnded = false;
  quint64 chunks   = 0;
  while (true) {
    if (pos > bytes.size() - 2) invalid("truncated chunk length");
    const quint16 chunkSize = little16(bytes, pos);
    pos += 2;
    if (chunkSize == 0) break;
    if (streamEnded || chunkSize > bytes.size() - pos)
      invalid("chunk boundary");
    if (++chunks > 1000000) invalid("chunk count exceeds import limit");
    if ((chunks & 255u) == 0) package.checkCancelled();
    stream.next_in =
        reinterpret_cast<Bytef *>(const_cast<char *>(bytes.constData() + pos));
    stream.avail_in = chunkSize;
    while (stream.avail_in) {
      package.checkCancelled();
      const uInt beforeIn   = stream.avail_in;
      const uLong beforeOut = stream.total_out;
      ensureSpace(stream.total_out);
      stream.next_out = reinterpret_cast<Bytef *>(result.straightRgba.data() +
                                                  stream.total_out);
      stream.avail_out =
          uInt(quint64(result.straightRgba.size()) - stream.total_out);
      const int code = inflate(&stream, Z_NO_FLUSH);
      if (stream.total_out > outputSize) invalid("decoded size exceeds header");
      if (code == Z_STREAM_END) {
        if (stream.avail_in) invalid("trailing compressed data");
        streamEnded = true;
        break;
      }
      if (code != Z_OK ||
          (beforeIn == stream.avail_in && beforeOut == stream.total_out))
        invalid("zlib stream");
    }
    pos += chunkSize;
  }
  if (!streamEnded || stream.total_out != outputSize || pos != bytes.size())
    invalid("incomplete or trailing stream");
  result.straightRgba.resize(int(outputSize));

  // Adobe stores each source row top-down as A,R,G,B. With alpha, channels
  // are premultiplied using alpha/256 and partial alpha is encoded as A+1.
  // Flare OVL rasters use straight RGBA, so invert that convention here.
  char *pixels = result.straightRgba.data();
  for (quint64 at = 0; at < outputSize; at += 4) {
    if ((at & 0x3ffffu) == 0) package.checkCancelled();
    const quint8 storedA = quint8(pixels[at]);
    quint8 r             = quint8(pixels[at + 1]);
    quint8 g             = quint8(pixels[at + 2]);
    quint8 b             = quint8(pixels[at + 3]);
    quint8 alpha         = 255;
    if (flags == 1) {
      if (storedA == 1) invalid("partial alpha value");
      alpha = storedA == 255 ? 255 : storedA == 0 ? 0 : storedA - 1;
      if (storedA == 0) {
        r = g = b = 0;
      } else if (storedA != 255) {
        if (r > storedA || g > storedA || b > storedA)
          invalid("premultiplied channel");
        const auto straight = [alpha](quint8 value) {
          const int channel = int(value) * 256 / int(alpha);
          return quint8(channel > 255 ? 255 : channel);
        };
        r = straight(r);
        g = straight(g);
        b = straight(b);
      }
    }
    pixels[at]     = char(r);
    pixels[at + 1] = char(g);
    pixels[at + 2] = char(b);
    pixels[at + 3] = char(alpha);
  }
  result.encoding = BitmapPayload::Encoding::LosslessDat;
}

QString compiledMember(Package &package, const QString &href) {
  if (package.contains(href)) return href;
  const QString bin = "bin/" + href;
  if (package.contains(bin)) return bin;
  throw Error("Missing compiled bitmap media: " + href);
}

QString sourceMember(Package &package, const QString &href) {
  const QString member = "LIBRARY/" + href;
  if (package.contains(member)) return member;
  const QString decoded = "LIBRARY/" + QUrl::fromPercentEncoding(href.toUtf8());
  if (package.contains(decoded)) return decoded;
  throw Error("Missing embedded bitmap source: " + href);
}

// The MPEG audio layer III frames of an MP3 file.
struct Mp3Stream {
  int sampleRate = 0, channels = 0, samplesPerFrame = 0;
  qint64 frames    = 0;              // Audio frames
  qint64 tagFrames = 0;              // A leading Xing/Info/VBRI frame: silence
  int audioBegin = 0, audioEnd = 0;  // Bytes of the audio frames
};

// Reads the frames of `data`: ID3v2 tags first, then consecutive layer III
// frames of one version, rate and channel count, then nothing but an ID3v1,
// APE or Lyrics tag or zero padding (a truncated last frame is left out).
// Returns why the data is not such a stream, empty when it is.
QString mp3Stream(const QByteArray &data, Mp3Stream &stream,
                  const std::function<void()> &check) {
  int pos = 0;
  while (pos <= data.size() - 10 && data.mid(pos, 3) == "ID3") {
    quint32 size = 0;
    for (int at = 6; at < 10; ++at) {
      if (byte(data, pos + at) & 0x80) return "invalid ID3 tag size";
      size = (size << 7) | byte(data, pos + at);
    }
    const qint64 next =
        qint64(pos) + 10 + size + ((byte(data, pos + 5) & 0x10) ? 10 : 0);
    if (next > data.size()) return "the ID3 tag runs past the data";
    pos = int(next);
  }
  static const int mpeg1Rates[15]    = {0,   32,  40,  48,  56,  64,  80, 96,
                                        112, 128, 160, 192, 224, 256, 320};
  static const int mpeg2Rates[15]    = {0,  8,  16, 24,  32,  40,  48, 56,
                                        64, 80, 96, 112, 128, 144, 160};
  static const int sampleRates[4][3] = {{11025, 12000, 8000},  // MPEG 2.5
                                        {0, 0, 0},
                                        {22050, 24000, 16000},   // MPEG 2
                                        {44100, 48000, 32000}};  // MPEG 1
  int version                        = -1;
  stream.audioBegin                  = pos;
  for (quint32 visited = 1; pos <= data.size() - 4; ++visited) {
    if ((visited & 1023) == 0 && check) check();
    const quint32 header   = big32(data, pos);
    const int frameVersion = (header >> 19) & 3;
    const int layer        = (header >> 17) & 3;
    const int bitrate      = (header >> 12) & 15;
    const int rate         = (header >> 10) & 3;
    const bool frame       = (header >> 21) == 0x7ff && frameVersion != 1 &&
                       layer == 1 && bitrate != 0 && bitrate != 15 && rate != 3;
    if (!frame) {
      const QByteArray rest = data.mid(pos);
      if ((rest.startsWith("TAG") && rest.size() == 128) ||
          rest.startsWith("APETAGEX") || rest.startsWith("LYRICSBEGIN") ||
          std::all_of(rest.cbegin(), rest.cend(),
                      [](char c) { return c == 0; }))
        break;
      if ((header >> 21) == 0x7ff && layer != 1)
        return "the data is MPEG audio, but not layer III";
      return QString("MP3 frame sync is lost at byte %1").arg(pos);
    }
    const bool mpeg1     = frameVersion == 3;
    const int channels   = ((header >> 6) & 3) == 3 ? 1 : 2;
    const int sampleRate = sampleRates[frameVersion][rate];
    const int kbps       = (mpeg1 ? mpeg1Rates : mpeg2Rates)[bitrate];
    const int size =
        (mpeg1 ? 144 : 72) * kbps * 1000 / sampleRate + ((header >> 9) & 1);
    if (version < 0) {
      version                = frameVersion;
      stream.sampleRate      = sampleRate;
      stream.channels        = channels;
      stream.samplesPerFrame = mpeg1 ? 1152 : 576;
    } else if (frameVersion != version || sampleRate != stream.sampleRate ||
               channels != stream.channels)
      return QString("MP3 frames change their layout at byte %1").arg(pos);
    if (size > data.size() - pos) break;  // a truncated last frame
    // An encoder's tag frame (Xing/Info after the side information, VBRI
    // at byte 36) holds no audio; it decodes as silence.
    if (stream.frames == 0 && stream.tagFrames == 0) {
      const int sideInfo =
          mpeg1 ? (channels == 1 ? 17 : 32) : (channels == 1 ? 9 : 17);
      const int tag = pos + 4 + (((header >> 16) & 1) ? 0 : 2) + sideInfo;
      const QByteArray magic = data.mid(tag, 4);
      if (magic == "Xing" || magic == "Info" ||
          data.mid(pos + 36, 4) == "VBRI") {
        stream.tagFrames = 1;
        pos += size;
        stream.audioBegin = pos;
        continue;
      }
    }
    ++stream.frames;
    pos += size;
  }
  stream.audioEnd = pos;
  if (stream.frames == 0) return "the data has no MP3 audio frames";
  return {};
}

}  // namespace

BitmapPayload readEncodedBitmap(Package &package,
                                const QMap<QString, QString> &attributes,
                                quint64 maxPixels) {
  const QString compiled = attributes.value("bitmapDataHRef");
  const QString href     = attributes.value("href");
  if (compiled.isEmpty() && href.isEmpty())
    throw Error("Bitmap item has no embedded media reference.");
  BitmapPayload result;
  result.member = !compiled.isEmpty() ? compiledMember(package, compiled)
                                      : sourceMember(package, href);
  result.bytes  = package.read(result.member);
  if (result.bytes.startsWith(QByteArray::fromHex("ffd8"))) {
    result.encoding = BitmapPayload::Encoding::Jpeg;
    jpegHeader(package, result, maxPixels);
  } else if (result.bytes.startsWith(QByteArray::fromHex("89504e470d0a1a0a"))) {
    result.encoding = BitmapPayload::Encoding::Png;
    pngHeader(result, maxPixels);
  } else if (result.bytes.startsWith(QByteArray::fromHex("0305"))) {
    losslessDat(package, result, maxPixels);
  } else {
    throw Error("Unsupported embedded bitmap payload: " + result.member);
  }
  return result;
}

SoundFormat soundFormat(const QString &format) {
  const auto unsupported = [&] {
    return SoundMediaError(SoundMediaError::Kind::Unsupported,
                           "Unsupported sound format: \"" + format + "\"");
  };
  const QStringList parts = format.simplified().toLower().split(' ');
  if (parts.size() != 3) throw unsupported();
  SoundFormat result;
  // Flash's rate names; its 5.5 kHz rate is 5512 Hz in published movies.
  static const QMap<QString, int> rates{
      {"5khz", 5512}, {"11khz", 11025}, {"22khz", 22050}, {"44khz", 44100}};
  result.sampleRate = rates.value(parts[0], 0);
  result.bits       = parts[1] == "8bit" ? 8 : parts[1] == "16bit" ? 16 : 0;
  result.channels   = parts[2] == "mono" ? 1 : parts[2] == "stereo" ? 2 : 0;
  if (!result.sampleRate || !result.bits || !result.channels)
    throw unsupported();
  return result;
}

SoundPayload readEmbeddedSound(Package &package,
                               const QMap<QString, QString> &attributes) {
  const QString name      = attributes.value("name");
  const QString reference = attributes.value("soundDataHRef");
  if (reference.isEmpty())
    throw SoundMediaError(
        SoundMediaError::Kind::Missing,
        "Sound has no embedded sample data, only an external source: " +
            attributes.value("sourceExternalFilepath",
                             attributes.value("href", name)));
  SoundPayload result;
  result.member = package.contains(reference)            ? reference
                  : package.contains("bin/" + reference) ? "bin/" + reference
                                                         : QString();
  if (result.member.isEmpty())
    throw SoundMediaError(SoundMediaError::Kind::Missing,
                          "Missing embedded sound data: " + reference);
  result.format           = soundFormat(attributes.value("format"));
  bool ok                 = false;
  result.sampleCount      = attributes.value("sampleCount").toLongLong(&ok);
  const qint64 frameBytes = result.format.frameBytes();
  if (!ok || result.sampleCount <= 0 ||
      result.sampleCount > std::numeric_limits<int>::max() / frameBytes)
    throw SoundMediaError(SoundMediaError::Kind::Unsupported,
                          "Invalid sound sample count: \"" +
                              attributes.value("sampleCount") + "\"");
  const QByteArray data = package.read(result.member);
  const qint64 expected = result.sampleCount * frameBytes;
  // dataLength, when written, is the size of the stored sound; an MP3 is
  // followed by a waveform preview.
  bool lengthKnown = false;
  const qint64 dataLength =
      attributes.value("dataLength").toLongLong(&lengthKnown);
  lengthKnown &= dataLength > 0 && dataLength <= data.size();
  if (data.size() == expected || (lengthKnown && dataLength == expected)) {
    result.pcm = data.left(int(expected));
    return result;
  }
  const QByteArray stored = lengthKnown ? data.left(int(dataLength)) : data;
  // MP3 (an ID3 tag or a frame sync first).
  const bool mp3 = stored.startsWith("ID3") ||
                   (stored.size() >= 2 && byte(stored, 0) == 0xff &&
                    (byte(stored, 1) & 0xe0) == 0xe0);
  QString problem;
  if (mp3) {
    Mp3Stream stream;
    problem = mp3Stream(stored, stream, [&] { package.checkCancelled(); });
    if (problem.isEmpty()) {
      result.encoding = SoundPayload::Encoding::Mp3;
      result.format   = {stream.sampleRate, 16, stream.channels};
      result.sampleCount =
          (stream.frames + stream.tagFrames) * stream.samplesPerFrame;
      if (result.sampleCount >
          std::numeric_limits<int>::max() / result.format.frameBytes())
        throw SoundMediaError(SoundMediaError::Kind::Unsupported,
                              "MP3 sound is too long: " + result.member);
      result.mp3Frames =
          stored.mid(stream.audioBegin, stream.audioEnd - stream.audioBegin);
      result.mp3LeadingSilence = stream.tagFrames * stream.samplesPerFrame;
      return result;
    }
  } else if (!stored.isEmpty() && result.sampleCount >= 64) {
    // SWF ADPCM: its size follows from the sample count exactly (too few
    // samples make that no evidence).
    const int bits = 2 + (byte(stored, 0) >> 6);
    if (adpcmSize(bits, result.format.channels, result.sampleCount) ==
        stored.size()) {
      result.encoding = SoundPayload::Encoding::Adpcm;
      result.pcm =
          decodeAdpcm(stored, result.format.channels, result.sampleCount);
      result.format.bits = 16;
      return result;
    }
  }
  throw SoundMediaError(
      SoundMediaError::Kind::Unsupported,
      mp3 ? QString("Embedded sound data is MP3-compressed but cannot be "
                    "read: %1")
                .arg(problem)
          : QString("Embedded sound data is not raw PCM, MP3 or SWF ADPCM "
                    "(%1 bytes; raw %2 PCM needs %3)")
                .arg(stored.size())
                .arg(attributes.value("format"))
                .arg(expected));
}

qint64 adpcmSize(int bits, int channels, qint64 samples) {
  if (bits < 2 || bits > 5 || channels < 1 || channels > 2 || samples <= 0)
    return -1;
  const qint64 packets = samples / 4096, rest = samples % 4096;
  qint64 total = 2 + packets * channels * (22 + 4095 * qint64(bits));
  if (rest) total += channels * (22 + (rest - 1) * qint64(bits));
  return (total + 7) / 8;
}

QByteArray decodeAdpcm(const QByteArray &data, int channels, qint64 samples) {
  if (channels < 1 || channels > 2 || samples <= 0 ||
      samples > std::numeric_limits<int>::max() / (2 * channels))
    throw Error("Invalid ADPCM sound layout.");
  if (data.isEmpty()) throw Error("ADPCM sound data is empty.");
  const int bits = 2 + (byte(data, 0) >> 6);
  if (adpcmSize(bits, channels, samples) > data.size())
    throw Error("ADPCM sound data is shorter than its sample count.");
  qint64 position = 2;  // in bits, after the code size
  const auto read = [&](int count) {
    quint32 value = 0;
    for (int bit = 0; bit < count; ++bit, ++position)
      value = (value << 1) |
              ((byte(data, int(position >> 3)) >> (7 - (position & 7))) & 1);
    return value;
  };
  static const int steps[89] = {
      7,     8,     9,     10,    11,    12,    13,    14,    16,    17,
      19,    21,    23,    25,    28,    31,    34,    37,    41,    45,
      50,    55,    60,    66,    73,    80,    88,    97,    107,   118,
      130,   143,   157,   173,   190,   209,   230,   253,   279,   307,
      337,   371,   408,   449,   494,   544,   598,   658,   724,   796,
      876,   963,   1060,  1166,  1282,  1411,  1552,  1707,  1878,  2066,
      2272,  2499,  2749,  3024,  3327,  3660,  4026,  4428,  4871,  5358,
      5894,  6484,  7132,  7845,  8630,  9493,  10442, 11487, 12635, 13899,
      15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767};
  static const int indices[4][16] = {
      {-1, 2},
      {-1, -1, 2, 4},
      {-1, -1, -1, -1, 2, 4, 6, 8},
      {-1, -1, -1, -1, -1, -1, -1, -1, 1, 2, 4, 6, 8, 10, 13, 16}};
  const int *table   = indices[bits - 2];
  const int signMask = 1 << (bits - 1);
  QByteArray pcm(int(samples * channels * 2), '\0');
  char *out      = pcm.data();
  const auto put = [&out](int value) {
    *out++ = char(value & 0xff);
    *out++ = char((value >> 8) & 0xff);
  };
  int predictor[2] = {0, 0}, index[2] = {0, 0};
  for (qint64 done = 0; done < samples;) {
    for (int channel = 0; channel < channels; ++channel) {
      const quint32 initial = read(16);
      predictor[channel]    = int(qint16(quint16(initial)));
      index[channel]        = int(read(6));
      if (index[channel] > 88) index[channel] = 88;
      put(predictor[channel]);
    }
    ++done;
    for (int count = 0; count < 4095 && done < samples; ++count, ++done)
      for (int channel = 0; channel < channels; ++channel) {
        // The reference decoder: (code + 0.5) * step / 2^(bits - 2), summed
        // from halved steps, then the sign bit.
        const int code = int(read(bits));
        int step       = steps[index[channel]];
        int difference = 0;
        for (int mask = signMask >> 1; mask; mask >>= 1, step >>= 1)
          if (code & mask) difference += step;
        difference += step;
        predictor[channel] += code & signMask ? -difference : difference;
        predictor[channel] =
            std::max(-32768, std::min(32767, predictor[channel]));
        index[channel] = std::max(
            0, std::min(88, index[channel] + table[code & (signMask - 1)]));
        put(predictor[channel]);
      }
  }
  return pcm;
}

void decodeMp3Sound(SoundPayload &payload, const QString &ffmpeg,
                    const std::function<void()> &check) {
  if (payload.encoding != SoundPayload::Encoding::Mp3 ||
      payload.mp3Frames.isEmpty() || payload.format.channels < 1 ||
      payload.format.channels > 2 || payload.format.sampleRate <= 0)
    throw Error("Sound is not undecoded MP3 data: " + payload.member);
  if (ffmpeg.isEmpty() || !QFileInfo(ffmpeg).isFile())
    throw Error("FFmpeg was not found: " + ffmpeg);
  const int frameBytes = 2 * payload.format.channels;
  QTemporaryFile input(
      QDir(QDir::tempPath()).filePath("flare-flash-sound-XXXXXX.mp3"));
  if (!input.open() ||
      input.write(payload.mp3Frames) != payload.mp3Frames.size() ||
      !input.flush())
    throw Error("Cannot write a temporary MP3 stream: " + input.errorString());
  input.close();
  QTemporaryFile output(
      QDir(QDir::tempPath()).filePath("flare-flash-sound-XXXXXX.raw"));
  if (!output.open())
    throw Error("Cannot create a temporary sound file: " +
                output.errorString());
  output.close();

  QProcess process;
  process.setProcessChannelMode(QProcess::MergedChannels);
  // Every frame decodes: without its tag frame (left out) FFmpeg has no
  // gapless information to trim with.
  process.start(ffmpeg, {"-hide_banner",
                         "-nostdin",
                         "-v",
                         "error",
                         "-y",
                         "-f",
                         "mp3",
                         "-i",
                         QDir::toNativeSeparators(input.fileName()),
                         "-map",
                         "0:a:0",
                         "-vn",
                         "-sn",
                         "-dn",
                         "-c:a",
                         "pcm_s16le",
                         "-ac",
                         QString::number(payload.format.channels),
                         "-ar",
                         QString::number(payload.format.sampleRate),
                         "-f",
                         "s16le",
                         QDir::toNativeSeparators(output.fileName())});
  if (!process.waitForStarted(30000))
    throw Error("FFmpeg could not be started: " + process.errorString());
  QByteArray log;
  QElapsedTimer clock;
  clock.start();
  try {
    while (!process.waitForFinished(50)) {
      log += process.readAll();
      if (log.size() > 8192) log = log.right(4096);
      if (check) check();
      if (clock.hasExpired(600000))
        throw Error("FFmpeg did not finish decoding the sound " +
                    payload.member);
    }
  } catch (...) {
    process.kill();
    process.waitForFinished(10000);
    throw;
  }
  log += process.readAll();
  if (process.exitStatus() != QProcess::NormalExit || process.exitCode() != 0) {
    const QString text = QString::fromLocal8Bit(log).trimmed();
    throw Error(
        "FFmpeg could not decode the MP3 sound " + payload.member + ": " +
        (text.isEmpty() ? QString("exit code %1").arg(process.exitCode())
                        : text.section('\n', -3).trimmed()));
  }
  QFile decodedFile(output.fileName());
  const qint64 wanted = payload.sampleCount - payload.mp3LeadingSilence;
  if (!decodedFile.open(QIODevice::ReadOnly))
    throw Error("FFmpeg wrote no samples for the MP3 sound " + payload.member);
  // Bounded: four times the expected samples is far more than any decoder
  // adds.
  const QByteArray decoded = decodedFile.read(4 * wanted * frameBytes + 16);
  decodedFile.close();
  const qint64 count = decoded.size() / frameBytes;
  if (count <= 0)
    throw Error("FFmpeg decoded no samples of the MP3 sound " + payload.member);
  QByteArray pcm(int(payload.mp3LeadingSilence * frameBytes), '\0');
  pcm += decoded.left(int(std::min(count, wanted) * frameBytes));
  if (count < wanted)
    pcm += QByteArray(int((wanted - count) * frameBytes), '\0');
  payload.decodeNote =
      count == wanted
          ? QString()
          : QString(
                "FFmpeg decoded %1 of the %2 samples of its MP3 frames; "
                "the sound was %3 to that length")
                .arg(count)
                .arg(wanted)
                .arg(count < wanted ? "padded with silence" : "cut");
  payload.pcm = pcm;
}

QByteArray envelopedPcm(const SoundFormat &format, const QByteArray &pcm,
                        const QVector<SoundLevelPoint> &envelope,
                        SoundFormat &result) {
  if ((format.bits != 8 && format.bits != 16) || format.channels < 1 ||
      format.channels > 2)
    throw Error("Invalid sound layout for an envelope.");
  bool stereo = format.channels == 2;
  for (const auto &point : envelope) stereo |= point.left != point.right;
  result               = {format.sampleRate, 16, stereo ? 2 : 1};
  const qint64 samples = pcm.size() / format.frameBytes();
  if (samples > std::numeric_limits<int>::max() / result.frameBytes())
    throw Error("Sound is too long for its envelope.");
  QByteArray out(int(samples * result.frameBytes()), '\0');
  const uchar *in   = reinterpret_cast<const uchar *>(pcm.constData());
  char *to          = out.data();
  const auto sample = [&](qint64 at, int channel) {
    const qint64 index = at * format.channels + channel;
    return format.bits == 8
               ? (int(in[index]) - 128) * 256
               : int(qint16(quint16(in[2 * index] | (in[2 * index + 1] << 8))));
  };
  const auto put = [&to](double value) {
    const int level =
        int(std::lround(std::max(-32768.0, std::min(32767.0, value))));
    *to++ = char(level & 0xff);
    *to++ = char((level >> 8) & 0xff);
  };
  // Ruffle's EnvelopeSignal: the first point's levels from sample 0.
  const SoundLevelPoint first =
      envelope.isEmpty() ? SoundLevelPoint{} : envelope.front();
  SoundLevelPoint previous{0, first.left, first.right}, next = first;
  int following      = 1;
  const auto advance = [&] {
    previous    = next;
    next        = following < envelope.size()
                      ? envelope[following++]
                      : SoundLevelPoint{std::numeric_limits<qint64>::max(),
                                 previous.left, previous.right};
    next.sample = std::max(next.sample, previous.sample);
  };
  for (qint64 at = 0; at < samples; ++at) {
    double left = next.left, right = next.right;
    if (previous.sample < next.sample) {
      const double t =
          double(at - previous.sample) / double(next.sample - previous.sample);
      left  = previous.left + (next.left - previous.left) * t;
      right = previous.right + (next.right - previous.right) * t;
    }
    const int l = sample(at, 0);
    const int r = format.channels == 2 ? sample(at, 1) : l;
    if (stereo) {
      put(l * left);
      put(r * right);
    } else
      put(l * left);
    while (at + 1 > next.sample) advance();
  }
  return out;
}

namespace {
void little(QByteArray &bytes, quint32 value, int size) {
  for (int at = 0; at < size; ++at) bytes.append(char(value >> (8 * at)));
}

quint32 little32(const QByteArray &bytes, int at) {
  return quint32(little16(bytes, at)) |
         (quint32(little16(bytes, at + 2)) << 16);
}
}  // namespace

QByteArray waveFile(const SoundFormat &format, const QByteArray &pcm) {
  const quint32 padding = pcm.size() & 1;  // RIFF chunks have even sizes
  QByteArray bytes;
  bytes.reserve(44 + pcm.size() + int(padding));
  bytes += "RIFF";
  little(bytes, 36 + quint32(pcm.size()) + padding, 4);
  bytes += "WAVEfmt ";
  little(bytes, 16, 4);
  little(bytes, 1, 2);  // WAVE_FORMAT_PCM: 8-bit unsigned, 16-bit signed
  little(bytes, quint32(format.channels), 2);
  little(bytes, quint32(format.sampleRate), 4);
  little(bytes, quint32(format.sampleRate * format.frameBytes()), 4);
  little(bytes, quint32(format.frameBytes()), 2);
  little(bytes, quint32(format.bits), 2);
  bytes += "data";
  little(bytes, quint32(pcm.size()), 4);
  bytes += pcm;
  if (padding) bytes.append(char(0));
  return bytes;
}

bool readWaveFile(const QByteArray &bytes, SoundFormat &format,
                  QByteArray &pcm) {
  if (bytes.size() < 44 || !bytes.startsWith("RIFF") ||
      bytes.mid(8, 8) != "WAVEfmt " || little32(bytes, 16) != 16 ||
      little16(bytes, 20) != 1 || bytes.mid(36, 4) != "data")
    return false;
  SoundFormat result;
  result.channels    = little16(bytes, 22);
  result.sampleRate  = int(little32(bytes, 24));
  result.bits        = little16(bytes, 34);
  const quint32 size = little32(bytes, 40);
  if ((result.bits != 8 && result.bits != 16) ||
      (result.channels != 1 && result.channels != 2) ||
      result.sampleRate <= 0 ||
      little32(bytes, 28) != quint32(result.sampleRate * result.frameBytes()) ||
      little16(bytes, 32) != result.frameBytes() ||
      size % quint32(result.frameBytes()) ||
      quint64(bytes.size()) != 44ull + size + (size & 1) ||
      little32(bytes, 4) != quint32(bytes.size() - 8))
    return false;
  format = result;
  pcm    = bytes.mid(44, int(size));
  return true;
}

}  // namespace Flash
