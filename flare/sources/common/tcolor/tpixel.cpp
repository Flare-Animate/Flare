

#include "tpixel.h"
#include "tpixelgr.h"

const int TPixelRGBM32::maxChannelValue = 0xff;
const int TPixelRGBM64::maxChannelValue = 0xffff;
const int TPixelGR8::maxChannelValue    = 0xff;
const int TPixelGR16::maxChannelValue   = 0xffff;

const TPixelRGBM32 TPixelRGBM32::Red(maxChannelValue, 0, 0);
const TPixelRGBM32 TPixelRGBM32::Green(0, maxChannelValue, 0);
const TPixelRGBM32 TPixelRGBM32::Blue(0, 0, maxChannelValue);
const TPixelRGBM32 TPixelRGBM32::Yellow(maxChannelValue, maxChannelValue, 0);
const TPixelRGBM32 TPixelRGBM32::Cyan(0, maxChannelValue, maxChannelValue);
const TPixelRGBM32 TPixelRGBM32::Magenta(maxChannelValue, 0, maxChannelValue);
const TPixelRGBM32 TPixelRGBM32::Gray(maxChannelValue/2, maxChannelValue/2,
                                      maxChannelValue/2);
const TPixelRGBM32 TPixelRGBM32::White(maxChannelValue, maxChannelValue,
                                       maxChannelValue);
const TPixelRGBM32 TPixelRGBM32::Black(0, 0, 0);
const TPixelRGBM32 TPixelRGBM32::Transparent(0, 0, 0, 0);
//---------------------------------------------------

const TPixelRGBM64 TPixelRGBM64::Red(maxChannelValue, 0, 0);
const TPixelRGBM64 TPixelRGBM64::Green(0, maxChannelValue, 0);
const TPixelRGBM64 TPixelRGBM64::Blue(0, 0, maxChannelValue);
const TPixelRGBM64 TPixelRGBM64::Yellow(maxChannelValue, maxChannelValue, 0);
const TPixelRGBM64 TPixelRGBM64::Cyan(0, maxChannelValue, maxChannelValue);
const TPixelRGBM64 TPixelRGBM64::Magenta(maxChannelValue, 0, maxChannelValue);
const TPixelRGBM64 TPixelRGBM64::Gray(maxChannelValue/2, maxChannelValue/2,
                                      maxChannelValue/2);
const TPixelRGBM64 TPixelRGBM64::White(maxChannelValue, maxChannelValue,
                                       maxChannelValue);
const TPixelRGBM64 TPixelRGBM64::Black(0, 0, 0);
const TPixelRGBM64 TPixelRGBM64::Transparent(0, 0, 0, 0);
//---------------------------------------------------
const TPixelD TPixelD::Red(1, 0, 0);
const TPixelD TPixelD::Green(0, 1, 0);
const TPixelD TPixelD::Blue(0, 0, 1);
const TPixelD TPixelD::Yellow(1, 1, 0);
const TPixelD TPixelD::Cyan(0, 1, 1);
const TPixelD TPixelD::Magenta(1, 0, 1);
const TPixelD TPixelD::Gray(0.5, 0.5, 0.5);
const TPixelD TPixelD::White(1, 1, 1);
const TPixelD TPixelD::Black(0, 0, 0);
const TPixelD TPixelD::Transparent(0, 0, 0, 0);
//---------------------------------------------------
const float TPixelF::maxChannelValue = 1.f;
const TPixelF TPixelF::Red(1.f, 0.f, 0.f);
const TPixelF TPixelF::Green(0.f, 1.f, 0.f);
const TPixelF TPixelF::Blue(0.f, 0.f, 1.f);
const TPixelF TPixelF::Yellow(1.f, 1.f, 0.f);
const TPixelF TPixelF::Cyan(0.f, 1.f, 1.f);
const TPixelF TPixelF::Magenta(1.f, 0.f, 1.f);
const TPixelF TPixelF::White(1.f, 1.f, 1.f);
const TPixelF TPixelF::Black(0.f, 0.f, 0.f);
const TPixelF TPixelF::Transparent(0.f, 0.f, 0.f, 0.f);
//---------------------------------------------------
const TPixelGR8 TPixelGR8::White(maxChannelValue);
const TPixelGR8 TPixelGR8::Black(0);

const TPixelGR16 TPixelGR16::White(maxChannelValue);
const TPixelGR16 TPixelGR16::Black(0);

// TPixelGR8::from(TPixel32), TPixelGR16::from(TPixel64) and TPixelGRF::from(TPixelF)
// moved to tpixel.rs (inline in tpixelgr.h). Only the static constant objects stay here.
