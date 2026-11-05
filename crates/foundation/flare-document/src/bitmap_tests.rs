#![allow(clippy::expect_used)]

use flare_core::{BitmapId, InstanceId, Transform2D};

use crate::{Bitmap, BitmapError, BitmapInstance, MAX_BITMAP_SIDE};

#[test]
fn rejects_bad_sizes_and_lengths() {
    let id = BitmapId::new();
    assert_eq!(Bitmap::new(id, "a", 0, 1, vec![]), Err(BitmapError::Size));
    assert_eq!(
        Bitmap::new(id, "a", MAX_BITMAP_SIDE + 1, 1, vec![]),
        Err(BitmapError::Size)
    );
    assert_eq!(
        Bitmap::new(id, "a", 8192, 8192, vec![]),
        Err(BitmapError::TooManyPixels)
    );
    assert!(matches!(
        Bitmap::new(id, "a", 2, 2, vec![0; 15]),
        Err(BitmapError::PixelLength {
            expected: 16,
            got: 15,
            ..
        })
    ));
}

#[test]
fn equal_pixels_hash_equal_and_size_matters() {
    let a = Bitmap::new(BitmapId::new(), "a", 2, 1, vec![1; 8]).expect("valid");
    let b = Bitmap::new(BitmapId::new(), "b", 2, 1, vec![1; 8]).expect("valid");
    let c = Bitmap::new(BitmapId::new(), "c", 1, 2, vec![1; 8]).expect("valid");
    assert_eq!(a.content_hash(), b.content_hash());
    assert_ne!(a.content_hash(), c.content_hash());
}

#[test]
fn opacity_is_clamped() {
    let instance = |o: f32| {
        BitmapInstance::new(InstanceId::new(), BitmapId::new(), Transform2D::IDENTITY)
            .with_opacity(o)
            .opacity()
    };
    assert_eq!(instance(7.0), 1.0);
    assert_eq!(instance(-1.0), 0.0);
    assert_eq!(instance(f32::NAN), 1.0);
}
