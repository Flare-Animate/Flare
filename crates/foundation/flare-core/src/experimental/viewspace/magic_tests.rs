use super::*;

type T = Result<(), ViewSpaceError>;

/// 8x4: left half red, right half blue, with a black line at x = 3.
fn two_colours() -> Result<Bitmap, ViewSpaceError> {
    let mut data = Vec::new();
    for _y in 0..4 {
        for x in 0..8 {
            data.extend(match x {
                3 => [0, 0, 0, 255],
                0..=2 => [255, 0, 0, 255],
                _ => [0, 0, 255, 255],
            });
        }
    }
    Bitmap::new(8, 4, data)
}

#[test]
fn contiguous_wand_stops_at_other_colours() -> T {
    let m = wand(
        &two_colours()?,
        (0, 0),
        &WandSettings {
            tolerance: 10.0,
            edge_bias: 0.0,
            contiguous: true,
        },
    )?;
    assert_eq!(m.area(), 12);
    assert_eq!(m.get(3, 0), 0);
    Ok(())
}

#[test]
fn non_contiguous_wand_takes_every_similar_pixel() -> T {
    let img = Bitmap::new(
        3,
        1,
        [[255u8, 0, 0, 255], [0, 0, 255, 255], [255, 0, 0, 255]].concat(),
    )?;
    let s = WandSettings {
        tolerance: 10.0,
        edge_bias: 0.0,
        contiguous: true,
    };
    assert_eq!(wand(&img, (0, 0), &s)?.area(), 1);
    assert_eq!(
        wand(
            &img,
            (0, 0),
            &WandSettings {
                contiguous: false,
                ..s
            }
        )?
        .area(),
        2
    );
    Ok(())
}

#[test]
fn tolerance_widens_the_selection() -> T {
    // A gradient: 0, 20, 40, 60 grey.
    let img = Bitmap::new(
        4,
        1,
        (0..4).flat_map(|i| [i * 20, i * 20, i * 20, 255]).collect(),
    )?;
    let at = |t| {
        wand(
            &img,
            (0, 0),
            &WandSettings {
                tolerance: t,
                edge_bias: 0.0,
                contiguous: true,
            },
        )
        .map(|m| m.area())
    };
    assert_eq!(at(1.0)?, 1);
    assert!(at(25.0)? >= 2 && at(25.0)? < 4);
    assert_eq!(at(200.0)?, 4);
    Ok(())
}

#[test]
fn edge_bias_refuses_steps_along_a_gradient() -> T {
    let img = Bitmap::new(
        4,
        1,
        (0..4).flat_map(|i| [i * 20, i * 20, i * 20, 255]).collect(),
    )?;
    let plain = wand(
        &img,
        (0, 0),
        &WandSettings {
            tolerance: 100.0,
            edge_bias: 0.0,
            contiguous: true,
        },
    )?;
    let biased = wand(
        &img,
        (0, 0),
        &WandSettings {
            tolerance: 100.0,
            edge_bias: 0.95,
            contiguous: true,
        },
    )?;
    assert_eq!(plain.area(), 4);
    assert!(biased.area() < plain.area());
    Ok(())
}

#[test]
fn wand_rejects_bad_input() -> T {
    let img = two_colours()?;
    let s = WandSettings {
        tolerance: 10.0,
        edge_bias: 0.0,
        contiguous: true,
    };
    assert!(wand(&img, (8, 0), &s).is_err());
    assert!(wand(&img, (0, 4), &s).is_err());
    assert!(
        wand(
            &img,
            (0, 0),
            &WandSettings {
                tolerance: f32::NAN,
                ..s
            }
        )
        .is_err()
    );
    assert!(
        wand(
            &img,
            (0, 0),
            &WandSettings {
                tolerance: -1.0,
                ..s
            }
        )
        .is_err()
    );
    Ok(())
}

fn square(n: i64) -> Result<RegionMask, ViewSpaceError> {
    let mut m = RegionMask::new(11, 11)?;
    for y in 5 - n..=5 + n {
        for x in 5 - n..=5 + n {
            m.set(x, y, 255);
        }
    }
    Ok(m)
}

#[test]
fn grow_and_shrink_are_inverse_on_a_square_and_bounded() -> T {
    let m = square(1)?;
    assert_eq!(m.area(), 9);
    assert_eq!(
        grow(&m, 1)?.area(),
        9 + 4 * 3,
        "4-neighbour grow adds one pixel ring minus corners"
    );
    assert_eq!(shrink(&grow(&m, 1)?, 1)?.area(), 9);
    assert_eq!(shrink(&m, 1)?.area(), 1);
    assert_eq!(shrink(&m, 2)?.area(), 0);
    assert!(grow(&m, MAX_ITERATIONS + 1).is_err());
    assert!(shrink(&m, MAX_ITERATIONS + 1).is_err());
    Ok(())
}

#[test]
fn shrink_eats_into_the_image_border() -> T {
    let full = Bitmap::filled(4, 4, [1; 4])?;
    let m = wand(
        &full,
        (0, 0),
        &WandSettings {
            tolerance: 1.0,
            edge_bias: 0.0,
            contiguous: true,
        },
    )?;
    assert_eq!(m.area(), 16);
    assert_eq!(shrink(&m, 1)?.area(), 4);
    Ok(())
}

