use std::cmp::Ordering;
use std::fmt::{Binary, Debug, Display, Formatter, LowerHex, Octal, UpperHex, Write};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};
use bytemuck::{Pod, Zeroable};
use crate::i48_int::i48;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct Fract(u16);

impl Debug for Fract {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

const FRACTIONAL_SCALE: u32 = 1 << 16;
const FRACTIONAL_SCALE_F32: f32 = FRACTIONAL_SCALE as f32;


impl Fract {
    pub const ZERO: Self = Self(0);
    pub const HALF: Self = Self::recip(2);
    
    fn fmt_fractional<const RADIX: u32, const UPPERCASE: bool>(self, f: &mut Formatter) -> std::fmt::Result {
        const { assert!(2 <= RADIX && RADIX <= 16, "radix must be in range 2..=16") }

        if self.0 == 0 {
            return f.write_char('0');
        }

        const DIGITS_LOWER: [char; 16] = {
            let mut chars = ['\0'; 16];
            let mut i = 0;
            while i < 16 {
                chars[i as usize] = match i {
                    0..=9 => b'0' + i,
                    10.. => b'a' + i - 10
                } as char;
                i += 1
            }

            chars
        };
        const DIGITS_UPPER: [char; 16] = {
            let mut chars = DIGITS_LOWER;
            let mut char_ptr = (&mut chars) as &mut [char];

            while let [char, rest @ ..] = char_ptr {
                *char = char.to_ascii_uppercase();
                char_ptr = rest;
            }

            chars
        };

        let radix = RADIX;
        let digits = match UPPERCASE {
            true => &DIGITS_UPPER,
            false => &DIGITS_LOWER
        };

        let mut numerator = self.0 as u32 * radix;
        let max_digits = f.precision();

        let mut digits_emitted = 0;
        while numerator != 0 && max_digits.is_none_or(|max| digits_emitted < max) {
            let (quotient, remainder) = (numerator / FRACTIONAL_SCALE, numerator % FRACTIONAL_SCALE);
            numerator = remainder * radix;

            // its mathematically impossible to fail, but it isn't worth risking the unsafe code
            //
            // self < FRACTIONAL_SCALE
            // self * RADIX < FRACTIONAL_SCALE * RADIX
            // (self * RADIX)/FRACTIONAL_SCALE < RADIX

            let digit_index = quotient as usize;

            let digit = digits[digit_index];
            f.write_char(digit)?;

            digits_emitted += 1;
        }

        if let Some(max) = max_digits {
            for _ in digits_emitted..max {
                f.write_char('0')?
            }
        }

        Ok(())
    }
    
    /// Computes the reciprocal
    pub const fn recip(x: u16) -> Self {
        // 1 / x * FRACTIONAL_SCALE
        assert!(x > 1, "reciprocal 1/1 and 1/0 are invalid");
        
        Self((FRACTIONAL_SCALE / x as u32) as u16)
    }
    
    pub const fn from_f32(float: f32) -> Self {
        debug_assert!(
            0.0 <= float && float < 1.0,
            "invalid fractional passed into `Fract::from_f32`"
        );

        // this is saturating which is nice that we don't have to deal with it
        Self((float * FRACTIONAL_SCALE_F32) as u16)
    }

    pub const fn as_f32(&self) -> f32 {
        self.0 as f32 / FRACTIONAL_SCALE_F32
    }
}


impl Add for Fract {
    type Output = FixedPoint;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        // this never overflows
        FixedPoint(self.0 as i64 + rhs.0 as i64)
    }
}

impl Sub for Fract {
    type Output = FixedPoint;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        // this never overflows
        FixedPoint(self.0 as i64 - rhs.0 as i64)
    }
}

impl Mul for Fract {
    type Output = Fract;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        // expected = fract(1) * fract(2) * SCALE
        // num(n) = fract(n) * SCALE
        //
        // num(1) * num(2) 
        // = (fract(1) * SCALE) * (fract(2) * SCALE)
        // = fract(1) * fract(2) * (SCALE^2)
        // ==> apply / SCALE
        // (num(1) * num(2))/SCALE
        // = fract(1) * fract(2) * SCALE
        // = expected
        
        let result = ((self.0 as u32 * rhs.0 as u32) / FRACTIONAL_SCALE) as u16;
        Self(result)
    }
}

impl Div for Fract {
    type Output = FixedPoint;

