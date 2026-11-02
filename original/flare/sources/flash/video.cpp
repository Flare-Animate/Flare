#include "video.h"

#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QProcess>
#include <QRegularExpression>
#include <QTemporaryFile>
#include <QtEndian>
#include <cmath>
#include <cstring>

namespace Flash {
namespace {

constexpr int indexStart  = 0x4b;  // First video index record
constexpr int recordBytes = 17;
// An FLV tag body holds at most 2^24 - 1 bytes, one of them the codec byte.
constexpr quint32 maxFrameBytes = 0xffffff - 1;

quint8 byte(const QByteArray &bytes, qint64 at) {
  return quint8(bytes.at(int(at)));
}

quint32 little32(const QByteArray &bytes, qint64 at) {
  return qFromLittleEndian<quint32>(bytes.constData() + at);
}

double little64f(const QByteArray &bytes, qint64 at) {
  const quint64 bits = qFromLittleEndian<quint64>(bytes.constData() + at);
  double value;
  std::memcpy(&value, &bits, sizeof value);
  return value;
}

void big(QByteArray &bytes, quint32 value, int size) {
  for (int at = size - 1; at >= 0; --at) bytes.append(char(value >> (8 * at)));
}

struct Track {
  QVector<VideoFrameRecord> records;
  QVector<quint32> kinds;
  qint64 dataStart = 0;
  quint32 dataSize = 0;
};

// Index records {u8 1, u32 offset, u32 size, u32 1, u32 kind}, u8 0, then
// u32 data size and the data. Returns false on any inconsistency.
bool readTrack(const QByteArray &bytes, qint64 pos, int maxRecords,
               Track &track, QString &problem) {
  const qint64 size = bytes.size();
  while (pos < size && byte(bytes, pos) == 1) {
    if (track.records.size() >= maxRecords) {
      problem = "frame count exceeds the import limit";
      return false;
    }
    if (pos > size - recordBytes) {
      problem = "truncated frame index";
      return false;
    }
    VideoFrameRecord record;
    record.offset = little32(bytes, pos + 1);
    record.size   = little32(bytes, pos + 5);
    if (little32(bytes, pos + 9) != 1) {
      problem = "unknown frame record layout";
      return false;
    }
    track.kinds.push_back(little32(bytes, pos + 13));
    track.records.push_back(record);
    pos += recordBytes;
  }
  if (pos > size - 5 || byte(bytes, pos) != 0) {
    problem = "missing index terminator";
    return false;
  }
  track.dataSize  = little32(bytes, pos + 1);
  track.dataStart = pos + 5;
  if (track.dataSize > size - track.dataStart) {
    problem = "truncated frame data";
    return false;
  }
  for (const auto &record : track.records)
    if (record.offset > track.dataSize ||
        record.size > track.dataSize - record.offset) {
      problem = "frame outside its data";
      return false;
    }
  return true;
}

// The displayed size a VP6 key frame declares: stored macroblock columns and
// rows less the FLV adjustment (crop) byte, as FFmpeg's decoder applies it.
bool vp6KeyFrameSize(const char *frame, quint32 size, int &width, int &height) {
  // frame[0] is the adjustment byte, then the VP6 frame header.
  if (size < 7) return false;
  const quint8 adjust = quint8(frame[0]), first = quint8(frame[1]),
               second = quint8(frame[2]);
  if (first & 0x80) return false;  // not an intra frame
  int at = 3;
  // Separated coefficients, or no filter header: a 16-bit offset follows.
  if ((first & 1) || !(second & 0x06)) at += 2;
  if (quint32(at) + 2 > size) return false;
  const int rows = quint8(frame[at]), columns = quint8(frame[at + 1]);
  width  = columns * 16 - (adjust >> 4);
  height = rows * 16 - (adjust & 0x0f);
  return rows > 0 && columns > 0;
}

}  // namespace

VideoPayload parseEmbeddedVideo(const QByteArray &bytes, const QString &member,
                                int maxFrames) {
  const auto unsupported = [&](const QString &detail) {
    return VideoMediaError(
        VideoMediaError::Kind::Unsupported,
        "Unsupported embedded video data (" + detail + "): " + member);
  };
  if (bytes.size() < indexStart + 5) throw unsupported("truncated header");
  if (byte(bytes, 0) != 3)
    throw unsupported("container version " + QString::number(byte(bytes, 0)));
  VideoPayload result;
  result.member       = member;
  const quint32 codec = little32(bytes, 1);
  // FLV video codec ids: 4 is VP6, 5 is VP6 with an alpha channel.
  if (codec != 4 && codec != 5)
    throw unsupported("video codec " + QString::number(codec));
  result.codec                = int(codec);
  result.frameRate            = little64f(bytes, 5);
  const double soundRate      = little64f(bytes, 0x0d);
  const quint32 soundBits     = little32(bytes, 0x15);
  const quint32 soundChannels = little32(bytes, 0x19);
  const quint32 width = little32(bytes, 0x1d), height = little32(bytes, 0x21);
  result.duration = little64f(bytes, 0x25);
  if (!std::isfinite(result.frameRate) || result.frameRate < 0.01 ||
      result.frameRate > 1000)
    throw unsupported("frame rate");
  if (width < 1 || height < 1 || width > 16384 || height > 16384)
    throw unsupported("frame size");
  if (!std::isfinite(result.duration) || result.duration < 0)
    throw unsupported("duration");
  result.width  = int(width);
  result.height = int(height);

  Track video;
  QString problem;
  if (!readTrack(bytes, indexStart, std::max(0, maxFrames), video, problem))
    throw unsupported(problem);
  if (video.records.isEmpty()) throw unsupported("no video frames");
  for (int index = 0; index < video.records.size(); ++index) {
    auto &record       = video.records[index];
    const quint32 kind = video.kinds[index];
    if (kind != 4 && kind != 5) throw unsupported("unknown frame kind");
    record.keyframe = kind == 5;
    if (record.size < 2 || record.size > maxFrameBytes)
      throw unsupported("frame size " + QString::number(record.size));
    // The last timestamp must fit FLV's signed 32-bit milliseconds.
    if (index * 1000.0 / result.frameRate > 2147483647.0)
      throw unsupported("duration exceeds the FLV time range");
    if (codec != 4) continue;
    // VP6 frames are self-describing: check the stored frame type and, for
    // key frames, the frame size against the index and the header.
    const char *frame = bytes.constData() + video.dataStart + record.offset;
    const bool intra  = !(quint8(frame[1]) & 0x80);
    if (intra != record.keyframe)
      throw unsupported("frame type differs from its index");
    int frameWidth = 0, frameHeight = 0;
    if (record.keyframe &&
        (!vp6KeyFrameSize(frame, record.size, frameWidth, frameHeight) ||
         frameWidth != result.width || frameHeight != result.height))
      throw unsupported("key frame size differs from the video size");
  }
  if (!video.records.front().keyframe)
    throw unsupported("the first frame is not a key frame");
  result.frames = video.records;
  result.data   = bytes.mid(int(video.dataStart), int(video.dataSize));

  // Optional sound track: one record locating raw PCM in the header's sound
  // layout, then the trailer.
  const qint64 soundAt = video.dataStart + qint64(video.dataSize);
  if (soundAt == bytes.size()) return result;
  const auto unreadable = [&](const QString &detail) {
    result.sound        = VideoPayload::Sound::Unreadable;
    result.soundProblem = detail;
    return result;
  };
  if (soundAt > bytes.size() - 4) return unreadable("truncated sound track");
  const quint32 tracks = little32(bytes, soundAt);
  if (tracks == 0) return result;
  if (tracks != 1) return unreadable(QString("%1 sound tracks").arg(tracks));
  Track sound;
  if (!readTrack(bytes, soundAt + 4, 16, sound, problem))
    return unreadable("sound track: " + problem);
  if (sound.records.size() != 1 || sound.kinds.front() != 0 ||
      sound.records.front().offset != 0 ||
      sound.records.front().size != sound.dataSize)
    return unreadable("unknown sound track index");
  if (bytes.size() - (sound.dataStart + qint64(sound.dataSize)) != 12)
    return unreadable("unknown sound track trailer");
  // Sound rates are whole hertz (Flash's "5 kHz" is 5512 Hz).
  if (!std::isfinite(soundRate) || soundRate < 1000 || soundRate > 192000 ||
      soundRate != std::floor(soundRate) ||
      (soundBits != 8 && soundBits != 16) ||
      (soundChannels != 1 && soundChannels != 2))
    return unreadable(QString("sound layout %1 Hz, %2 bits, %3 channels")
                          .arg(soundRate)
                          .arg(soundBits)
                          .arg(soundChannels));
  SoundFormat format;
  format.sampleRate       = int(soundRate);
  format.bits             = int(soundBits);
  format.channels         = int(soundChannels);
  const qint64 frameBytes = format.frameBytes();
  if (sound.dataSize % frameBytes)
    return unreadable("partial sound sample frame");
  // Raw PCM covers the video, give or take a video frame (and half a sample
  // for the stored duration's rounding), plus the decoder's padding (up to
  // about 2268 samples, 51 ms); compressed audio is far smaller.
  const qint64 samples  = qint64(sound.dataSize) / frameBytes;
  const double expected = result.duration * soundRate;
  if (double(samples) + 0.5 < expected - soundRate / result.frameRate ||
      double(samples) - 0.5 > expected + soundRate / 4)
    return unreadable(QString("%1 sound samples for %2 s at %3 Hz: "
                              "compressed or inconsistent data")
                          .arg(samples)
                          .arg(result.duration)
                          .arg(soundRate));
  result.sound        = VideoPayload::Sound::Silent;
  result.soundFormat  = format;
  result.soundSamples = samples;
  // 16-bit samples are signed (silence 0); 8-bit ones unsigned (silence 128).
  const char *pcm  = bytes.constData() + sound.dataStart;
  const char quiet = soundBits == 8 ? char(0x80) : char(0);
  for (quint32 at = 0; at < sound.dataSize; ++at)
    if (pcm[at] != quiet) {
      result.sound    = VideoPayload::Sound::Audible;
      result.soundPcm = bytes.mid(int(sound.dataStart), int(sound.dataSize));
      break;
    }
  return result;
}

VideoPayload readEmbeddedVideo(Package &package,
                               const QMap<QString, QString> &attributes) {
  const QString name      = attributes.value("name");
  const QString reference = attributes.value("videoDataHRef");
  if (reference.isEmpty())
    throw VideoMediaError(
        VideoMediaError::Kind::Missing,
        "Video has no embedded data, only an external source: " +
            attributes.value("sourceExternalFilepath", name));
  const QString member = package.contains(reference) ? reference
                         : package.contains("bin/" + reference)
                             ? "bin/" + reference
                             : QString();
  if (member.isEmpty())
    throw VideoMediaError(VideoMediaError::Kind::Missing,
                          "Missing embedded video data: " + reference);
  VideoPayload result = parseEmbeddedVideo(package.read(member), member);
  // The library item repeats the stored size; a disagreement means the data
  // is not what the item describes.
  bool ok          = false;
  const int width  = attributes.value("width").toInt(&ok);
  const bool known = ok;
  const int height = attributes.value("height").toInt(&ok);
  if (known && ok && (width != result.width || height != result.height))
    throw VideoMediaError(VideoMediaError::Kind::Unsupported,
                          QString("Embedded video data is %1x%2 pixels; its "
                                  "library item says %3x%4: %5")
                              .arg(result.width)
                              .arg(result.height)
                              .arg(width)
                              .arg(height)
                              .arg(member));
  // It repeats the sound layout too; a sound track that disagrees is not
  // what the item describes.
  if (result.sound == VideoPayload::Sound::Silent ||
      result.sound == VideoPayload::Sound::Audible) {
    const SoundFormat &format = result.soundFormat;
    for (const auto &field :
         {std::make_pair(QString("rate"), format.sampleRate),
          std::make_pair(QString("bits"), format.bits),
          std::make_pair(QString("channels"), format.channels)}) {
      const int value = attributes.value(field.first).toInt(&ok);
      if (!ok || value == field.second) continue;
      result.sound        = VideoPayload::Sound::Unreadable;
      result.soundProblem = QString(
                                "the sound track has %1 %2; its library "
                                "item says %3")
                                .arg(field.second)
                                .arg(field.first)
                                .arg(value);
      result.soundPcm.clear();
      break;
    }
  }
  return result;
}

SoundPayload videoSoundTrack(const VideoPayload &video) {
  const qint64 frameBytes = video.soundFormat.frameBytes();
  if (video.sound != VideoPayload::Sound::Audible || frameBytes <= 0 ||
      video.soundPcm.size() != video.soundSamples * frameBytes)
    throw Error("Embedded video has no audible sound track: " + video.member);
  SoundPayload result;
  result.format      = video.soundFormat;
  result.member      = video.member;
  result.sampleCount = video.soundSamples;
  result.pcm         = video.soundPcm;
  return result;
}

QByteArray flvStream(const VideoPayload &video) {
  if (video.frames.isEmpty() || video.frameRate <= 0)
    throw Error("Embedded video has no frames: " + video.member);
  QByteArray result;
  result.reserve(13 + video.frames.size() * 16 + video.data.size());
  result += "FLV";
  result += char(1);  // version
  result += char(1);  // video tags only
  big(result, 9, 4);  // header size
  big(result, 0, 4);  // PreviousTagSize0
  for (int index = 0; index < video.frames.size(); ++index) {
    const auto &frame = video.frames[index];
    if (frame.offset > quint32(video.data.size()) ||
        frame.size > quint32(video.data.size()) - frame.offset ||
        frame.size > maxFrameBytes)
      throw Error("Embedded video frame is outside its data: " + video.member);
    const quint32 body = frame.size + 1;
    const quint32 time =
        quint32(std::llround(index * 1000.0 / video.frameRate));
    result += char(9);  // video tag
    big(result, body, 3);
    big(result, time & 0xffffff, 3);
    result += char(time >> 24);
    big(result, 0, 3);  // stream id
    // Frame type (1 key frame, 2 inter frame) and codec id.
    result += char(((frame.keyframe ? 1 : 2) << 4) | video.codec);
    result.append(video.data.constData() + frame.offset, int(frame.size));
    big(result, 11 + body, 4);
  }
  return result;
}

void decodeVideoFrames(const VideoPayload &video, const QString &ffmpeg,
                       const QString &directory, const QString &baseName,
                       const std::function<void(int)> &progress) {
  const int count = video.frames.size();
  const QDir folder(directory);
  if (ffmpeg.isEmpty() || !QFileInfo(ffmpeg).isFile())
    throw Error("FFmpeg was not found: " + ffmpeg);
  if (directory.isEmpty() || !folder.exists())
    throw Error("Video frame folder is missing: " + directory);
  // The name becomes an FFmpeg image pattern: keep it free of '%'.
  static const QRegularExpression safeName("^[A-Za-z0-9_-]+$");
  if (!safeName.match(baseName).hasMatch() || count < 1)
    throw Error("Invalid video frame file name: " + baseName);
  const auto frameFile = [&](int frame) {
    return folder.filePath(
        QString("%1.%2.png").arg(baseName).arg(frame, 4, 10, QChar('0')));
  };
  for (const int frame : {1, count, count + 1})
    if (QFileInfo::exists(frameFile(frame)) ||
        QFileInfo(frameFile(frame)).isSymLink())
      throw Error("Video frame file already exists: " + frameFile(frame));

  QTemporaryFile input(
      QDir(QDir::tempPath()).filePath("flare-flash-video-XXXXXX.flv"));
  {
    const QByteArray stream = flvStream(video);
    if (!input.open() || input.write(stream) != stream.size() || !input.flush())
      throw Error("Cannot write a temporary video stream: " +
                  input.errorString());
    input.close();
  }
  // Remove what FFmpeg wrote unless every frame was verified.
  struct Output {
    const std::function<QString(int)> file;
    int count;
    bool keep = false;
    void remove() const {
      for (int frame = 1; frame <= count + 1; ++frame)
        QFile::remove(file(frame));
    }
    ~Output() {
      if (!keep) remove();
    }
  } output{frameFile, count};

  // Returns FFmpeg's error text, empty on success.
  const auto run = [&](bool legacySync) -> QString {
    QProcess process;
    process.setWorkingDirectory(directory);
    process.setProcessChannelMode(QProcess::MergedChannels);
    QStringList arguments{
        "-hide_banner", "-nostdin",
        "-v",           "error",
        "-f",           "flv",
        "-i",           QDir::toNativeSeparators(input.fileName()),
        "-map",         "0:v:0",
        "-an",          "-sn",
        "-dn"};
    // One image per stored frame, whatever the timestamps say. FFmpeg 5.1
    // replaced -vsync with -fps_mode.
    arguments << (legacySync ? QStringList{"-vsync", "0"}
                             : QStringList{"-fps_mode", "passthrough"});
    arguments << "-frames:v" << QString::number(count) << "-start_number"
              << "1"
              << "-f"
              << "image2"
              << "-c:v"
              << "png" << baseName + ".%04d.png";
    process.start(ffmpeg, arguments);
    if (!process.waitForStarted(30000))
      return "FFmpeg could not be started: " + process.errorString();
    QByteArray log;
    int written      = 0;
    const auto drain = [&] {
      log += process.readAll();
      if (log.size() > 8192) log = log.right(4096);
    };
    // FFmpeg writes several frames a second; a long silence means it hangs.
    QElapsedTimer idle;
    idle.start();
    try {
      while (!process.waitForFinished(50)) {
        drain();
        // A frame is complete once FFmpeg has started the next one.
        const int before = written;
        while (written + 1 < count && QFileInfo::exists(frameFile(written + 2)))
          ++written;
        if (written != before) idle.restart();
        if (progress) progress(written);
        if (idle.hasExpired(300000))
          throw Error("FFmpeg stopped writing video frames: " + video.member);
      }
    } catch (...) {
      process.kill();
      process.waitForFinished(10000);
      throw;
    }
    drain();
    if (process.exitStatus() == QProcess::NormalExit && process.exitCode() == 0)
      return QString();
    const QString text = QString::fromLocal8Bit(log).trimmed();
    return text.isEmpty()
               ? QString("FFmpeg exited with code %1").arg(process.exitCode())
               : text.section('\n', -3).trimmed();
  };
  QString failure = run(false);
  if (!failure.isEmpty() && failure.contains("fps_mode")) {
    output.remove();
    failure = run(true);
  }
  if (!failure.isEmpty())
    throw Error("FFmpeg could not decode the embedded video " + video.member +
                ": " + failure);
  int decoded = 0;
  while (decoded < count && QFileInfo(frameFile(decoded + 1)).isFile())
    ++decoded;
  if (decoded != count || QFileInfo::exists(frameFile(count + 1)))
    throw Error(QString("FFmpeg decoded %1 of the %2 frames of %3.")
                    .arg(decoded)
                    .arg(count)
                    .arg(video.member));
  // PNG signature and IHDR: the decoded size must be the video's size.
  QFile first(frameFile(1));
  const QByteArray header =
      first.open(QIODevice::ReadOnly) ? first.read(24) : QByteArray();
  if (header.size() != 24 ||
      !header.startsWith(QByteArray::fromHex("89504e470d0a1a0a")) ||
      header.mid(12, 4) != "IHDR" ||
      qFromBigEndian<quint32>(header.constData() + 16) !=
          quint32(video.width) ||
      qFromBigEndian<quint32>(header.constData() + 20) != quint32(video.height))
    throw Error("FFmpeg decoded frames of a different size: " + video.member);
  if (progress) progress(count);
  output.keep = true;
}

}  // namespace Flash
