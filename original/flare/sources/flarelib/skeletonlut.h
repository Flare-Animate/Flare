#pragma once

#ifndef _SKELETONLUT_H_
#define _SKELETONLUT_H_

// The tables live in skeletonlut.rs (flare_formats) as C data symbols; skeletonlut.cpp
// is retired. The old `SkeletonLut::<Name>[]` names are kept as array references, so
// `FirstPreseedTable[code]` in the walkers is unchanged. Requires UCHAR (tcommon.h).

extern "C" {
extern const UCHAR flare_skeletonlut_first_preseed_table[256];
extern const UCHAR flare_skeletonlut_first_preseed_table_rev[256];
extern const UCHAR flare_skeletonlut_next_point_table[2048];
extern const UCHAR flare_skeletonlut_next_point_table_rev[2048];
extern const UCHAR flare_skeletonlut_connection_table[256];
extern const UCHAR flare_skeletonlut_next_seed_table[256];
extern const UCHAR flare_skeletonlut_endpoint_table[256];
extern const UCHAR flare_skeletonlut_extreme_endpoint_table[256];
}

namespace SkeletonLut {

inline const UCHAR (&FirstPreseedTable)[256] = ::flare_skeletonlut_first_preseed_table;
inline const UCHAR (&FirstPreseedTableRev)[256] = ::flare_skeletonlut_first_preseed_table_rev;
inline const UCHAR (&NextPointTable)[2048] = ::flare_skeletonlut_next_point_table;
inline const UCHAR (&NextPointTableRev)[2048] = ::flare_skeletonlut_next_point_table_rev;
inline const UCHAR (&ConnectionTable)[256] = ::flare_skeletonlut_connection_table;
inline const UCHAR (&NextSeedTable)[256] = ::flare_skeletonlut_next_seed_table;
inline const UCHAR (&EndpointTable)[256] = ::flare_skeletonlut_endpoint_table;
inline const UCHAR (&ExtremeEndpointTable)[256] = ::flare_skeletonlut_extreme_endpoint_table;

}  // namespace SkeletonLut

#endif