    fn div(self, rhs: Self) -> Self::Output {
        // expected = (fract(1)/fract(2)) * SCALE
        // num(n) = fract(n) * SCALE
        //
        // num(1) / num(2) 
        // = (fract(1) * SCALE) / (fract(2) * SCALE)
        // = fract(1) / fract(2)
        // ==> apply * SCALE
        // num(1) / num(2) * SCALE
        // = (fract(1) / fract(2)) * SCALE
        // = expected
        
        let result = (self.0/rhs.0) as i64 * FRACTIONAL_SCALE as i64;
        FixedPoint(result)
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct FixedPoint(i64);

impl Debug for FixedPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (int, frac) = self.to_raw();

        f.debug_struct("FixedFloat")
            .field("integer", &int)
            .field("fractional", &frac)
            .field("bits", &format_args!("{:016X}", self.0))
            .finish()
    }
}

impl FixedPoint {
    pub const ZERO: Self = Self(0);
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);
    
    #[inline(always)]
    pub const fn is_negative(self) -> bool {
        // in memory stored as [i48,fractional]
        self.0 < 0
    }

    #[inline(always)]
    pub const fn from_raw(integer: i48, fractional: Fract) -> Self {
        let bits = (integer.to_bits() << 16) | fractional.0 as u64;
        Self(bits as i64)
    }


    #[inline(always)]
    pub const fn from_int(int: i48) -> Self {
        Self((int.to_bits() << 16) as i64)
    }
    
    #[inline(always)]
    pub const fn from_fract(fractional: Fract) -> Self {
        // zero extends
        Self(fractional.0 as i64)
    }

    #[inline(always)]
    pub const fn to_raw(self) -> (i48, Fract) {
        (self.int(), self.fract())
    }

    pub const fn int(self) -> i48 {
        unsafe { i48::from_bits_unchecked((self.0 as u64) >> 16) }
    }

    pub const fn fract(self) -> Fract {
        // truncates
        Fract(self.0 as u16)
    }

    #[inline(always)]
    pub const fn from_f32(float: f32) -> Self {
        Self((float * FRACTIONAL_SCALE_F32) as i64)
    }

    #[inline(always)]
    pub const fn as_f32(self) -> f32 {
        self.0 as f32 / FRACTIONAL_SCALE_F32
    }
}

// Addition and subtraction arithmetic
// When you add two fixed-point numbers with the same scale (both have 16 fractional bits), 
// you can simply add their underlying integer representations.
// The fractional parts will automatically carry into the integer part when needed.
//
// Saturating arithmetic handles overflow this is similar to how ieee floats work
// and is how most people expect floats to work
// this may and probably should change since saturating is expensive
//
// This matches the behavior of i48 
//
// Two's complement works correctly:
// Because the integer part is stored in two's complement form,
// the arithmetic operations work correctly for both positive and negative numbers.


impl Add for FixedPoint {
    type Output = FixedPoint;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for FixedPoint {
    fn add_assign(&mut self, rhs: Self) {
       *self = (*self) + rhs
    }
}

impl Sub for FixedPoint {
    type Output = FixedPoint;

    fn sub(self, rhs: Self) -> Self::Output {
       Self(self.0.saturating_sub(rhs.0))
    }
}

impl SubAssign for FixedPoint {
    fn sub_assign(&mut self, rhs: Self) {
        *self = (*self) - rhs
    }
}

impl Mul for FixedPoint {
    type Output = FixedPoint;

    fn mul(self, rhs: Self) -> Self::Output {
        // read on Frac::mul
        // on why this works
        
        let x = self.0 as i128;
        let y = rhs.0 as i128;
        let result = (x * y) / FRACTIONAL_SCALE as i128;
        
        if result < FixedPoint::MIN.0 as i128 { 
            return FixedPoint::MIN
        }
        
        if result > FixedPoint::MAX.0 as i128 { 
            return FixedPoint::MAX
        }
        
        FixedPoint(result as i64)
    }
}


impl MulAssign for FixedPoint {
    fn mul_assign(&mut self, rhs: Self) {
        *self = (*self) * rhs
    }
}

impl Div for FixedPoint {
    type Output = FixedPoint;
    
