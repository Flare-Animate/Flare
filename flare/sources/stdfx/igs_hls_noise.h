#pragma once

#ifndef igs_hls_noise_h
#define igs_hls_noise_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_hls_noise.rs (flare_formats); returns 0, 1 (bad channels ->
// std::domain_error) or 3 (null buffer / negative size -> std::invalid_argument).
extern "C" {
int flare_igs_hls_noise_change(
    float *image_array, int height, int width, int channels, const float *ref,
    int camera_x, int camera_y, int camera_w, int camera_h, double hue_range,
    double lig_range, double sat_range, double alp_range,
    unsigned long random_seed, double near_blur, double lig_effective,
    double lig_center, int lig_type, double sat_effective, double sat_center,
    int sat_type, double alp_effective, double alp_center, int alp_type,
    bool add_blend_sw, bool cylindrical);
}

namespace igs {
namespace hls_noise {
// (header-only now: IGS_HLS_NOISE_EXPORT is gone, nothing to export)
inline void change(
    float *image_array, const int height, const int width, const int channels,
    const float *ref, /* 求める画像と同じ高、幅*/
    /* image_arrayに余白が変化してもノイズパターンが変わらない
            ようにするためにカメラエリアを指定する */
    const int camera_x, const int camera_y, const int camera_w,
    const int camera_h, const double hue_range, /* =0.025  0 ... 1.0 */
    const double lig_range,                     /* =0.035  0 ... 1.0 */
    const double sat_range,                     /* =0.0	   0 ... 1.0 */
    const double alp_range,                     /* =0.0	   0 ... 1.0 */
    const unsigned long random_seed,            /* =1      0 ... ULONG_MAX */
    const double near_blur,                     /* =0.500  0 ... 0.5 */
    const double lig_effective,                 /* =0.0	   0 ... 1.0 */
    const double lig_center,                    /* =0.5    0 ... 1.0 */
    const int lig_type,                         /* =0
                             0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term) */
    const double sat_effective,                 /* =0.0	   0 ... 1.0 */
    const double sat_center,                    /* =0.5	   0 ... 1.0 */
    const int sat_type,                         /* =0
                             0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term) */
    const double alp_effective,                 /* =0.0	   0 ... 1.0 */
    const double alp_center,                    /* =0.5	   0 ... 1.0 */
    const int alp_type,                         /* =0
                             0(shift_whole),1(shift_term),2(decrease_whole),3(decrease_term) */
    const bool add_blend_sw,
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
    const bool cylindrical = true  // colorspace shape: cylindrical or bicone
) {
  switch (flare_igs_hls_noise_change(
      image_array, height, width, channels, ref, camera_x, camera_y, camera_w,
      camera_h, hue_range, lig_range, sat_range, alp_range, random_seed,
      near_blur, lig_effective, lig_center, lig_type, sat_effective,
      sat_center, sat_type, alp_effective, alp_center, alp_type, add_blend_sw,
      cylindrical)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  default:
    throw std::invalid_argument("igs::hls_noise::change: bad image");
  }
}
}  // namespace hls_noise
}  // namespace igs

#endif /* !igs_hls_noise_h */
