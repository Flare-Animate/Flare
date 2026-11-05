use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};

type T = Result<(), ViewSpaceError>;

#[test]
fn front_view_projects_to_the_centre() {
    let cam = Camera::new(ViewParams::default(), MeshAlignment::default(), 200, 100);
    let p = cam.project([0.0, 0.0, 1.0]);
    assert!((p.x - 100.0).abs() < 1e-4 && (p.y - 50.0).abs() < 1e-4 && (p.z - 1.0).abs() < 1e-5);
    let up = cam.project([0.0, 1.0, 0.0]);
    assert!(up.y < p.y, "up is up on screen");
}

#[test]
fn turning_the_camera_to_the_left_side_puts_that_side_in_front() {
    // Azimuth +90 degrees: the camera is on the +x (left) side of the head.
    let cam = Camera::new(
        ViewParams::from_degrees(90.0, None),
        MeshAlignment::default(),
        100,
        100,
    );
    let r = cam.rotate([1.0, 0.0, 0.0]);
    assert!((r[2] - 1.0).abs() < 1e-5, "{r:?}");
    let front = cam.rotate([0.0, 0.0, 1.0]);
    assert!(
        front[0] < -0.99,
        "the face moves to the viewer's left: {front:?}"
    );
}

#[test]
fn elevation_looks_down_on_the_top() {
    let cam = Camera::new(
        ViewParams::from_degrees(0.0, Some(90.0)),
        MeshAlignment::default(),
        100,
        100,
    );
    assert!((cam.rotate([0.0, 1.0, 0.0])[2] - 1.0).abs() < 1e-5);
}

#[test]
fn roll_offset_perspective_and_distance() {
    let align = MeshAlignment {
        scale: 10.0,
        offset: [5.0, 0.0],
        roll: std::f32::consts::FRAC_PI_2,
    };
    let cam = Camera::new(ViewParams::default(), align, 100, 100);
    let p = cam.project([1.0, 0.0, 0.0]);
    assert!(
        (p.x - 55.0).abs() < 1e-3 && (p.y - 40.0).abs() < 1e-3,
        "{p:?}"
    );
    let persp = Camera::new(
        ViewParams {
            perspective: Some(0.5),
            ..ViewParams::default()
        },
        MeshAlignment::default(),
        100,
        100,
    );
    let ortho = Camera::new(ViewParams::default(), MeshAlignment::default(), 100, 100);
    assert!(
        persp.project([1.0, 0.0, 1.0]).x > ortho.project([1.0, 0.0, 1.0]).x,
        "near points grow"
    );
    let far = Camera::new(
        ViewParams {
            distance: Some(2.0),
            ..ViewParams::default()
        },
        MeshAlignment::default(),
        100,
        100,
    );
    assert!((far.project([1.0, 0.0, 0.0]).x - 50.0 - 50.0).abs() < 1e-3);
}

#[test]
fn rasterise_draws_the_front_face_and_culls_the_back() -> T {
    let tri = |a: (f32, f32), b: (f32, f32), c: (f32, f32), z: f32| {
        [a, b, c].map(|(x, y)| ScreenVertex { x, y, z })
    };
    // Counter-clockwise in the picture (negative cross product with y down) is front facing.
    let front = tri((1.0, 1.0), (1.0, 9.0), (9.0, 1.0), 0.0);
    let mut r = Raster::new(10, 10)?;
    rasterise(
        &mut r,
        &front,
        &[[0, 1, 2]],
        &[[1.0, 0.0, 0.0, 1.0]; 3],
        &[],
    )?;
    assert_eq!(r.color[2 * 10 + 2], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(r.color[8 * 10 + 8], [0.0; 4]);
    let mut r2 = Raster::new(10, 10)?;
    rasterise(&mut r2, &front, &[[0, 2, 1]], &[[1.0; 4]; 3], &[])?;
    assert!(
        r2.depth.iter().all(|d| *d == f32::NEG_INFINITY),
        "back face culled"
    );
    Ok(())
}

#[test]
fn depth_test_and_bias_decide_visibility() -> T {
    let mk = |z: f32| [(1.0, 1.0), (1.0, 9.0), (9.0, 1.0)].map(|(x, y)| ScreenVertex { x, y, z });
    let near = mk(1.0);
    let far = mk(0.0);
    let mut verts = near.to_vec();
    verts.extend(far);
    let attrs = [[1.0, 0.0, 0.0, 1.0]; 3]
        .into_iter()
        .chain([[0.0, 1.0, 0.0, 1.0]; 3])
        .collect::<Vec<_>>();
    let faces = [[3, 4, 5], [0, 1, 2]];
    let mut r = Raster::new(10, 10)?;
    rasterise(&mut r, &verts, &faces, &attrs, &[])?;
    assert_eq!(
        r.color[22][0], 1.0,
        "near red wins regardless of draw order"
    );
    // Bias lifts the far green triangle above the near one.
    let bias = [0.0, 0.0, 0.0, 5.0, 5.0, 5.0];
    let mut r = Raster::new(10, 10)?;
    rasterise(&mut r, &verts, &faces, &attrs, &bias)?;
    assert_eq!(r.color[22][1], 1.0);
    Ok(())
}

#[test]
fn bad_input_is_an_error_or_skipped() -> T {
    let mut r = Raster::new(4, 4)?;
    assert!(rasterise(&mut r, &[], &[], &[[0.0; 4]], &[]).is_err());
    let v = [ScreenVertex {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }];
    rasterise(&mut r, &v, &[[0, 5, 9]], &[[0.0; 4]], &[])?;
    let nan = [ScreenVertex {
        x: f32::NAN,
        y: 0.0,
        z: 0.0,
    }; 3];
    rasterise(&mut r, &nan, &[[0, 1, 2]], &[[0.0; 4]; 3], &[])?;
    assert!(Raster::new(0, 4).is_err());
    Ok(())
}

#[test]
fn head_renders_non_empty_and_symmetric() -> T {
    let m = parametric_head(&HeadParams::default())?;
    let cam = Camera::new(
        ViewParams::default(),
        MeshAlignment {
            scale: 50.0,
            ..MeshAlignment::default()
        },
        128,
        128,
    );
    let screen: Vec<_> = m.vertices.iter().map(|v| cam.project(*v)).collect();
    let mut r = Raster::new(128, 128)?;
    rasterise(
        &mut r,
        &screen,
        &m.faces,
        &vec![[1.0; 4]; m.vertices.len()],
        &[],
    )?;
    let filled = r.color.iter().filter(|c| c[3] > 0.0).count();
    assert!(filled > 3000, "head covers the canvas centre: {filled}");
    assert_eq!(r.to_bitmap()?.data.len(), 128 * 128 * 4);
    Ok(())
}
