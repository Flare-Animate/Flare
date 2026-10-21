#pragma once

#include "archive.h"

#include <QMap>
#include <QVector>
#include <functional>

namespace Flash {

// A validated embedded bitmap. JPEG/PNG retain encoded bytes for native
// decoding. Lossless DAT also carries straight top-down RGBA pixels; its
// original bytes remain available for provenance and diagnostics.
struct BitmapPayload {
  enum class Encoding { Jpeg, Png, LosslessDat };
  Encoding encoding = Encoding::Jpeg;
  QString member;
  QByteArray bytes;
  QByteArray straightRgba;
  int width  = 0;
  int height = 0;
};

// DOMBitmapItem attributes are passed directly from the source descriptor.
// Prefer authored compiled data. An opaque compiled DAT must never be silently
// replaced by an external/source-link image that may differ from its artwork.
BitmapPayload readEncodedBitmap(Package &package,
                                const QMap<QString, QString> &attributes,
                                quint64 maxPixels = 64ull * 1024 * 1024);

// Sample layout of decoded sound: interleaved channels, 16-bit signed
// little-endian or 8-bit unsigned samples (the SWF uncompressed convention).
struct SoundFormat {
  int sampleRate = 0;  // Hz: 5512, 11025, 22050 or 44100 for embedded PCM
  int bits       = 0;  // 8 or 16
  int channels   = 0;  // 1 or 2
  int frameBytes() const { return bits / 8 * channels; }
};

// The samples of an embedded DOMSoundItem. Animate keeps them in
// bin/<soundDataHRef>, in one of these forms:
//  - Pcm: headerless PCM of the item's format ("44kHz 16bit Mono") and
//    sampleCount, the form of imported WAV and AIFF files (dataLength, when
//    written, is its size).
//  - Mp3: an imported MP3 file as it was (dataLength bytes: ID3 tags and
//    MPEG audio layer III frames), then one byte 01 and a waveform preview
//    of cachedSampleCount samples in cacheFormat ("5kHz 8bit Stereo"). The
//    item's sampleCount is every frame's samples, a leading Xing/Info/VBRI
//    tag frame included: Flash plays all of them, without gapless trimming.
//    Decoding needs FFmpeg (decodeMp3Sound()); `pcm` stays empty until then.
//  - Adpcm: SWF ADPCM (2-5 bits per sample, 4096-sample packets), as a
//    published SWF's DefineSound holds it. Decoded here.
struct SoundPayload {
  enum class Encoding { Pcm, Mp3, Adpcm };
  Encoding encoding = Encoding::Pcm;
  SoundFormat format;  // The decoded samples' layout
  QString member;
  qint64 sampleCount = 0;  // Sample frames: one sample per channel
  QByteArray pcm;          // Exactly sampleCount * format.frameBytes() bytes
  // Mp3: the audio frames FFmpeg decodes (tags left out), and the silent
  // sample frames of a leading tag frame, which FFmpeg skips.
  QByteArray mp3Frames;
  qint64 mp3LeadingSilence = 0;
  // Set by a decoder whose output had to be padded or cut to sampleCount.
  QString decodeNote;
  bool decoded() const { return !pcm.isEmpty(); }
};

// Why a sound item has no decodable samples: the data is absent from the
// package (only an external source path), or it is stored in a form without
// a verified decoder (unknown format string, compressed or inconsistent data).
class SoundMediaError : public Error {
public:
  enum class Kind { Missing, Unsupported };
  SoundMediaError(Kind kind, const QString &message)
      : Error(message), m_kind(kind) {}
  Kind kind() const { return m_kind; }

private:
  Kind m_kind;
};

// Parses a DOMSoundItem format attribute such as "44kHz 16bit Mono". Throws
// SoundMediaError (Unsupported) for anything else.
SoundFormat soundFormat(const QString &format);
// DOMSoundItem attributes are passed directly from the source descriptor.
// Raw PCM whose size matches format and sampleCount, MP3 frames (left
// undecoded) and SWF ADPCM whose size matches sampleCount (decoded) are
// accepted; anything else throws SoundMediaError.
SoundPayload readEmbeddedSound(Package &package,
                               const QMap<QString, QString> &attributes);

// SWF ADPCM ------------------------------------------------------------------
// ADPCMSOUNDDATA: a 2-bit code size (bits per sample - 2), then packets of
// 4096 sample frames (the last may be shorter), each a 16-bit initial sample
// and 6-bit step index per channel, then 4095 codes per channel
// (interleaved), all bits most significant first, without byte alignment.
// The size in bytes of `samples` sample frames.
qint64 adpcmSize(int bits, int channels, qint64 samples);
// Decodes exactly `samples` sample frames to 16-bit PCM with the Macromedia
// reference algorithm (an IMA ADPCM variant). Throws Error if the data is
// shorter than that.
QByteArray decodeAdpcm(const QByteArray &data, int channels, qint64 samples);

// MP3 ------------------------------------------------------------------------
// Decodes payload.mp3Frames with FFmpeg into payload.pcm (16-bit), the tag
// frame's silence first, so that it holds payload.sampleCount samples (a
// decoder that returns more or fewer is padded or cut, noted in
// payload.decodeNote). `check` is called while FFmpeg runs and may throw to
// cancel it. Throws Error when FFmpeg is missing or fails.
void decodeMp3Sound(SoundPayload &payload, const QString &ffmpeg,
                    const std::function<void()> &check = {});

// Volume envelopes
// ------------------------------------------------------------- One point of a
// sound's volume envelope at a sample of its own rate, counted from its first
// played sample (its in point); 1 is full volume.
struct SoundLevelPoint {
  qint64 sample = 0;
  double left = 1, right = 1;
};
// Multiplies sound samples by an envelope as the Flash player does (Ruffle's
// mixer): before the first point its levels, between points linear, after the
// last one its levels; the envelope runs on through loops. The result is
// 16-bit, stereo unless the sound is mono and every point has equal levels.
QByteArray envelopedPcm(const SoundFormat &format, const QByteArray &pcm,
                        const QVector<SoundLevelPoint> &envelope,
                        SoundFormat &result);
// A canonical RIFF/WAVE PCM file (44-byte header) for that sample layout.
QByteArray waveFile(const SoundFormat &format, const QByteArray &pcm);
// Reads back the canonical layout written by waveFile; false otherwise.
bool readWaveFile(const QByteArray &bytes, SoundFormat &format,
                  QByteArray &pcm);

}  // namespace Flash
