#ifndef IGS_ATTENUATION_DISTRIBUTION_H
#define IGS_ATTENUATION_DISTRIBUTION_H

// C++ wrapper for Rust igs_attenuation_distribution FFI
// Original C++ implementation ported to Rust; this header bridges legacy callers.
//
// TRANSITIONAL FFI — remove once `stdfx/igs_fog.cpp` is ported to Rust; it is the only
// caller. Delete this header, its `#include` in `igs_fog.cpp`, its CMake HEADERS entry,
// `tests/native/igs_attenuation_wrapper_probe.cpp`, and the two exports in
// `igs_attenuation_distribution.rs`. Keep the `igs_attenuation_cases.txt` golden.

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
        default:
            throw std::bad_alloc(); // -4: allocation failed
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
