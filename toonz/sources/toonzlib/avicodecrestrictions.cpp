

#include "avicodecrestrictions.h"
#include "tconvert.h"
#include <windows.h>
#include <vfw.h>

namespace {

void WideChar2Char(LPCWSTR wideCharStr, char *str, int strBuffSize) {
  if (!wideCharStr || !str) return;
  WideCharToMultiByte(CP_ACP, 0, wideCharStr, -1, str, strBuffSize, nullptr,
                      nullptr);
}

HIC getCodec(const std::wstring &codecName, int &bpp) {
  HIC hic = 0;
  ICINFO icinfo;
  memset(&icinfo, 0, sizeof(ICINFO));
  bool found = false;

  char descr[2048], name[2048];
  DWORD fccType = 0;

  BITMAPINFO inFmt;
  memset(&inFmt, 0, sizeof(BITMAPINFO));

  inFmt.bmiHeader.biSize  = sizeof(BITMAPINFOHEADER);
  inFmt.bmiHeader.biWidth = inFmt.bmiHeader.biHeight = 100;
  inFmt.bmiHeader.biPlanes                           = 1;
  inFmt.bmiHeader.biCompression                      = BI_RGB;
  for (bpp = 32; (bpp >= 24) && !found; bpp -= 8) {
    // find the codec.
    inFmt.bmiHeader.biBitCount = bpp;
    for (int i = 0; ICInfo(fccType, i, &icinfo); i++) {
      hic = ICOpen(icinfo.fccType, icinfo.fccHandler, ICMODE_COMPRESS);

      ICGetInfo(hic, &icinfo, sizeof(ICINFO));  // Find out the compressor name
      WideCharToMultiByte(CP_ACP, 0, icinfo.szDescription, -1, descr,
                          sizeof(descr), 0, 0);
      WideCharToMultiByte(CP_ACP, 0, icinfo.szName, -1, name, sizeof(name), 0,
                          0);

      std::string compressorName;
      compressorName = std::string(name) + " '" + std::to_string(bpp) + "' " +
                       std::string(descr);

      if (hic) {
        if (ICCompressQuery(hic, &inFmt, NULL) != ICERR_OK) {
          ICClose(hic);
          continue;  // Skip this compressor if it can't handle the format.
        }
        if (::to_wstring(compressorName) == codecName) {
          found = true;
          break;
        }
        ICClose(hic);
      }
    }
    if (found) break;
  }
  return hic;
}

//-----------------------------------------------------------------------------

bool canWork(const HIC &hic, const TDimension &resolution, int bpp) {
  int lx = resolution.lx, ly = resolution.ly;

  BITMAPINFO bi;
  bi.bmiHeader.biSize          = sizeof(BITMAPINFOHEADER);
  bi.bmiHeader.biPlanes        = 1;
  bi.bmiHeader.biCompression   = BI_RGB;
  bi.bmiHeader.biXPelsPerMeter = 80;
  bi.bmiHeader.biYPelsPerMeter = 72;
  bi.bmiHeader.biClrUsed       = 0;
  bi.bmiHeader.biClrImportant  = 0;
  bi.bmiHeader.biBitCount      = bpp;
  bi.bmiHeader.biWidth         = lx;
  bi.bmiHeader.biHeight        = ly;

  return ICERR_OK == ICCompressQuery(hic, &bi.bmiHeader, NULL);
}
}  // namespace

//-----------------------------------------------------------------------------

void AviCodecRestrictions::getRestrictions(const std::wstring &codecName,
                                           QString &restrictions) {
  restrictions.clear();
  if (codecName == L"Uncompressed") {
    restrictions = QObject::tr("No restrictions for uncompressed avi video");
    return;
  }
  // find the codec
  int bpp;
  HIC hic = getCodec(codecName, bpp);
  if (!hic) {
    restrictions = QObject::tr(
        "It is not possible to communicate with the codec.\n Probably the "
        "codec cannot work correctly.");
    return;
  }

  BITMAPINFO bi;
  bi.bmiHeader.biSize          = sizeof(BITMAPINFOHEADER);
  bi.bmiHeader.biPlanes        = 1;
  bi.bmiHeader.biCompression   = BI_RGB;
  bi.bmiHeader.biXPelsPerMeter = 80;
  bi.bmiHeader.biYPelsPerMeter = 72;
  bi.bmiHeader.biClrUsed       = 0;
  bi.bmiHeader.biClrImportant  = 0;

  int lx = 640, ly = 480;
  bi.bmiHeader.biWidth  = lx;
  bi.bmiHeader.biHeight = ly;

  // Loop until we can find a width, height, and depth that works!
  int i;

  // check the x length
  bi.bmiHeader.biBitCount = bpp;
  for (i = 3; i >= 0; i--) {
    bi.bmiHeader.biWidth = lx + (1 << i);
    bi.bmiHeader.biSizeImage =
        ((bi.bmiHeader.biWidth * bi.bmiHeader.biBitCount + 31) / 32) * 4 * ly;

    if (ICERR_OK != ICCompressQuery(hic, &bi.bmiHeader, NULL)) break;
  }
  if (i >= 0)
    restrictions = QObject::tr("video width must be a multiple of %1")
                       .arg(QString::number(1 << (i + 1)));

  // check the y length
  bi.bmiHeader.biWidth = 640;
  for (i = 3; i >= 0; i--) {
    bi.bmiHeader.biHeight = ly + (1 << i);
    bi.bmiHeader.biSizeImage =
        ((lx * bi.bmiHeader.biBitCount + 31) / 32) * 4 * bi.bmiHeader.biHeight;

    if (ICERR_OK != ICCompressQuery(hic, &bi.bmiHeader, NULL)) break;
  }
  if (i >= 0)
    restrictions = restrictions + "\n" +
                   QObject::tr("video length must be a multiple of %1")
                       .arg(QString::number(1 << (i + 1)));

  ICClose(hic);

  if (restrictions.isEmpty())
    restrictions = QObject::tr("No restrictions for this codec");
  else
    restrictions.prepend(QObject::tr("Resolution restrictions:") + "\n");
}

