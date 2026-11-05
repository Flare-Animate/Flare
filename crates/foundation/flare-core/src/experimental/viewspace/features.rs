//! Layer 3.3 data: semantic feature presets, materials and pixel masks. The selection algorithms
//! that produce masks are in `magic`.

use super::ViewSpaceError;
use super::keyframe::{KeyframeSample, check_dims};
use serde::{Deserialize, Serialize};

/// Built-in semantic features (spec 3.3). The numbers are the quick-assign hotkeys: `1` to `9`
/// then `0` for the first ten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FeaturePreset {
    EyeL,
    EyeR,
    Pupil,
    Iris,
    Eyebrow,
    Nose,
    UpperLip,
    LowerLip,
    Teeth,
    Tongue,
    Chin,
    CheekFold,
    Forehead,
    Ear,
    HairBangs,
    HairSideLocks,
    HairMain,
    HairAccessory,
    Collar,
    Sleeve,
    Crease,
    Zipper,
    Seam,
    Belt,
    Pocket,
    Scar,
    Accessory,
}

impl FeaturePreset {
    /// Presets in hotkey order.
    pub const ALL: [FeaturePreset; 27] = [
        Self::EyeL,
        Self::EyeR,
        Self::Pupil,
        Self::Iris,
        Self::Eyebrow,
        Self::Nose,
        Self::UpperLip,
        Self::LowerLip,
        Self::Teeth,
        Self::Tongue,
        Self::Chin,
        Self::CheekFold,
        Self::Forehead,
        Self::Ear,
        Self::HairBangs,
        Self::HairSideLocks,
        Self::HairMain,
        Self::HairAccessory,
        Self::Collar,
        Self::Sleeve,
        Self::Crease,
        Self::Zipper,
        Self::Seam,
        Self::Belt,
        Self::Pocket,
        Self::Scar,
        Self::Accessory,
    ];

    /// Number-key quick assign: '1'..='9' and '0'.
    pub fn from_hotkey(key: char) -> Option<Self> {
        let i = match key {
            '1'..='9' => key as usize - '1' as usize,
            '0' => 9,
            _ => return None,
        };
        Self::ALL.get(i).copied()
    }

    /// Mesh region this feature normally lives in (used to seed correspondence); None when the
    /// feature is not tied to a head region (hair, clothing).
    pub fn default_region(self) -> Option<u16> {
        use super::mesh::region as r;
        Some(match self {
            Self::EyeL | Self::Pupil | Self::Iris => r::L_EYE_SOCKET,
            Self::EyeR => r::R_EYE_SOCKET,
            Self::Eyebrow | Self::Forehead => r::FOREHEAD,
            Self::Nose => r::NOSE_BRIDGE,
            Self::UpperLip | Self::Teeth | Self::Tongue => r::UPPER_LIP,
            Self::LowerLip => r::LOWER_LIP,
            Self::Chin => r::CHIN,
            Self::CheekFold => r::L_CHEEK,
            Self::HairBangs | Self::HairSideLocks | Self::HairMain | Self::HairAccessory => {
                r::HAIR_ROOT
            }
            Self::Collar => r::CLOTHING_ATTACH,
            _ => return None,
        })
    }
}

/// A preset or a custom user feature stored in the project. Custom names are 1 to 64 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FeatureId {
    Preset(FeaturePreset),
    Custom(String),
}

impl FeatureId {
    pub fn custom(name: &str) -> Result<Self, ViewSpaceError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(ViewSpaceError::Invalid(
                "a custom feature name is 1 to 64 characters".into(),
            ));
        }
        Ok(Self::Custom(name.to_string()))
    }
}

/// Material tags (second pass). They drive blending, shading and deformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Material {
    Unset = 0,
    Skin = 1,
    Hair = 2,
    Eyes = 3,
    Clothing = 4,
    Accessory = 5,
}

impl Material {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Skin,
            2 => Self::Hair,
            3 => Self::Eyes,
            4 => Self::Clothing,
            5 => Self::Accessory,
            _ => Self::Unset,
        }
    }
}