    fn div(self, rhs: Self) -> Self::Output {
        // read on Frac::div
        // on why this works
        
        let x = self.0;
        let y = rhs.0;
        let result = (x / y).saturating_mul(FRACTIONAL_SCALE as i64);
        
        FixedPoint(result)
    }
}

impl DivAssign for FixedPoint {
    fn div_assign(&mut self, rhs: Self) {
        *self = (*self) / rhs
    }
}

impl From<Fract> for FixedPoint {
    #[inline]
    fn from(value: Fract) -> Self {
        Self::from_fract(value)
    }
}

macro_rules! impl_cmp {
    ($ty: ty) => {
        impl $ty {
            #[inline(always)]
            pub const fn const_lt(&self, other: Self) -> bool {
                self.0 < other.0
            }
            
            #[inline(always)]
            pub const fn const_le(&self, other: Self) -> bool {
                self.0 <= other.0
            }
            
            #[inline(always)]
            pub const fn const_gt(&self, other: Self) -> bool {
                self.0 > other.0 
            }
            
            #[inline(always)]
            pub const fn const_ge(&self, other: Self) -> bool {
                self.0 >= other.0
            }
        }
        
        impl PartialOrd for $ty {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                PartialOrd::partial_cmp(&self.0, &other.0)
            }
            
            fn lt(&self, other: &Self) -> bool {
                self.0 < other.0
            }
            
            fn le(&self, other: &Self) -> bool {
                self.0 <= other.0
            }
            
            fn gt(&self, other: &Self) -> bool {
                self.0 > other.0 
            }
            
            fn ge(&self, other: &Self) -> bool {
                self.0 >= other.0
            }
        }
        
        impl Ord for $ty {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> Ordering {
                Ord::cmp(&self.0, &other.0)
            }
        }
    };
}

impl_cmp!(Fract);
impl_cmp!(FixedPoint);

macro_rules! is_upper {
    (uppercase) => { true  };
    (         ) => { false };
}

macro_rules! impl_fmt {
    ($trait: path; base: $base: literal $(uppercase $(@ $upper:tt)?)?) => {
        impl $trait for Fract {
            fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
                f.write_str("0.")?;
                self.fmt_fractional::<{ $base }, {
                    is_upper!($(uppercase $($upper)?)?)
                }>(f)
            }
        }

        impl $trait for FixedPoint {
            fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
                let (int, frac) = self.to_raw();
                <i48 as $trait>::fmt(&int, f)?;
                f.write_char('.')?;
                frac.fmt_fractional::<{ $base }, {
                    is_upper!($(uppercase $($upper)?)?)
                }>(f)
            }
        }
    };
}

impl_fmt! { Display;  base:   10 }
impl_fmt! { Binary;   base: 0b10 }
impl_fmt! { Octal;    base: 0o10 }
impl_fmt! { LowerHex; base: 0x10 }
impl_fmt! { UpperHex; base: 0x10 uppercase }


#[cfg(test)]
mod tests {
    use crate::i48;
    use super::*;

    #[test]
    fn test_fmt() {
        assert_eq!(Fract(8000).to_string(), "0.1220703125");
        assert_eq!(Fract(32768).to_string(), "0.5");
        assert_eq!(Fract(1).to_string(), "0.0000152587890625");
        assert_eq!(Fract(65535).to_string(), "0.9999847412109375");
        assert_eq!(Fract(0).to_string(), "0.0");
        assert_eq!(FixedPoint::from_f32(1.5).to_string(), "1.5");
        assert_eq!(format!("{:.04}", FixedPoint::from_f32(1.5)), "1.5000");
    }

    #[test]
    fn test_fractional_roundtrip() {
        let input = 0.75f32;
        let frac = Fract::from_f32(input);
        let output = frac.as_f32();
        assert!((output - input).abs() < 1e-5, "roundtrip failed: got {}", output);
    }

    #[test]
    #[should_panic(expected = "invalid fractional")]
    fn test_fractional_from_f32_invalid() {
        // This should panic due to the debug_assert
        let _ = Fract::from_f32(1.0);
    }

    #[test]
    fn test_fixed_float_roundtrip() {
        let inputs = [-12345.678, -1.5, 0.0, 0.999, 42.125, 1e6];
        for &val in &inputs {
            let ff = FixedPoint::from_f32(val);
            let out = ff.as_f32();
            let diff = (val - out).abs();
            assert!(
                diff < 1e-5,
                "roundtrip failed: input = {}, output = {}, diff = {}",
                val, out, diff
            );
        }
    }
    
    #[test]
    fn test_arithmetic() {
        assert_eq!(
            FixedPoint::from_int(i48!(2)) / FixedPoint::from_fract(Fract::HALF),
            FixedPoint::from_int(i48!(4))
        )
    }

    #[test]
    fn test_fixed_float_to_from_raw() {
        let integer = i48::from_bits(123456).unwrap();
        let fractional = Fract::from_f32(0.5);
        let ff = FixedPoint::from_raw(integer, fractional);
        let (int_out, frac_out) = ff.to_raw();
        assert_eq!(integer, int_out);
        assert_eq!(fractional, frac_out);
    }

    #[test]
    fn test_fixed_float_is_negative() {
        let neg = FixedPoint::from_f32(-10.25);
        let pos = FixedPoint::from_f32(3.75);
        assert!(neg.is_negative());
        assert!(!pos.is_negative());
    }
}
