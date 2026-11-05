use super::*;

fn az(deg: f32) -> ViewParams {
    ViewParams::from_degrees(deg, None)
}

#[test]
fn distance_wraps_around_and_is_symmetric() {
    assert!((angular_distance(&az(350.0), &az(10.0)).to_degrees() - 20.0).abs() < 1e-3);
    assert!((angular_distance(&az(0.0), &az(180.0)).to_degrees() - 180.0).abs() < 1e-3);
    let a = ViewParams::from_degrees(30.0, Some(20.0));
    assert_eq!(
        angular_distance(&a, &az(5.0)),
        angular_distance(&az(5.0), &a)
    );
    assert!(angular_distance(&a, &a) < 1e-6);
}

#[test]
fn elevation_counts_in_distance() {
    let up = ViewParams::from_degrees(0.0, Some(90.0));
    assert!((angular_distance(&az(0.0), &up).to_degrees() - 90.0).abs() < 1e-3);
}

#[test]
fn weights_sum_to_one_and_favour_the_nearer() {
    let views = [az(0.0), az(90.0), az(-90.0), az(180.0)];
    let n = select(&az(30.0), &views, &NeighbourConfig::default()).unwrap_or_default();
    assert_eq!(n.len(), 3);
    assert!((n.iter().map(|x| x.weight).sum::<f32>() - 1.0).abs() < 1e-5);
    assert_eq!(n[0].index, 0);
    assert!(n[0].weight > n[1].weight);
}

#[test]
fn exact_hit_gets_all_weight() {
    let views = [az(0.0), az(45.0)];
    let n = select(&az(45.0), &views, &NeighbourConfig::default()).unwrap_or_default();
    assert_eq!(n.len(), 1);
    assert_eq!((n[0].index, n[0].weight), (1, 1.0));
}

#[test]
fn threshold_and_empty_inputs() {
    let cfg = NeighbourConfig {
        max_distance: Some(0.1),
        ..NeighbourConfig::default()
    };
    assert!(
        select(&az(90.0), &[az(0.0)], &cfg)
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        select(&az(0.0), &[], &NeighbourConfig::default())
            .unwrap_or_default()
            .is_empty()
    );
}

#[test]
fn all_weight_functions_normalise_and_slerp_matches_linear_at_midpoint() {
    let views = [az(0.0), az(60.0)];
    for f in [
        WeightFn::InverseDistance { power: 1.0 },
        WeightFn::Slerp,
        WeightFn::Gaussian { sigma: 0.5 },
    ] {
        let cfg = NeighbourConfig {
            k: 2,
            weight_fn: f,
            ..NeighbourConfig::default()
        };
        let n = select(&az(30.0), &views, &cfg).unwrap_or_default();
        assert!(
            (n.iter().map(|x| x.weight).sum::<f32>() - 1.0).abs() < 1e-5,
            "{f:?}"
        );
        assert!(
            (n[0].weight - 0.5).abs() < 1e-3,
            "midpoint is even for {f:?}"
        );
    }
}

#[test]
fn gaussian_far_away_still_normalises() {
    let cfg = NeighbourConfig {
        weight_fn: WeightFn::Gaussian { sigma: 0.001 },
        ..NeighbourConfig::default()
    };
    let n = select(&az(90.0), &[az(0.0), az(180.0)], &cfg).unwrap_or_default();
    assert!((n.iter().map(|x| x.weight).sum::<f32>() - 1.0).abs() < 1e-5);
}

#[test]
fn invalid_views_are_errors() {
    let bad = ViewParams {
        azimuth: f32::NAN,
        ..ViewParams::default()
    };
    assert!(select(&bad, &[az(0.0)], &NeighbourConfig::default()).is_err());
    assert!(select(&az(0.0), &[bad], &NeighbourConfig::default()).is_err());
}