#[test]
fn border_is_the_inner_ring() -> T {
    let b = border(&square(2)?, 1)?;
    assert_eq!(b.area(), 25 - 9);
    assert_eq!(b.get(5, 5), 0);
    Ok(())
}

#[test]
fn smooth_removes_speckles_and_fills_pinholes() -> T {
    let mut m = square(3)?;
    m.set(5, 5, 0);
    m.set(0, 0, 255);
    let s = smooth(&m, 1)?;
    assert_eq!(s.get(0, 0), 0, "speckle gone");
    assert_eq!(s.get(5, 5), 255, "pinhole filled");
    assert!(smooth(&m, MAX_ITERATIONS + 1).is_err());
    Ok(())
}

#[test]
fn lasso_fills_a_polygon_and_handles_degenerate_input() -> T {
    let tri = lasso(20, 20, &[(2.0, 2.0), (18.0, 2.0), (2.0, 18.0)])?;
    assert!(tri.get(4, 4) == 255 && tri.get(16, 16) == 0);
    let a = tri.area() as f32;
    assert!((a - 128.0).abs() < 20.0, "area {a} near 16*16/2");
    let rect = lasso(10, 10, &[(2.0, 2.0), (6.0, 2.0), (6.0, 5.0), (2.0, 5.0)])?;
    assert_eq!(rect.area(), 12);
    assert_eq!(lasso(10, 10, &[(1.0, 1.0), (5.0, 5.0)])?.area(), 0);
    assert!(lasso(10, 10, &[(f32::NAN, 0.0); 3]).is_err());
    let off = lasso(10, 10, &[(-50.0, -50.0), (-40.0, -50.0), (-40.0, -40.0)])?;
    assert_eq!(off.area(), 0);
    let big = lasso(
        10,
        10,
        &[(-50.0, -50.0), (50.0, -50.0), (50.0, 50.0), (-50.0, 50.0)],
    )?;
    assert_eq!(
        big.area(),
        100,
        "a polygon larger than the canvas is clipped"
    );
    Ok(())
}

#[test]
fn brush_hardness_pressure_and_erase() -> T {
    let mut hard = RegionMask::new(21, 21)?;
    stamp(
        &mut hard,
        (10.5, 10.5),
        &Brush {
            size: 10.0,
            hardness: 1.0,
            pressure: 1.0,
        },
        false,
    )?;
    let mut soft = RegionMask::new(21, 21)?;
    stamp(
        &mut soft,
        (10.5, 10.5),
        &Brush {
            size: 10.0,
            hardness: 0.0,
            pressure: 1.0,
        },
        false,
    )?;
    assert_eq!(hard.get(10, 10), 255);
    assert!(
        soft.area() < hard.area(),
        "a soft brush selects less at 50%"
    );
    assert!(soft.get(13, 10) > 0 && soft.get(13, 10) < 255);
    let mut light = RegionMask::new(21, 21)?;
    stamp(
        &mut light,
        (10.5, 10.5),
        &Brush {
            size: 10.0,
            hardness: 1.0,
            pressure: 0.0,
        },
        false,
    )?;
    assert!(light.get(10, 10) < 100 && light.get(10, 10) > 0);
    assert!(
        light.area() == 0,
        "light pressure is faint, below the 50% selection level"
    );
    stamp(
        &mut hard,
        (10.5, 10.5),
        &Brush {
            size: 10.0,
            hardness: 1.0,
            pressure: 1.0,
        },
        true,
    )?;
    assert_eq!(hard.area(), 0);
    assert!(
        stamp(
            &mut hard,
            (0.0, 0.0),
            &Brush {
                size: 0.0,
                hardness: 1.0,
                pressure: 1.0
            },
            false
        )
        .is_err()
    );
    assert!(
        stamp(
            &mut hard,
            (f32::NAN, 0.0),
            &Brush {
                size: 5.0,
                hardness: 1.0,
                pressure: 1.0
            },
            false
        )
        .is_err()
    );
    // Off canvas is harmless.
    stamp(
        &mut hard,
        (-100.0, 500.0),
        &Brush {
            size: 5.0,
            hardness: 1.0,
            pressure: 1.0,
        },
        false,
    )?;
    Ok(())
}

#[test]
fn combine_operations() -> T {
    let (a, b) = (square(2)?, square(1)?);
    assert_eq!(combine(&a, &b, Combine::Add)?.area(), 25);
    assert_eq!(combine(&a, &b, Combine::Subtract)?.area(), 16);
    assert_eq!(combine(&a, &b, Combine::Intersect)?.area(), 9);
    assert!(combine(&a, &RegionMask::new(2, 2)?, Combine::Add).is_err());
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let img = two_colours()?;
    assert!(matches!(
        stroke_aware_select(&img, &[]),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    let mats = super::super::features::MaterialMap::new(2, 2)?;
    assert!(matches!(
        semantic_grow(&RegionMask::new(2, 2)?, &mats, 1),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
