use super::*;
use crate::experimental::viewspace::mesh::{dot, region};

type T = Result<(), ViewSpaceError>;

#[test]
fn default_head_is_valid_and_in_budget() -> T {
    let m = parametric_head(&HeadParams::default())?;
    assert_eq!(m.vertices.len(), 11 * 24 + 2);
    assert!(m.vertices.len() < 1000);
    Ok(())
}

#[test]
fn normals_point_outwards() -> T {
    let m = parametric_head(&HeadParams::default())?;
    let normals = m.vertex_normals();
    for (v, n) in m.vertices.iter().zip(&normals) {
        assert!(dot(*v, *n) > 0.0, "inward normal at {v:?}");
    }
    Ok(())
}

#[test]
fn regions_are_labelled_and_symmetric() -> T {
    let m = parametric_head(&HeadParams::default())?;
    for id in [
        region::FOREHEAD,
        region::CHIN,
        region::HAIR_ROOT,
        region::L_EYE_SOCKET,
        region::R_EYE_SOCKET,
    ] {
        assert!(!m.region_vertices(id).is_empty(), "region {id} empty");
    }
    // Left is +x.
    assert!(
        m.region_vertices(region::L_EYE_SOCKET)
            .iter()
            .all(|&i| m.vertices[i][0] > 0.0)
    );
    assert_eq!(
        m.region_vertices(region::L_EYE_SOCKET).len(),
        m.region_vertices(region::R_EYE_SOCKET).len()
    );
    Ok(())
}

#[test]
fn jaw_width_narrows_the_chin_and_eye_spacing_moves_anchors() -> T {
    let wide = parametric_head(&HeadParams {
        jaw_width: 1.0,
        ..HeadParams::default()
    })?;
    let narrow = parametric_head(&HeadParams {
        jaw_width: 0.5,
        ..HeadParams::default()
    })?;
    let chin_w = |m: &BaseMesh| {
        m.region_vertices(region::CHIN)
            .iter()
            .map(|&i| m.vertices[i][0].abs())
            .fold(0.0, f32::max)
    };
    assert!(chin_w(&narrow) < chin_w(&wide));
    let far = parametric_head(&HeadParams {
        eye_spacing: 0.6,
        ..HeadParams::default()
    })?;
    assert!(far.feature_anchors["eye_l"][0] > wide.feature_anchors["eye_l"][0]);
    Ok(())
}

#[test]
fn bad_parameters_are_errors_not_panics() {
    for p in [
        HeadParams {
            width: 0.0,
            ..HeadParams::default()
        },
        HeadParams {
            height: f32::NAN,
            ..HeadParams::default()
        },
        HeadParams {
            rings: 2,
            ..HeadParams::default()
        },
        HeadParams {
            segments: 100_000,
            ..HeadParams::default()
        },
        HeadParams {
            eye_spacing: 2.0,
            ..HeadParams::default()
        },
    ] {
        assert!(parametric_head(&p).is_err());
    }
}
