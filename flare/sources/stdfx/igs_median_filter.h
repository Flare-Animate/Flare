#pragma once

#ifndef igs_median_filter_h
#define igs_median_filter_h

#include <stdexcept> /* std::domain_error, std::invalid_argument */

// Implemented in igs_median_filter.rs (flare_formats); returns 0, 1 (bad channels),
// 2 (bad bits) -> std::domain_error, or 3 (null/overlapping buffers, bad
// geometry, bad radius) -> std::invalid_argument. `in_image` and `out_image`
// must not overlap.
extern "C" {
int flare_igs_median_filter_convert(const unsigned char *in_image, unsigned char *out_image,
          int height, int width, int channels, int bits,
          const unsigned char *ref, int ref_bits, int ref_mode, int zz,
          double radius, int out_side_type);
}

namespace igs {
namespace median_filter {
// (header-only now: IGS_MEDIAN_FILTER_EXPORT is gone, nothing to export)
inline void convert(
    const unsigned char *in_image, unsigned char *out_image

    ,
    const int height, const int width, const int channels, const int bits

    ,
    const unsigned char *ref /* 求める画像と同じ高、幅、channels数 */
    ,
    const int ref_bits /* refがゼロのときはここもゼロ */
    ,
    const int ref_mode /* 0=R,1=G,2=B,3=A,4=Luminance,5=Nothing */

    ,
    const int zz /* =0	0(R),1(G),2(B),3(A),4(EachCh) */
    ,
    const double radius /* =1.7	0... */
    ,
    const int out_side_type /* =0	0(Spread),1(Flip),2(bk),3(Repeat) */
    /* 2013-11-11現在0(Spread)のみ使用 */
) {
  switch (flare_igs_median_filter_convert(in_image, out_image, height, width, channels, bits, ref,
                ref_bits, ref_mode, zz, radius, out_side_type)) {
  case 0:
    return;
  case 1:
    throw std::domain_error("Bad channels,Not rgba/rgb/grayscale");
  case 2:
    throw std::domain_error("Bad bits,Not uchar/ushort");
  default:
    throw std::invalid_argument("igs::median_filter::convert: bad image/radius");
  }
}
}  // namespace median_filter
}  // namespace igs

#endif /* !igs_median_filter_h */
