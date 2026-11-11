pub(super) fn builtin_profile_key(
    profile: flare_tools::brush::BuiltinBrushProfile,
) -> &'static str {
    use flare_tools::brush::BuiltinBrushProfile;
    match profile {
        BuiltinBrushProfile::Plain => "opt-profile-plain",
        BuiltinBrushProfile::Pencil => "opt-profile-pencil",
        BuiltinBrushProfile::Ink => "opt-profile-krita",
        BuiltinBrushProfile::GreasePencil => "opt-profile-grease-pencil",
        BuiltinBrushProfile::Calligraphy => "opt-profile-calligraphy",
        BuiltinBrushProfile::Vector => "opt-profile-opentoonz",
        BuiltinBrushProfile::Animate => "opt-profile-animate",
        BuiltinBrushProfile::GPen => "opt-profile-g-pen",
        BuiltinBrushProfile::MappingPen => "opt-profile-mapping-pen",
        BuiltinBrushProfile::TurnipPen => "opt-profile-turnip-pen",
        BuiltinBrushProfile::BrushPen => "opt-profile-brush-pen",
    }
}

/// The message naming a symmetry mode; the library's own label is English.
pub(super) fn symmetry_key(mode: flare_tools::mirror::Symmetry) -> &'static str {
    use flare_tools::mirror::Symmetry;
    match mode {
        Symmetry::Off => "opt-symmetry-off",
        Symmetry::Vertical => "opt-symmetry-left-right",
        Symmetry::Horizontal => "opt-symmetry-top-bottom",
        Symmetry::Both => "opt-symmetry-four-way",
    }
}
