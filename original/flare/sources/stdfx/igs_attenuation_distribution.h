#ifndef IGS_ATTENUATION_DISTRIBUTION_H
#define IGS_ATTENUATION_DISTRIBUTION_H

// C++ wrapper for Rust igs_attenuation_distribution FFI
// Original C++ implementation ported to Rust; this header bridges legacy callers.
//
// TRANSITIONAL FFI — remove once `stdfx/igs_fog.cpp` is ported to Rust; it is the only
// caller. Delete this header, its `#include` in `igs_fog.cpp`, its CMake HEADERS entry,
// `tests/native/igs_attenuation_wrapper_probe.cpp`, and the two exports in
// `igs_attenuation_distribution.rs`. Keep the `igs_attenuation_cases.txt` golden.
//
// Status contract (all-or-nothing):
//   0   OK         - all four out-parameters are published together
//  -1   curve is zero                      -> std::domain_error
//  -2   diameter <= 1                      -> std::domain_error
//  -3   null out-parameter; nothing written -> std::invalid_argument
//  -4   malloc failed; nothing published    -> std::bad_alloc
//  -5   matrix exceeds 1 GiB (~11.5k px, radius ~5.8k) -> std::invalid_argument
// The Rust side null-checks first, then zeroes all four out-parameters, then computes,
// then publishes. So on every failure path the caller can free unconditionally, and
// `-5` is deliberately distinct from `-4`: an absurd radius is a caller bug, not an
// allocation failure. The 1 GiB guard is a divergence from the original (which would
// attempt the allocation and let bad_alloc propagate); it is compatible because
// igs_fog.cpp never asks for a lens anywhere near that size.

#include <vector>
#include <stdexcept>
#include <new> // std::bad_alloc for the allocation-failure status

extern "C" {
    int flare_igs_attenuation_distribution(
        double radius,
        double curve,
        int polygon_num,
        double degree,
        double** out_matrix,
        int** out_offsets,
        int** out_sizes,
        int* out_odd_diameter
    );

    void flare_igs_attenuation_free(
        double* matrix,
        int* offsets,
        int* sizes
    );
}

// C++ convenience wrapper matching original signature
inline void attenuation_distribution_(
    std::vector<std::vector<double>>& lens_matrix,
    std::vector<int>& lens_offsets,
    std::vector<double*>& lens_starts,  // row pointers (computed from matrix)
    std::vector<int>& lens_sizes,
    int& odd_diameter,
    double radius,
    double curve,
    int polygon_num = 2,
    double degree = 0.0
) {
    double* matrix_raw = nullptr;
    int* offsets_raw = nullptr;
    int* sizes_raw = nullptr;
    int diam = 0;

    int status = flare_igs_attenuation_distribution(
        radius, curve, polygon_num, degree,
        &matrix_raw, &offsets_raw, &sizes_raw, &diam
    );

    if (status != 0) {
        // All-or-nothing: on any non-zero status no array was published.
        switch (status) {
        case -1:
            throw std::domain_error("attenuation_distribution: curve is zero");
        case -2:
            throw std::domain_error("attenuation_distribution: diameter is <= 1");
        case -3:
            throw std::invalid_argument("attenuation_distribution: null out-parameter");
        case -5:
            throw std::invalid_argument(
                "attenuation_distribution: matrix too large (1 GiB guard)");
        case -4:
            throw std::bad_alloc(); // malloc failed
        default:
            throw std::runtime_error("attenuation_distribution: unknown status");
        }
    }

    odd_diameter = diam;

    // Convert flat matrix to 2D vector
    lens_matrix.resize(diam);
    for (int y = 0; y < diam; ++y) {
        lens_matrix[y].resize(diam);
        for (int x = 0; x < diam; ++x) {
            lens_matrix[y][x] = matrix_raw[y * diam + x];
        }
    }

    // Copy offsets and sizes
    lens_offsets.assign(offsets_raw, offsets_raw + diam);
    lens_sizes.assign(sizes_raw, sizes_raw + diam);

    // Compute starts: pointer to the first covered pixel of each row (the
    // active-run start), NULL for rows with no coverage — matches the original
    // lens_starts assignment in igs_attenuation_distribution.cpp.
    lens_starts.resize(diam);
    for (int y = 0; y < diam; ++y) {
        lens_starts[y] = (lens_offsets[y] >= 0)
                             ? lens_matrix[y].data() + lens_offsets[y]
                             : nullptr;
    }

    flare_igs_attenuation_free(matrix_raw, offsets_raw, sizes_raw);
}

#endif // IGS_ATTENUATION_DISTRIBUTION_H
