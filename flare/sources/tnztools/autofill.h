#pragma once

#ifndef _AUTOFILL_H_
#define _AUTOFILL_H_

#include "tflareimage.h"
#include "tvectorimage.h"
class TTileSetCM32;

void rect_autofill_learn(const TflareImageP &imgToLearn, int x1, int y1, int x2,
                         int y2);
bool rect_autofill_apply(const TflareImageP &imgToApply, int x1, int y1, int x2,
                         int y2, bool selective, TTileSetCM32 *tileSet);

void autofill_learn(const TflareImageP &imgToLearn);
bool autofill_apply(const TflareImageP &imgToApply, bool selective,
                    TTileSetCM32 *tileSet);

void rect_autofill_learn(const TVectorImageP &imgToLearn, const TRectD &rect);
bool rect_autofill_apply(const TVectorImageP &imgToApply, const TRectD &rect,
                         bool selective);

void stroke_autofill_learn(const TVectorImageP &imgToLearn, TStroke *stroke);
bool stroke_autofill_apply(const TVectorImageP &imgToApply, TStroke *stroke,
                           bool selective);

#endif

