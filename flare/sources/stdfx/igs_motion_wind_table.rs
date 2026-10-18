//! Port of stdfx/igs_motion_wind_table.cpp: generate motion-wind decay tables.
//!
//! Creates a 1D lookup table for motion wind effects with parametric length/force
//! density modulation. Used by igs_motion_wind_pixel (Rust) and the FFI in that module.

/// Returns required table size for given length range.
/// C++: `int igs::motion_wind::table_size(double length_min, double length_max)`
pub fn table_size(length_min: f64, length_max: f64) -> i32 {
    let len = if length_min < length_max { length_max } else { length_min };
    len.ceil() as i32
}

/// Fills `table` with motion wind decay coefficients. Returns the table length.
/// Pure-Rust twin of the original `igs::motion_wind::make_table`; the RNG values are
/// supplied as closures so the pixel port can feed its own seeded LCG stream.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature parameter-for-parameter
pub fn make_table_inplace(
    table: &mut [f64],
    length_min: f64,
    length_max: f64,
    length_bias: f64,
    force_min: f64,
    force_max: f64,
    force_bias: f64,
    density_min: f64,
    density_max: f64,
    density_bias: f64,
    mut length_random_next: impl FnMut() -> f64,
    mut force_random_next: impl FnMut() -> f64,
    mut density_random_next: impl FnMut() -> f64,
) -> i32 {
    // Compute length with bias
    let mut length = length_min;
    if length_min != length_max {
        let mut dif = length_random_next(); // 0..1
        if 0.0 != length_bias {
            dif = dif.powf(1.0 / length_bias);
        }
        length += dif * (length_max - length_min);
    }

    // Compute force curve with bias
    let mut force = force_min;
    if force_min != force_max {
        let mut dif = force_random_next();
        if 0.0 != force_bias {
            dif = dif.powf(1.0 / force_bias);
        }
        force += dif * (force_max - force_min);
    }

    // Compute density with bias
    let mut density = density_min;
    if density_min != density_max {
        let mut dif = density_random_next();
        if 0.0 != density_bias {
            dif = dif.powf(1.0 / density_bias);
        }
        density += dif * (density_max - density_min);
    }

    // `const int table_len = static_cast<int>(ceil(length));`
    let table_len_i32 = length.ceil() as i32;
    if table_len_i32 <= 0 {
        return 0;
    }
    let table_len = table_len_i32 as usize;
    if table_len > table.len() {
        return 0; // the C++ would throw std::out_of_range here
    }

    let table_slice = &mut table[..table_len];

    // Linear decay: table[i] = (length - i) / length
    for (ii, val) in table_slice.iter_mut().enumerate() {
        *val = (length - ii as f64) / length;
    }

    // Apply force curve (gamma)
    if 1.0 != force {
        if 0.0 < force {
            for val in table_slice.iter_mut().skip(1) {
                *val = val.powf(1.0 / force);
            }
        } else {
            // force <= 0: zero out
            for val in table_slice.iter_mut().skip(1) {
                *val = 0.0;
            }
        }
    }

    // Apply density scaling
    for val in table_slice.iter_mut().skip(1) {
        *val *= density;
    }

    table_len as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_size_max() {
        assert_eq!(table_size(2.3, 5.7), 6);
    }

    #[test]
    fn table_size_min() {
        assert_eq!(table_size(5.7, 2.3), 6);
    }

    #[test]
    fn make_table_fixed_length() {
        let mut table = vec![0.0; 10];
        let len = make_table_inplace(
            &mut table,
            3.0,
            3.0, // fixed length
            1.0,
            1.0,
            1.0, // fixed force
            1.0,
            1.0,
            1.0, // fixed density
            1.0,
            || 0.5,
            || 0.5,
            || 0.5,
        );
        assert_eq!(len, 3);
        // Linear decay: [1.0, 0.666..., 0.333...]
        assert!((table[0] - 1.0).abs() < 1e-9);
        assert!((table[1] - 0.666666).abs() < 1e-4);
        assert!((table[2] - 0.333333).abs() < 1e-4);
    }
}
