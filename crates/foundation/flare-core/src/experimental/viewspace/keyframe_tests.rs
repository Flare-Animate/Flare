use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};

type T = Result<(), ViewSpaceError>;

fn mesh() -> Result<BaseMesh, ViewSpaceError> {
    parametric_head(&HeadParams::default())
}

#[test]
fn view_params_validation() {
    assert!(ViewParams::default().validate().is_ok());
    for v in [
        ViewParams {
            azimuth: f32::NAN,
            ..ViewParams::default()
        },
        ViewParams {
            elevation: Some(2.0),
            ..ViewParams::default()
        },
        ViewParams {
            distance: Some(0.0),
            ..ViewParams::default()
        },
        ViewParams {
            perspective: Some(1.0),
            ..ViewParams::default()
        },
    ] {
        assert!(v.validate().is_err(), "{v:?}");
    }
}

#[test]
fn bitmap_checks_sizes_and_samples() -> T {
    assert!(Bitmap::new(2, 2, vec![0; 3]).is_err());
    assert!(Bitmap::new(0, 2, vec![]).is_err());
    assert!(Bitmap::new(MAX_DIM + 1, 1, vec![]).is_err());
    assert!(
        Bitmap::filled(MAX_DIM, MAX_DIM, [0; 4]).is_err(),
        "pixel cap"
    );
    let b = Bitmap::filled(2, 2, [255, 0, 0, 255])?;
    assert_eq!(b.premul(0, 0), Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(b.premul(2, 0), None);
    let mid = b.sample(1.0, 1.0);
    assert_eq!(mid, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(b.sample(-5.0, -5.0), None);
    Ok(())
}

#[test]
fn registration_sees_front_vertices_only() -> T {
    let m = mesh()?;
    let img = Bitmap::filled(256, 256, [200, 100, 50, 255])?;
    let reg = register(
        &m,
        KeyframeSample::new(img, ViewParams::default(), MeshAlignment::default()),
    )?;
    let normals = m.vertex_normals();
    for (i, n) in normals.iter().enumerate() {
        if n[2] > 0.5 {
            assert!(reg.visibility[i] > 0.9, "front vertex {i} unseen");
            assert!((reg.vertex_color[i][0] - 200.0 / 255.0).abs() < 1e-3);
        }
        if n[2] < -0.1 {
            assert_eq!(reg.visibility[i], 0.0, "back vertex {i} seen");
        }
    }
    Ok(())
}

#[test]
fn off_image_vertices_are_unseen() -> T {
    let m = mesh()?;
    let img = Bitmap::filled(16, 16, [255; 4])?;
    let reg = register(
        &m,
        KeyframeSample::new(img, ViewParams::default(), MeshAlignment::default()),
    )?;
    assert!(reg.visibility.iter().filter(|v| **v > 0.0).count() < m.vertices.len() / 4);
    Ok(())
}

#[test]
fn bad_fields_are_rejected() -> T {
    let m = mesh()?;
    let img = Bitmap::filled(8, 8, [0; 4])?;
    let mut s = KeyframeSample::new(img, ViewParams::default(), MeshAlignment::default());
    s.depth_override = Some(vec![0.5; 3]);
    assert!(register(&m, s.clone()).is_err());
    s.depth_override = None;
    s.other_param_fields
        .insert("thickness".into(), vec![f32::NAN; m.vertices.len()]);
    assert!(register(&m, s.clone()).is_err());
    s.other_param_fields.clear();
    s.alignment.scale = 0.0;
    assert!(register(&m, s).is_err());
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let m = mesh()?;
    let img = Bitmap::filled(2, 2, [0; 4])?;
    assert!(matches!(
        MeshAlignment::auto_align(&m, &img),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        import_layers(&[]),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
