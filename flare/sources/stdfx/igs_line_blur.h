#ifndef igs_line_blur_h
#define igs_line_blur_h

#include <stdexcept> /* std::domain_error */

#ifndef IGS_LINE_BLUR_EXPORT
#define IGS_LINE_BLUR_EXPORT
#endif

// Implemented in igs_line_blur.rs (flare_formats). Returns 0, or the status whose text
// flare_igs_line_blur_message() gives: every non-zero status is thrown as
// std::domain_error (the original threw std::domain_error for all its failures).
// reference_channel is passed as long long (C++ `long` is 32 bit on LLP64).
extern "C" {
int flare_igs_line_blur_convert(
    const void *in, void *out, int height, int width, int in_wrap, int out_wrap,
    int channels, int bits, int b_blur_count, double b_blur_power,
    int b_subpixel, int b_blur_near_ref, int b_blur_near_len,
    int b_smudge_thick, double b_smudge_remain, int v_smooth_retry,
    int v_near_ref, int v_near_len, bool mv_sw, bool pv_sw, bool cv_sw,
    long long reference_channel, bool debug_save_sw, int brush_action);
const char *flare_igs_line_blur_message(int status);
}

namespace igs {
namespace line_blur {
inline void convert(
    /* 入出力画像 */
    const void *in,    // no_margin
    void *out,         // no_margin
    const int height,  // no_margin
    const int width,   // no_margin
    const int in_wrap, const int out_wrap, const int channels, const int bits,
    /* Action Geometry */
    const int b_blur_count,    /* min=1   def=51   incr=1   max=100  */
    const double b_blur_power, /* min=0.1 def=1    incr=0.1 max=10.0 */
    const int b_subpixel,      /* min=1   def=1    incr=1   max=8	 */
    const int b_blur_near_ref, /* min=1   def=5    incr=1   max=100	 */
    const int b_blur_near_len, /* min=1   def=160  incr=1   max=1000 */
    const int b_smudge_thick,  /* min=1   def=7    incr=1   max=100	 */
    const double b_smudge_remain,
    /* min=0.0 def=0.85 incr=0.001 max=1.0 */
    const int v_smooth_retry,     /* min=0   def=100  incr=1   max=1000 */
    const int v_near_ref,         /* min=0   def=4    incr=1   max=100	 */
    const int v_near_len,         /* min=2   def=160  incr=1   max=1000 */
    const bool mv_sw,             /* false=OFF */
    const bool pv_sw,             /* false=OFF */
    const bool cv_sw,             /* false=OFF */
    const long reference_channel, /* 3 =Alpha:RGBA orBGRA */
    const bool debug_save_sw,     /* false=OFF */
    const int brush_action        /* 0 =Curve Blur ,1=Smudge Brush */
) {
  const int status = flare_igs_line_blur_convert(
      in, out, height, width, in_wrap, out_wrap, channels, bits, b_blur_count,
      b_blur_power, b_subpixel, b_blur_near_ref, b_blur_near_len,
      b_smudge_thick, b_smudge_remain, v_smooth_retry, v_near_ref, v_near_len,
      mv_sw, pv_sw, cv_sw, static_cast<long long>(reference_channel),
      debug_save_sw, brush_action);
  if (status != 0)
    throw std::domain_error(flare_igs_line_blur_message(status));
}
}  // namespace line_blur
}  // namespace igs

#endif /* !igs_line_blur_h */
