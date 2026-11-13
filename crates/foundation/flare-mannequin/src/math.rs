//! The little 3D arithmetic the figure needs: points, rotations and the angles a joint is posed
//! by. Rotations are orthonormal 3 x 3 matrices, so an inverse is a transpose.

use std::ops::{Add, Mul, Sub};

/// A point or direction in the figure's space: x to the figure's left, y up, z toward the front.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn scale(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k, self.z * k)
    }

    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Each part of this vector multiplied by the same part of `o`.
    pub fn times(self, o: Self) -> Self {
        Self::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    /// The direction of this vector, or none for one too short to have one.
    pub fn normalized(self) -> Option<Self> {
        let length = self.length();
        (length > 1e-12).then(|| self.scale(1.0 / length))
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

/// A rotation, rows of a matrix that turns a column vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rot3(pub [[f64; 3]; 3]);

impl Rot3 {
    pub const IDENTITY: Self = Self([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    pub fn about_x(degrees: f64) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        Self([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])
    }

    pub fn about_y(degrees: f64) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        Self([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])
    }

    pub fn about_z(degrees: f64) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        Self([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])
    }

    /// A turn of `degrees` about the unit `axis`, counterclockwise looking down the axis at the
    /// origin (Rodrigues' formula).
    pub fn about_axis(axis: Vec3, degrees: f64) -> Self {
        let (s, c) = degrees.to_radians().sin_cos();
        let t = 1.0 - c;
        let Vec3 { x, y, z } = axis;
        Self([
            [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
        ])
    }

    /// The smallest turn that takes the direction `from` to the direction `to` (both unit).
    pub fn between(from: Vec3, to: Vec3) -> Self {
        Self::toward(from, to, 1.0)
    }

    /// `share` of the smallest turn that takes the direction `from` to `to` (both unit).
    pub fn toward(from: Vec3, to: Vec3, share: f64) -> Self {
        let axis = from.cross(to);
        let angle = axis.length().atan2(from.dot(to)).to_degrees() * share;
        match axis.normalized() {
            Some(axis) => Self::about_axis(axis, angle),
            None if from.dot(to) >= 0.0 => Self::IDENTITY,
            // Opposite: any axis across `from` does.
            None => {
                let across = from
                    .cross(Vec3::new(1.0, 0.0, 0.0))
                    .normalized()
                    .or_else(|| from.cross(Vec3::new(0.0, 1.0, 0.0)).normalized())
                    .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
                Self::about_axis(across, 180.0 * share)
            }
        }
    }

    /// A joint's rotation from its angles: twist about the bone's own length first, then the bend,
    /// then the sideways swing, so a twist never moves where the bone points.
    pub fn from_angles(angles: Angles) -> Self {
        Self::about_z(angles.side) * Self::about_x(angles.bend) * Self::about_y(angles.twist)
    }

    /// The angles [`from_angles`](Self::from_angles) makes this rotation from. At a bend of a
    /// quarter turn the twist and the swing are the same motion, and the twist is taken as zero.
    pub fn to_angles(self) -> Angles {
        let m = self.0;
        let bend = m[2][1].clamp(-1.0, 1.0).asin();
        if bend.cos() > 1e-9 {
            Angles {
                bend: bend.to_degrees(),
                twist: (-m[2][0]).atan2(m[2][2]).to_degrees(),
                side: (-m[0][1]).atan2(m[1][1]).to_degrees(),
            }
        } else {
            Angles {
                bend: bend.to_degrees(),
                twist: 0.0,
                side: m[1][0].atan2(m[0][0]).to_degrees(),
            }
        }
    }

    pub fn transpose(self) -> Self {
        let m = self.0;
        Self([
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ])
    }

    pub fn apply(self, v: Vec3) -> Vec3 {
        let m = self.0;
        Vec3::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z,
        )
    }
}

impl Mul for Rot3 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let (a, b) = (self.0, o.0);
        let mut out = [[0.0; 3]; 3];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, cell) in row.iter_mut().enumerate() {
                *cell = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
            }
        }
        Self(out)
    }
}

/// How a joint is turned, in degrees: `bend` about the joint's crosswise axis (forward and back),
/// `twist` about the bone's own length, and `side` about the front-to-back axis (out to the side).
/// On the figure's right the twist and the swing are mirrored, so one set of angles poses either
/// side the same way.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Angles {
    pub bend: f64,
    pub twist: f64,
    pub side: f64,
}

impl Angles {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(bend: f64, twist: f64, side: f64) -> Self {
        Self { bend, twist, side }
    }

    pub fn is_finite(self) -> bool {
        self.bend.is_finite() && self.twist.is_finite() && self.side.is_finite()
    }

    /// The same turn seen in a mirror: the twist and the swing change direction.
    pub fn mirrored(self) -> Self {
        Self::new(self.bend, -self.twist, -self.side)
    }

    pub fn as_array(self) -> [f64; 3] {
        [self.bend, self.twist, self.side]
    }

    pub fn from_array([bend, twist, side]: [f64; 3]) -> Self {
        Self::new(bend, twist, side)
    }
}

/// `degrees` brought into (-180, 180].
pub fn wrap_degrees(degrees: f64) -> f64 {
    let wrapped = degrees.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}
