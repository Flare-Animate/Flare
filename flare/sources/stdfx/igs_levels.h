#pragma once

#ifndef igs_levels_h
#define igs_levels_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_levels.rs (flare_formats); returns 0, 1 (bad channels),
// 2 (bad bits / unsupported image+reference depth) -> std::domain_error, or 3
// (null array / negative size / misaligned buffer) -> std::invalid_argument.
extern "C" {
int flare_igs_levels_change(
    unsigned char *image_array, int height, int width, int channels, int bits,
    const unsigned char *ref, int ref_bits, int ref_mode, double r_in_min,
    double r_in_max, double g_in_min, double g_in_max, double b_in_min,
    double b_in_max, double a_in_min, double a_in_max, double r_gamma,
    double g_gamma, double b_gamma, double a_gamma, double r_out_min,
    double r_out_max, double g_out_min, double g_out_max, double b_out_min,
    double b_out_max, double a_out_min, double a_out_max, bool clamp_sw,
    bool alpha_sw, bool add_blend_sw);
}

namespace igs {
namespace levels {
// (header-only now: IGS_LEVELS_EXPORT is gone, nothing to export)
inline void change(
    unsigned char *image_array, const int height, const int width,
    const int channels, const int bits

    ,
    const unsigned char *ref /* 求める画像と同じ高、幅、channels数 */
    ,
    const int ref_bits /* refがゼロのときはここもゼロ */
    ,
    const int ref_mode /* 0=R,1=G,2=B,3=A,4=Luminance,5=Nothing */

    ,
    const double r_in_min, const double r_in_max  // 0...1
    ,
    const double g_in_min, const double g_in_max  // 0...1
    ,
    const double b_in_min, const double b_in_max  // 0...1
    ,
    const double a_in_min, const double a_in_max  // 0...1
    ,
    const double r_gamma  // 0.1 ... 10.0
    ,
    const double g_gamma  // 0.1 ... 10.0
    ,
    const double b_gamma  // 0.1 ... 10.0
    ,
    const double a_gamma  // 0.1 ... 10.0
    ,
    const double r_out_min, const double r_out_max  // 0...1
    ,
    const double g_out_min, const double g_out_max  // 0...1
    ,
    const double b_out_min, const double b_out_max  // 0...1
    ,
    const double a_out_min, const double a_out_max  // 0...1

    ,
    const bool clamp_sw,
    const bool alpha_sw /* Alphaチャンネルを処理するか否かのSW */
    ,
    const bool add_blend_sw
    /* 効果(変化量)をアルファブレンドするか否かのスイッチ
                add_blend_sw == true
                    アルファ値によりRGBの変化量を調整する
                    合成方法が
                            合成画 = 下絵 * (1 - alpha) + 上絵
                    の場合こちらを使う
                add_blend_sw == false
                    アルファ値に関係なくRGBが変化する
                    合成方法が
                            合成画 = 下絵 * (1 - alpha) + 上絵 * alpha
                    の場合こちらを使う
            */
    ) {
  switch (flare_igs_levels_change(
      image_array, height, width, channels, bits, ref, ref_bits, ref_mode,
      r_in_min, r_in_max, g_in_min, g_in_max, b_in_min, b_in_max, a_in_min,
      a_in_max, r_gamma, g_gamma, b_gamma, a_gamma, r_out_min, r_out_max,
      g_out_min, g_out_max, b_out_min, b_out_max, a_out_min, a_out_max,
      clamp_sw, alpha_sw, add_blend_sw)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  default:
    throw std::invalid_argument("igs::levels::change: bad image");
  }
}
}  // namespace levels
}  // namespace igs

#endif /* !igs_levels_h */
