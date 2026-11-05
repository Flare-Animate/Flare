#![allow(clippy::expect_used)]
use super::import;
use flare_document::Content;

fn contents(svg: &str) -> Vec<Content> {
    let doc = import(svg.as_bytes()).expect("imports").document;
    let layer = doc.scenes()[0].timeline().layers()[0].clone();
    layer
        .spans()
        .iter()
        .flat_map(|s| std::iter::once(s.content().clone()).chain(s.extras().iter().cloned()))
        .collect()
}

#[test]
fn rect_and_transformed_group_become_shapes() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
      <rect width="10" height="10" fill="#ff0000"/>
      <g transform="translate(20 5)" opacity="0.5"><circle cx="5" cy="5" r="5" fill="none" stroke="#00f" stroke-width="2"/></g>
    </svg>"##;
    let doc = import(svg.as_bytes()).expect("imports").document;
    assert_eq!((doc.stage().width(), doc.stage().height()), (100, 50));
    let c = contents(svg);
    assert_eq!(c.len(), 2);
    let Content::Shape(rect) = &c[0] else {
        panic!("shape")
    };
    assert_eq!(rect.fill().r, 255);
    let Content::Shape(ring) = &c[1] else {
        panic!("shape")
    };
    let stroke = ring.stroke().expect("stroke");
    assert!((stroke.width() - 2.0).abs() < 1e-6);
    assert!((stroke.opacity() - 0.5).abs() < 0.01);
    assert!(ring.fill_is_invisible());
}

#[test]
fn linear_gradient_is_kept_radial_is_reported() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient>
      <radialGradient id="r"><stop offset="0" stop-color="#0f0"/><stop offset="1" stop-color="#000"/></radialGradient></defs>
      <rect width="10" height="10" fill="url(#g)"/><rect width="5" height="5" fill="url(#r)"/></svg>"##;
    let out = import(svg.as_bytes()).expect("imports");
    let c = contents(svg);
    let Content::Shape(a) = &c[0] else {
        panic!("shape")
    };
    assert!(a.gradient().is_some());
    assert!(out.diagnostics.iter().any(|d| d.message.contains("radial")));
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    assert!(import(b"not svg").is_err());
    assert!(import(b"").is_err());
    // A size past the stage limit is refused.
    let huge = r#"<svg xmlns="http://www.w3.org/2000/svg" width="900000" height="10"/>"#;
    assert!(import(huge.as_bytes()).is_err());
    let big = vec![b' '; 33 * 1024 * 1024];
    assert!(import(&big).is_err());
}

#[test]
fn external_pictures_are_not_read() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="10" height="10"><image xlink:href="/etc/passwd" width="10" height="10"/><rect width="1" height="1"/></svg>"#;
    assert_eq!(contents(svg).len(), 1);
}

#[test]
fn deep_nesting_is_bounded() {
    let mut svg =
        String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">"#);
    for _ in 0..200 {
        svg.push_str("<g>");
    }
    svg.push_str(r#"<rect width="1" height="1"/>"#);
    for _ in 0..200 {
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    // Either refused by the parser or cut off by the depth bound; never a crash.
    if let Ok(out) = import(svg.as_bytes()) {
        assert!(
            out.diagnostics.iter().any(|d| d.message.contains("nested"))
                || contents(&svg).len() <= 1
        );
    }
}
