#include "flashbitmap.h"

#include <QBuffer>
#include <QImageReader>
#include "trop.h"
#include <cmath>

namespace FlashNative {

TRasterImageP bitmapImage(const Flash::BitmapPayload &payload, double dpi) {
  if (!std::isfinite(dpi) || dpi <= 0)
    throw Flash::Error("Invalid native bitmap DPI.");
  if (payload.encoding == Flash::BitmapPayload::Encoding::LosslessDat) {
    const quint64 expected =
        quint64(payload.width) * quint64(payload.height) * 4;
    if (payload.width <= 0 || payload.height <= 0 ||
        expected != quint64(payload.straightRgba.size()))
      throw Flash::Error("Invalid decoded lossless bitmap size: " +
                         payload.member);
    TRaster32P raster(payload.width, payload.height);
    raster->lock();
    for (int y = 0; y < payload.height; ++y) {
      // DAT and RGBA buffers are top-down; Flare rasters are bottom-up.
      const auto *source = reinterpret_cast<const quint8 *>(
          payload.straightRgba.constData() +
          (payload.height - 1 - y) * payload.width * 4);
      auto *target = raster->pixels(y);
      for (int x = 0; x < payload.width; ++x) {
        const auto *pixel = source + x * 4;
        target[x]         = TPixel32(pixel[0], pixel[1], pixel[2], pixel[3]);
      }
    }
    raster->unlock();
    // Full-color native rasters use premultiplied channels. Native PNG I/O
    // depremultiplies on write and premultiplies on read.
    TRop::premultiply(raster);
    TRasterImageP result(raster);
    result->setDpi(dpi, dpi);
    return result;
  }
  QByteArray data = payload.bytes;
  QBuffer device(&data);
  if (!device.open(QIODevice::ReadOnly))
    throw Flash::Error("Cannot open embedded bitmap payload: " +
                       payload.member);
  QImageReader reader(&device,
                      payload.encoding == Flash::BitmapPayload::Encoding::Jpeg
                          ? "jpeg"
                          : "png");
  reader.setAutoTransform(false);
  const QSize headerSize = reader.size();
  if (headerSize != QSize(payload.width, payload.height))
    throw Flash::Error("Bitmap decoder dimensions disagree with source: " +
                       payload.member);
  const QImage decoded = reader.read();
  if (decoded.isNull() || decoded.size() != headerSize)
    throw Flash::Error("Cannot decode embedded bitmap: " + payload.member +
                       ": " + reader.errorString());
  // Qt rows run top-down. Native raster rows run bottom-up. Keep source
  // top-left placement by flipping only the pixel row order here.
  const QImage pixels =
      decoded.convertToFormat(QImage::Format_ARGB32).mirrored(false, true);
  TRaster32P raster(payload.width, payload.height);
  raster->lock();
  for (int y = 0; y < payload.height; ++y) {
    const auto *source =
        reinterpret_cast<const QRgb *>(pixels.constScanLine(y));
    auto *target = raster->pixels(y);
    for (int x = 0; x < payload.width; ++x) {
      const QRgb color = source[x];
      target[x] =
          TPixel32(qRed(color), qGreen(color), qBlue(color), qAlpha(color));
    }
  }
  raster->unlock();
  TRop::premultiply(raster);
  TRasterImageP result(raster);
  result->setDpi(dpi, dpi);
  return result;
}

TRasterImageP bitmapImage(Flash::Package &package,
                          const Flash::DocumentInfo &document,
                          const QString &libraryName, double dpi) {
  const QJsonObject item = document.assets.value(libraryName).toObject();
  if (libraryName.isEmpty() || item.value("type") != "DOMBitmapItem")
    throw Flash::Error("Missing bitmap library item: " + libraryName);
  QMap<QString, QString> attributes;
  for (auto it = item.constBegin(); it != item.constEnd(); ++it)
    if (it.key() != "type") attributes.insert(it.key(), it.value().toString());
  return bitmapImage(Flash::readEncodedBitmap(package, attributes), dpi);
}

}  // namespace FlashNative
