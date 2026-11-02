

#include "trandom.h"
//#include "tfxparam.h"
#include "perlinnoise.h"

// using std::cout;
// using std::endl;
//-------------------------------------------------------------------

// LinearNoise / Turbolence / Marble live in perlinnoise.rs (inline forwarders in
// perlinnoise.h); this file keeps the table fill, the tuning statics and doClouds.

PerlinNoise::PerlinNoise() : Noise(new float[Size * Size * TimeSize]) {
  TRandom random(1);
  for (int i = 0; i < Size; i++) {
    for (int j = 0; j < Size; j++) {
      for (int k = 0; k < TimeSize; k++) {
        float tmp = random.getFloat();
        // cout << "tmp = " << tmp << "HM = "<< HowMany<< endl;
        Noise[k + TimeSize * j + TimeSize * Size * i] = tmp;
      }
    }
  }

  // cout << "HowMany = " << HowMany <<endl;
}

int PerlinNoise::Size     = 60;
int PerlinNoise::TimeSize = 20;
int PerlinNoise::Offset   = 1000000;
double PerlinNoise::Pixel_size =
    0.01;  // il pixel size va animato da 1 (escluso)
// a 0.1 (consigliato) fino ad un min di 0.001

namespace {
template <typename PIXEL>
void doCloudsT(const TRasterPT<PIXEL> &ras, const TSpectrumT<PIXEL> &spectrum,
               TPointD &tilepos, double evolution, double size, double min,
               double max, int type, double scale) {
  int j;
  TAffine aff = TScale(1 / scale);
  PerlinNoise Noise;
  ras->lock();
  if (type == PNOISE_CLOUDS) {
    for (j = 0; j < ras->getLy(); j++) {
      TPointD pos = tilepos;
      pos.y += j;
      PIXEL *pix    = ras->pixels(j);
      PIXEL *endPix = pix + ras->getLx();
      while (pix < endPix) {
        TPointD posAff = aff * pos;
        double pnoise =
            Noise.Turbolence(posAff.x, posAff.y, evolution, size, min, max);
        pos.x += 1.0;
        *pix++ = spectrum.getPremultipliedValue(pnoise);
      }
    }
  } else {
    for (j = 0; j < ras->getLy(); j++) {
      TPointD pos = tilepos;
      pos.y += j;
      PIXEL *pix    = ras->pixels(j);
      PIXEL *endPix = pix + ras->getLx();
      while (pix < endPix) {
        TPointD posAff = aff * pos;
        double pnoise =
            Noise.Marble(posAff.x, posAff.y, evolution, size, min, max);
        pos.x += 1.0;
        *pix++ = spectrum.getPremultipliedValue(pnoise);
      }
    }
  }
  ras->unlock();
}
}

void doClouds(const TRasterP &ras, const TSpectrumParamP colors, TPointD pos,
              double evolution, double size, double min, double max, int type,
              double scale, double frame) {
  if ((TRaster32P)ras)
    doCloudsT<TPixel32>(ras, colors->getValue(frame), pos, evolution, size, min,
                        max, type, scale);
  else if ((TRaster64P)ras)
    doCloudsT<TPixel64>(ras, colors->getValue64(frame), pos, evolution, size,
                        min, max, type, scale);
  else
    throw TException("CloudsFx: unsupported Pixel Type");
}