/// Per-pixel material ids of a keyframe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl MaterialMap {
    pub fn new(width: u32, height: u32) -> Result<Self, ViewSpaceError> {
        let n = check_dims(width, height)?;
        Ok(Self {
            width,
            height,
            data: vec![0; n],
        })
    }

    /// Tag every selected pixel (mask >= 128) with `material`.
    pub fn assign(&mut self, mask: &RegionMask, material: Material) -> Result<(), ViewSpaceError> {
        if (mask.width, mask.height) != (self.width, self.height) {
            return Err(ViewSpaceError::Invalid(
                "mask and material map differ in size".into(),
            ));
        }
        for (m, s) in self.data.iter_mut().zip(&mask.data) {
            if *s >= 128 {
                *m = material as u8;
            }
        }
        Ok(())
    }

    pub fn at(&self, x: u32, y: u32) -> Material {
        if x >= self.width || y >= self.height {
            return Material::Unset;
        }
        Material::from_u8(
            self.data
                .get(y as usize * self.width as usize + x as usize)
                .copied()
                .unwrap_or(0),
        )
    }
}

/// A soft selection: 0 outside, 255 inside, in between feathered. Row-major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionMask {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RegionMask {
    pub fn new(width: u32, height: u32) -> Result<Self, ViewSpaceError> {
        let n = check_dims(width, height)?;
        Ok(Self {
            width,
            height,
            data: vec![0; n],
        })
    }

    pub fn from_data(width: u32, height: u32, data: Vec<u8>) -> Result<Self, ViewSpaceError> {
        if data.len() != check_dims(width, height)? {
            return Err(ViewSpaceError::Invalid(
                "mask data length does not match its size".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    pub fn get(&self, x: i64, y: i64) -> u8 {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return 0;
        }
        self.data
            .get(y as usize * self.width as usize + x as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn set(&mut self, x: i64, y: i64, v: u8) {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return;
        }
        if let Some(p) = self
            .data
            .get_mut(y as usize * self.width as usize + x as usize)
        {
            *p = v;
        }
    }

    /// Number of pixels selected at 50% or more.
    pub fn area(&self) -> usize {
        self.data.iter().filter(|v| **v >= 128).count()
    }

    /// Centroid of the selected pixels (a feature's position in the keyframe), or None if empty.
    pub fn centroid(&self) -> Option<[f32; 2]> {
        let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0u64);
        for (i, v) in self.data.iter().enumerate() {
            if *v >= 128 {
                sx += (i % self.width as usize) as f64 + 0.5;
                sy += (i / self.width as usize) as f64 + 0.5;
                n += 1;
            }
        }
        (n > 0).then(|| [(sx / n as f64) as f32, (sy / n as f64) as f32])
    }
}

/// Assign a mask to a feature of a keyframe (replacing any earlier mask of that feature). The mask
/// must have the keyframe's image size.
pub fn assign_feature(
    sample: &mut KeyframeSample,
    feature: FeatureId,
    mask: RegionMask,
) -> Result<(), ViewSpaceError> {
    let img = sample.image();
    if (mask.width, mask.height) != (img.width, img.height) {
        return Err(ViewSpaceError::Invalid(
            "mask size differs from the keyframe image".into(),
        ));
    }
    const MAX_FEATURES: usize = 256;
    if !sample.feature_map.contains_key(&feature) && sample.feature_map.len() >= MAX_FEATURES {
        return Err(ViewSpaceError::Limit {
            what: "features per keyframe",
            max: MAX_FEATURES,
        });
    }
    sample.feature_map.insert(feature, mask);
    Ok(())
}

/// Warn when the same feature sits at very different places in two keyframes.
pub fn check_feature_consistency(
    _a: &KeyframeSample,
    _b: &KeyframeSample,
) -> Result<Vec<String>, ViewSpaceError> {
    // TODO(viewspace:magic-consistency): For each feature mapped in both samples: take the mask
    // centroid and area, rotate the mesh anchor of the feature's default region into each view
    // (`Camera::project`), and compare the centroid's offset from that anchor, normalised by head
    // width, between the two keyframes. Warn when the two offsets differ by more than 15% of the
    // head width, or the areas differ by more than 3x after correcting for foreshortening
    // (cos of the angular distance between the views). Return one message per feature.
    Err(ViewSpaceError::NotImplemented(
        "multi-keyframe feature consistency check",
    ))
}

#[cfg(test)]
#[path = "features_tests.rs"]
mod tests;
