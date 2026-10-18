#pragma once

#ifndef igs_motion_wind_pixel_h
#define igs_motion_wind_pixel_h

/* The implementation moved to Rust: stdfx/igs_motion_wind_pixel.rs (crate
   flare_formats). This header keeps the original C++ class API and forwards to
   the opaque Rust handle across the C ABI.

   TRANSITIONAL FFI — remove once `stdfx/igs_motion_wind.cpp` is ported to Rust; it is
   the only caller. Delete this header, its `#include`, its CMake HEADERS entry, and the
   four `flare_igs_motion_wind_pixel_*` exports. Keep the `igs_motion_wind_cases.txt`
   golden. */

extern "C" {
void *flare_igs_motion_wind_pixel_new(
    bool blow_dark_sw, bool blow_alpha_sw, unsigned long length_random_seed,
    double length_min, double length_max, double length_bias, bool length_ref_sw,
    unsigned long force_random_seed, double force_min, double force_max,
    double force_bias, bool force_ref_sw, unsigned long density_random_seed,
    double density_min, double density_max, double density_bias,
    bool density_ref_sw);
int flare_igs_motion_wind_pixel_change(void *self, bool key_reset_sw,
                                       double ref_val, int channels,
                                       double *pixel_tgt);
void flare_igs_motion_wind_pixel_clear(void *self);
void flare_igs_motion_wind_pixel_free(void *self);
}

namespace igs {
namespace motion_wind {
class pixel {
public:
  pixel(const bool blow_dark_sw, const bool blow_alpha_sw

        ,
        const unsigned long length_random_seed, const double length_min,
        const double length_max, const double length_bias,
        const bool length_ref_sw

        ,
        const unsigned long force_random_seed, const double force_min,
        const double force_max, const double force_bias, const bool force_ref_sw

        ,
        const unsigned long density_random_seed, const double density_min,
        const double density_max, const double density_bias,
        const bool density_ref_sw)
      : h_(flare_igs_motion_wind_pixel_new(
            blow_dark_sw, blow_alpha_sw, length_random_seed, length_min,
            length_max, length_bias, length_ref_sw, force_random_seed,
            force_min, force_max, force_bias, force_ref_sw, density_random_seed,
            density_min, density_max, density_bias, density_ref_sw)) {}

  ~pixel() { flare_igs_motion_wind_pixel_free(this->h_); }

  int change(const bool key_reset_sw
             /***, const int ref_channel
        , const double *ref_pixel***/ /* ゼロなら参照しない */
             ,
             const double ref_val /* ゼロ以上なら有効値、マイナスなら無効 */
             ,
             const int channels, double *pixel_tgt) {
    return flare_igs_motion_wind_pixel_change(this->h_, key_reset_sw, ref_val,
                                              channels, pixel_tgt);
  }
  void clear(void) { flare_igs_motion_wind_pixel_clear(this->h_); }

private:
  void *h_;

  /* copy constructorを無効化 */
  pixel(const pixel &);

  /* 代入演算子を無効化 */
  pixel &operator=(const pixel &);
};
}  // namespace motion_wind
}  // namespace igs

#endif /* !igs_motion_wind_pixel_h */
