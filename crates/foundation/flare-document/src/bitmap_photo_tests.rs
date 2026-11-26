#![allow(clippy::expect_used)]

use super::*;

/// A smooth picture, as a frame of video is: a gradient with a soft blob, `alpha` everywhere.
fn picture(width: u32, height: u32, alpha: impl Fn(u32, u32) -> u8) -> Bitmap {
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let blob = (((x as f32 - 40.0).powi(2) + (y as f32 - 30.0).powi(2)).sqrt() * 2.0) as u8;
            rgba.extend_from_slice(&[(x * 2) as u8, (y * 3) as u8, blob, alpha(x, y)]);
        }
    }
    Bitmap::new(BitmapId::new(), "frame", width, height, rgba).expect("bitmap")
}

#[test]
fn a_photo_is_much_smaller_close_to_the_picture_and_keeps_its_identity() {
    let original = picture(96, 64, |_, _| 255);
    let photo = original.as_photo(DEFAULT_PHOTO_QUALITY).expect("photo");
    assert_eq!((photo.id(), photo.name()), (original.id(), original.name()));
    let stored = photo.photo().expect("kept as a photo");
    assert_eq!(stored.quality(), DEFAULT_PHOTO_QUALITY);
    assert!(
        stored.alpha().is_none(),
        "an opaque picture keeps no transparency"
    );
    assert!(
        stored.byte_len() * 4 < original.pixels().len(),
        "{}",
        stored.byte_len()
    );
    let worst = original
        .pixels()
        .iter()
        .zip(photo.pixels())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(worst <= 24, "the colors stay close: off by {worst} at most");
}

#[test]
fn a_photo_read_back_is_the_same_bitmap() {
    let photo = picture(96, 64, |_, _| 255).as_photo(80).expect("photo");
    let stored = photo.photo().expect("photo");
    let again = Bitmap::from_photo(
        photo.id(),
        photo.name(),
        96,
        64,
        stored.quality(),
        stored.jpeg().to_vec(),
        None,
    )
    .expect("read");
    assert_eq!(again, photo);
    assert_eq!(again.content_hash(), photo.content_hash());
}

#[test]
fn transparency_is_kept_exactly() {
    let original = picture(96, 64, |x, y| ((x + y) % 256) as u8);
    let photo = original.as_photo(90).expect("photo");
    let alpha: Vec<u8> = photo
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect();
    let wanted: Vec<u8> = original
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect();
    assert_eq!(alpha, wanted);
    assert_eq!(
        photo.photo().and_then(Photo::alpha),
        Some(wanted.as_slice())
    );
}

#[test]
fn the_same_quality_again_changes_nothing_and_lossless_drops_the_jpeg() {
    let photo = picture(96, 64, |_, _| 255).as_photo(70).expect("photo");
    assert_eq!(photo.as_photo(70).expect("again"), photo);
    let lossless = photo.as_lossless();
    assert!(lossless.photo().is_none());
    assert_eq!(
        lossless.pixels(),
        photo.pixels(),
        "the pixels stay as they are"
    );
}

#[test]
fn a_quality_out_of_range_or_a_picture_too_small_to_gain_is_refused() {
    let original = picture(96, 64, |_, _| 255);
    assert_eq!(original.as_photo(0), Err(BitmapError::PhotoQuality(0)));
    assert_eq!(original.as_photo(101), Err(BitmapError::PhotoQuality(101)));
    // A JPEG's headers alone outweigh one pixel.
    let dot = Bitmap::new(BitmapId::new(), "dot", 1, 1, vec![1, 2, 3, 255]).expect("dot");
    assert_eq!(dot.as_photo(90), Err(BitmapError::PhotoNotSmaller));
}

#[test]
fn a_stored_photo_that_does_not_fit_what_it_claims_is_refused() {
    let photo = picture(96, 64, |_, _| 255).as_photo(80).expect("photo");
    let jpeg = photo.photo().expect("photo").jpeg().to_vec();
    let id = BitmapId::new();
    // Another size than the JPEG's.
    assert!(matches!(
        Bitmap::from_photo(id, "x", 95, 64, 80, jpeg.clone(), None),
        Err(BitmapError::Photo(_))
    ));
    // Not a JPEG at all.
    assert!(matches!(
        Bitmap::from_photo(id, "x", 96, 64, 80, b"not a jpeg".to_vec(), None),
        Err(BitmapError::Photo(_))
    ));
    // Cut short: the decoder fills in what is missing rather than failing (a damaged record is
    // caught by its digest before this), but never makes more than the declared picture.
    match Bitmap::from_photo(id, "x", 96, 64, 80, jpeg[..jpeg.len() / 2].to_vec(), None) {
        Ok(read) => assert_eq!(read.pixels().len(), 96 * 64 * 4),
        Err(error) => assert!(matches!(error, BitmapError::Photo(_))),
    }
    // Transparency that is not a byte a pixel.
    assert!(matches!(
        Bitmap::from_photo(id, "x", 96, 64, 80, jpeg.clone(), Some(vec![0; 10])),
        Err(BitmapError::Photo(_))
    ));
    // A size no bitmap may have, refused before the decoder is asked.
    assert_eq!(
        Bitmap::from_photo(id, "x", 0, 64, 80, jpeg.clone(), None),
        Err(BitmapError::Size)
    );
    assert_eq!(
        Bitmap::from_photo(id, "x", 96, 64, 0, jpeg, None),
        Err(BitmapError::PhotoQuality(0))
    );
}

#[test]
fn a_gray_jpeg_reads_as_the_same_picture_in_color() {
    let gray: Vec<u8> = (0..96 * 64).map(|i| (i % 96) as u8 * 2).collect();
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 90)
        .write_image(&gray, 96, 64, ExtendedColorType::L8)
        .expect("gray jpeg");
    let bitmap = Bitmap::from_photo(BitmapId::new(), "gray", 96, 64, 90, jpeg, None).expect("read");
    assert!(
        bitmap
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255)
    );
}
