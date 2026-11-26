#![allow(clippy::expect_used)]

use flare_core::LayerId;

use crate::{Layer, LayerBlend};

#[test]
fn every_mode_reads_back_from_its_own_name_and_the_usual_spellings() {
    for blend in LayerBlend::ALL {
        assert_eq!(LayerBlend::parse(blend.name()), Some(blend));
        assert_eq!(LayerBlend::parse(blend.label()), Some(blend), "{blend:?}");
    }
    assert_eq!(
        LayerBlend::parse(" Color_Dodge "),
        Some(LayerBlend::ColorDodge)
    );
    assert_eq!(LayerBlend::parse("linear dodge"), Some(LayerBlend::Add));
    assert_eq!(LayerBlend::parse("dissolve"), None);
    assert_eq!(LayerBlend::parse(""), None);
}

#[test]
fn a_layers_blend_and_raster_kind_survive_new_spans() {
    let layer = Layer::new(LayerId::new(), "Paint", vec![])
        .expect("layer")
        .with_blend(LayerBlend::Multiply)
        .with_raster(true);
    assert_eq!(layer.blend(), LayerBlend::Multiply);
    assert!(layer.is_raster());
    let again = layer.with_spans(vec![]).expect("spans");
    assert_eq!(again.blend(), LayerBlend::Multiply);
    assert!(again.is_raster());

    let plain = Layer::new(LayerId::new(), "Ink", vec![]).expect("layer");
    assert_eq!(
        plain.blend(),
        LayerBlend::Normal,
        "a new layer blends normally"
    );
    assert!(!plain.is_raster(), "and is a vector layer");
}