//-----------------------------------------------------------------------------

bool AviCodecRestrictions::canWriteMovie(const std::wstring &codecName,
                                         const TDimension &resolution) {
  if (codecName == L"Uncompressed") {
    return true;
  }
  // find the codec
  int bpp;
  HIC hic = getCodec(codecName, bpp);
  if (!hic) return false;

  bool test = canWork(hic, resolution, bpp);

  ICClose(hic);
  return test;
}

//-----------------------------------------------------------------------------

bool AviCodecRestrictions::canBeConfigured(const std::wstring &codecName) {
  if (codecName == L"Uncompressed") return false;

  // find the codec
  int bpp;
  HIC hic = getCodec(codecName, bpp);
  if (!hic) return false;

  bool test = ICQueryConfigure(hic);
  ICClose(hic);
  return test;
}

//-----------------------------------------------------------------------------

void AviCodecRestrictions::openConfiguration(const std::wstring &codecName,
                                             void *winId) {
  if (codecName == L"Uncompressed") return;

  // find the codec
  int bpp;
  HIC hic = getCodec(codecName, bpp);
  if (!hic) return;

  ICConfigure(hic, winId);
  ICClose(hic);
}

//-----------------------------------------------------------------------------

QMap<std::wstring, bool> AviCodecRestrictions::getUsableCodecs(
    const TDimension &resolution) {
  QMap<std::wstring, bool> codecs;

  HIC hic = 0;
  ICINFO icinfo;
  memset(&icinfo, 0, sizeof(ICINFO));

  char descr[2048], name[2048], driver[2048];
  DWORD fccType = 0;

  BITMAPINFO inFmt;
  memset(&inFmt, 0, sizeof(BITMAPINFO));

  inFmt.bmiHeader.biSize  = sizeof(BITMAPINFOHEADER);
  inFmt.bmiHeader.biWidth = inFmt.bmiHeader.biHeight = 100;
  inFmt.bmiHeader.biPlanes                           = 1;
  inFmt.bmiHeader.biCompression                      = BI_RGB;

  // List of known DLLs that may cause crashes
  const std::vector<std::string> blockedDlls = {"msvfw32.dll", "lvcod64.dll",
                                                "ff_vfw.dll", "tsccvid64.dll",
                                                "hapcodec.dll"};

  int bpp;
  for (bpp = 32; bpp >= 24; bpp -= 8) {
    inFmt.bmiHeader.biBitCount = bpp;

    for (int i = 0; ICInfo(fccType, i, &icinfo); i++) {
      WideChar2Char(icinfo.szDriver, driver, sizeof(driver));

      // Convert driver name to lowercase for case-insensitive comparison
      std::string driverStr =
          QString::fromStdString(std::string(driver)).toLower().toStdString();

      // Skip blacklisted DLLs before even trying to open the codec
      bool isBlocked =
          std::any_of(blockedDlls.begin(), blockedDlls.end(),
                      [&](const std::string &blocked) {
                        return driverStr.find(blocked) != std::string::npos;
                      });

      if (isBlocked) {
        continue;
      }

      hic = ICOpen(icinfo.fccType, icinfo.fccHandler, ICMODE_COMPRESS);
      if (!hic) {
        continue;
      }

      ICGetInfo(hic, &icinfo, sizeof(ICINFO));

      WideCharToMultiByte(CP_ACP, 0, icinfo.szDescription, -1, descr,
                          sizeof(descr), 0, 0);
      WideCharToMultiByte(CP_ACP, 0, icinfo.szName, -1, name, sizeof(name), 0,
                          0);

      // Blackmagic codec can cause crashes, so stop processing if found
      if (strstr(descr, "Blackmagic") != nullptr) {
        ICClose(hic);
        break;
      }

      std::wstring compressorName =
          ::to_wstring(std::string(name) + " '" + std::to_string(bpp) + "' " +
                       std::string(descr));

      if (ICCompressQuery(hic, &inFmt, NULL) == ICERR_OK) {
        codecs[compressorName] = canWork(hic, resolution, bpp);
      }

      ICClose(hic);
    }
  }

  return codecs;
}
