#pragma once

#include "archive.h"
#include "media.h"

#include <QMap>
#include <QVector>
#include <functional>

namespace Flash {

// One compressed video frame of an embedded video, located in
// VideoPayload::data. Its bytes are an FLV video tag body without the leading
// frame-type/codec byte: for VP6, one adjustment (crop) byte and the frame.
struct VideoFrameRecord {
  quint32 offset = 0, size = 0;
  bool keyframe = false;
};

// An embedded DOMVideoItem ("vp6 media"). Animate keeps the video in its own
// container in bin/<videoDataHRef>, little-endian:
//   u8 3 (version), u32 codec (FLV video codec id: 4 VP6, 5 VP6 with alpha),
//   f64 frame rate, f64 sound rate, u32 sound bits, u32 sound channels,
//   u32 width, u32 height, f64 duration (seconds), 30 bytes of import
//   settings, then the video track: index records
//   {u8 1, u32 offset, u32 size, u32 1, u32 kind (5 key frame, 4 inter
//   frame)}, u8 0, u32 data size, frame data. A sound track of the same form
//   may follow: u32 track count (0 or 1), one record {u8 1, u32 0, u32 size,
//   u32 1, u32 0}, u8 0, u32 size, raw PCM in the header's sound layout
//   (interleaved; 16-bit signed or 8-bit unsigned, as embedded sounds), then
//   a 12-byte trailer. Animate decodes the source's audio into it: at 44.1
//   kHz 16-bit stereo it is a whole number of 1152-sample blocks, 1224-2268
//   samples longer than the duration. Its first sample plays with the first
//   video frame (published SWFs).
struct VideoPayload {
  enum class Sound { None, Silent, Audible, Unreadable };
  QString member;
  int codec        = 0;
  double frameRate = 0;
  int width = 0, height = 0;
  double duration = 0;
  QVector<VideoFrameRecord> frames;  // in playback order
  QByteArray data;                   // the video track's frame data
  Sound sound = Sound::None;
  // Silent or Audible: the sound track's layout and sample frames; its PCM
  // is kept for an audible track only. Unreadable: why.
  SoundFormat soundFormat;
  qint64 soundSamples = 0;
  QByteArray soundPcm;
  QString soundProblem;
};

// Why a video item cannot be read: the data is absent from the package, or it
// is in a form without a verified mapping (other codec, inconsistent index).
class VideoMediaError : public Error {
public:
  enum class Kind { Missing, Unsupported };
  VideoMediaError(Kind kind, const QString &message)
      : Error(message), m_kind(kind) {}
  Kind kind() const { return m_kind; }

private:
  Kind m_kind;
};

// Parses the container bytes of one embedded video (see VideoPayload). Every
// size and offset is checked; anything unexpected in the video throws
// VideoMediaError. A sound track that is not raw PCM covering the duration
// in the header's layout (compressed or inconsistent data) is Unreadable.
VideoPayload parseEmbeddedVideo(const QByteArray &bytes,
                                const QString &member = QString(),
                                int maxFrames         = 1000000);
// DOMVideoItem attributes are passed directly from the source descriptor.
// The item's size and sound layout (rate, bits, channels) must agree with
// the data.
VideoPayload readEmbeddedVideo(Package &package,
                               const QMap<QString, QString> &attributes);
// The audible sound track as an embedded sound; throws Error otherwise.
SoundPayload videoSoundTrack(const VideoPayload &video);
// A standard FLV stream of the video track (video tags only, one per frame,
// timestamps at the item's frame rate) that FFmpeg's vp6f/vp6a decoders read.
QByteArray flvStream(const VideoPayload &video);

// Decodes every frame with FFmpeg into <directory>/<baseName>.0001.png,
// .0002.png, ... (one file per video frame, at the video's pixel size).
// `progress` receives the number of frames written so far; it may throw to
// cancel, which stops FFmpeg and removes the partial output. Any failure
// removes the output and throws Error.
void decodeVideoFrames(const VideoPayload &video, const QString &ffmpeg,
                       const QString &directory, const QString &baseName,
                       const std::function<void(int)> &progress = {});

}  // namespace Flash
