//! Deterministic fixed-point math. See module docs on `Fixed` below.

/// Number of fractional bits: 8 gives 1/256-tile precision.
pub const FRAC_BITS: u32 = 8;
const FRAC_SCALE: i32 = 1 << FRAC_BITS;

/// A deterministic fixed-point number: raw `i32`, 8 fractional bits
/// (1/256 tile units). Never derive float math from sim state — only
/// `to_f32` at the render boundary.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const ZERO: Fixed = Fixed(0);
    pub const ONE: Fixed = Fixed(FRAC_SCALE);
    /// ~1/sqrt(2), used to scale diagonal movement so it isn't faster
    /// than axis-aligned movement.
    pub const INV_SQRT2: Fixed = Fixed(181);

    pub fn from_int(v: i32) -> Fixed {
        Fixed(v * FRAC_SCALE)
    }

    /// Render-side only: never feed this back into sim state.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / FRAC_SCALE as f32
    }

    /// Widens to i64 internally so the intermediate product can't
    /// overflow i32 before the shift back down.
    pub fn mul(self, rhs: Fixed) -> Fixed {
        Fixed(((self.0 as i64 * rhs.0 as i64) >> FRAC_BITS) as i32)
    }

    /// Widens to i64 internally: the left shift happens before the
    /// divide so fractional precision survives.
    pub fn div(self, rhs: Fixed) -> Fixed {
        Fixed((((self.0 as i64) << FRAC_BITS) / rhs.0 as i64) as i32)
    }

    pub fn abs(self) -> Fixed {
        Fixed(self.0.abs())
    }
}

impl std::ops::Add for Fixed {
    type Output = Fixed;
    fn add(self, rhs: Fixed) -> Fixed {
        Fixed(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Fixed {
    type Output = Fixed;
    fn sub(self, rhs: Fixed) -> Fixed {
        Fixed(self.0 - rhs.0)
    }
}

impl std::ops::Neg for Fixed {
    type Output = Fixed;
    fn neg(self) -> Fixed {
        Fixed(-self.0)
    }
}

/// A 2D point/vector of `Fixed` coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FixedVec2 {
    pub x: Fixed,
    pub y: Fixed,
}

impl FixedVec2 {
    pub fn new(x: Fixed, y: Fixed) -> Self {
        FixedVec2 { x, y }
    }

    /// Squared distance in raw units^2, as i64 so it cannot overflow:
    /// each axis delta is at most an i32, and i64 comfortably holds the
    /// square of an i32 twice over.
    pub fn dist_sq(self, other: FixedVec2) -> i64 {
        let dx = (self.x.0 - other.x.0) as i64;
        let dy = (self.y.0 - other.y.0) as i64;
        dx * dx + dy * dy
    }
}

impl std::ops::Add for FixedVec2 {
    type Output = FixedVec2;
    fn add(self, rhs: FixedVec2) -> FixedVec2 {
        FixedVec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for FixedVec2 {
    type Output = FixedVec2;
    fn sub(self, rhs: FixedVec2) -> FixedVec2 {
        FixedVec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_int_and_to_f32_round_trip() {
        assert_eq!(Fixed::from_int(3).to_f32(), 3.0);
        assert_eq!(Fixed::from_int(-4).to_f32(), -4.0);
        assert_eq!(Fixed::ZERO.to_f32(), 0.0);
        assert_eq!(Fixed::ONE.to_f32(), 1.0);
    }

    #[test]
    fn from_int_uses_frac_bits_as_the_scale() {
        // 1 << FRAC_BITS raw units per integer tile.
        assert_eq!(Fixed::from_int(1).0, 1 << FRAC_BITS);
        assert_eq!(Fixed::from_int(5).0, 5 * (1 << FRAC_BITS));
    }

    #[test]
    fn mul_is_exact_for_whole_numbers() {
        let six = Fixed::from_int(2).mul(Fixed::from_int(3));
        assert_eq!(six, Fixed::from_int(6));
    }

    #[test]
    fn div_is_exact_for_whole_numbers() {
        let three = Fixed::from_int(6).div(Fixed::from_int(2));
        assert_eq!(three, Fixed::from_int(3));
    }

    #[test]
    fn mul_div_round_trip_within_one_raw_unit() {
        let a = Fixed::from_int(7);
        let b = Fixed::from_int(3);
        let back = a.div(b).mul(b);
        assert!((back.0 - a.0).abs() <= 1, "round trip drifted: {:?} vs {:?}", back, a);
    }

    #[test]
    fn abs_negates_negative_values_only() {
        assert_eq!(Fixed::from_int(-5).abs(), Fixed::from_int(5));
        assert_eq!(Fixed::from_int(5).abs(), Fixed::from_int(5));
        assert_eq!(Fixed::ZERO.abs(), Fixed::ZERO);
    }

    #[test]
    fn add_sub_neg_operators() {
        let a = Fixed::from_int(5);
        let b = Fixed::from_int(2);
        assert_eq!(a + b, Fixed::from_int(7));
        assert_eq!(a - b, Fixed::from_int(3));
        assert_eq!(-a, Fixed::from_int(-5));
    }

    #[test]
    fn inv_sqrt2_applied_twice_halves_within_one_raw_unit() {
        // Rotating a unit vector 45 degrees twice should land near 0.5,
        // i.e. ONE * INV_SQRT2 * INV_SQRT2 ~= 0.5 (raw value 128).
        let result = Fixed::ONE.mul(Fixed::INV_SQRT2).mul(Fixed::INV_SQRT2);
        assert!(
            (result.0 - 128).abs() <= 1,
            "expected ~128 (0.5), got {:?}",
            result
        );
    }

    #[test]
    fn fixed_vec2_add_sub() {
        let a = FixedVec2::new(Fixed::from_int(3), Fixed::from_int(4));
        let b = FixedVec2::new(Fixed::from_int(1), Fixed::from_int(2));
        assert_eq!(a + b, FixedVec2::new(Fixed::from_int(4), Fixed::from_int(6)));
        assert_eq!(a - b, FixedVec2::new(Fixed::from_int(2), Fixed::from_int(2)));
    }

    #[test]
    fn dist_sq_matches_pythagorean_triple() {
        // A 3-4-5 triangle, scaled into fixed-point: squared distance
        // should equal (5 tiles)^2 in raw units.
        let a = FixedVec2::new(Fixed::ZERO, Fixed::ZERO);
        let b = FixedVec2::new(Fixed::from_int(3), Fixed::from_int(4));
        let five_raw = Fixed::from_int(5).0 as i64;
        assert_eq!(a.dist_sq(b), five_raw * five_raw);
    }

    #[test]
    fn dist_sq_does_not_overflow_i32_at_long_range() {
        // Two points 200 tiles apart on each axis. In raw units that's
        // 51_200 per axis; squaring either axis alone (~2.62 billion)
        // already exceeds i32::MAX (~2.15 billion), so this would have
        // silently wrapped if dist_sq used i32 arithmetic.
        let a = FixedVec2::new(Fixed::ZERO, Fixed::ZERO);
        let b = FixedVec2::new(Fixed::from_int(200), Fixed::from_int(200));
        let per_axis = Fixed::from_int(200).0 as i64;
        let expected = per_axis * per_axis * 2;
        assert_eq!(a.dist_sq(b), expected);
        assert!(expected > i32::MAX as i64, "test setup should exceed i32::MAX");
    }
}
