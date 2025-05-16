use std::cmp::Ordering;
use std::fmt::{Binary, Debug, Display, Formatter, LowerHex, Octal, UpperHex};
use std::hash::{Hash, Hasher};
use std::hint::assert_unchecked;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Not, Rem, RemAssign, Sub, SubAssign};
use bytemuck::{NoUninit, Zeroable};
use cfg_if::cfg_if;
use likely_stable::unlikely;

#[derive(Copy, Clone, NoUninit, Zeroable)]
#[repr(u8)]
enum AlwaysZero {
    #[expect(dead_code, reason = "this is a hint for the type system, not something created directly")]
    Zero = 0
}

cfg_if! {
    if #[cfg(target_endian = "little")] {
        #[derive(Copy, Clone, NoUninit, Zeroable)]
        #[repr(C, align(8))]
        struct Repr {
            data: [u8; 6],
            _zero0: AlwaysZero,
            _zero1: AlwaysZero,
        }
    } else if #[cfg(target_endian = "big")] {
        #[derive(Copy, Clone, NoUninit, Zeroable)]
        #[repr(C, align(8))]
        pub struct Repr {
            _zero0: AlwaysZero,
            _zero1: AlwaysZero,
            data: [u8; 6],
        }
    } else {
        compiler_error!("unknown endianness")
    }
}


const MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const MAX: i64 = 2_i64.pow(i48::BITS - 1) - 1;
const MIN: i64 = -2_i64.pow(i48::BITS - 1);

impl Repr {
    #[inline(always)]
    pub const unsafe fn hint_bits_good(x: u64) -> u64 {
        // Safety: up to caller
        unsafe {
            // translates to the 2 most significant bits arer zero
            assert_unchecked((x & MASK) == x && (x & !MASK) == 0);
            // and the other is bit magic for (x <= MAX && x >= MIN)
            assert_unchecked(x <= MASK);
        }

        x
    }

    #[inline(always)]
    pub const fn from_bits(x: u64) -> Option<Self> {
        // bit magic for (x > MAX || x < MIN)
        if unlikely(x > MASK) {
            return None
        }

        // Safety: checked above
        Some(unsafe { Self::from_bits_unchecked(x) })
    }

    #[inline(always)]
    pub const fn from_bits_wrapping(x: u64) -> Self {
        // checked by masking
        unsafe { Self::from_bits_unchecked(x & MASK) }
    }

    /// # Safety
    /// `x`'s two most significant bits must be zero
    #[inline(always)]
    pub const unsafe fn from_bits_unchecked(x: u64) -> Self {
        // Safety: up to caller
        unsafe { std::mem::transmute(Self::hint_bits_good(x)) }
    }

    #[inline(always)]
    pub const fn to_bits(self) -> u64 {
        let x = bytemuck::must_cast(self);
        // Safety: the bits of self are guaranteed to pass the safety checks
        unsafe { Self::hint_bits_good(x) }
    }

    #[inline(always)]
    pub const fn as_i64(self) -> i64 {
        // This does sign extension
        // it puts the msb of this
        // int into the msb of i64
        // it then pulls the number down again
        // with a signed shift right which sign extends
        // and fixes the numbers place again
        let x = ((self.to_bits() << 16) as i64) >> 16;
        unsafe { assert_unchecked(x <= MAX && x >= MIN) }
        x
    }
}

const _: () = assert!(
    size_of::<Repr>() == size_of::<i64>()
        && align_of::<Repr>() == align_of::<i64>()
);

#[derive(Copy, Clone, NoUninit, Zeroable)]
#[allow(non_camel_case_types)]
#[repr(transparent)]
pub struct i48(Repr);

impl Hash for i48 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // hashing the zeros would be BAD for hash quality
        state.write(&self.0.data)
    }
}

macro_rules! impl_checked_binop {
    ($($name:ident)+) => {
        $(#[inline(always)]
        pub const fn $name(self, rhs: Self) -> Option<Self> {
            match self.as_i64().$name(rhs.as_i64()) {
                Some(x) => Self::new(x),
                None => None
            }
        })+
    };
}


macro_rules! impl_checked_unop {
    ($($name:ident)+) => {
        $(#[inline(always)]
        pub const fn $name(self) -> Option<Self> {
            match self.as_i64().$name() {
                Some(x) => Self::new(x),
                None => None
            }
        })+
    };
}


macro_rules! impl_wrapping_binop {
    ($($name:ident)+) => {
        $(#[inline(always)]
        pub const fn $name(self, rhs: Self) -> Self {
            Self::new_wrapping(self.as_i64().$name(rhs.as_i64()))
        })+
    };
}

macro_rules! impl_wrapping_unop {
    ($($name:ident)+) => {
        $(#[inline(always)]
        pub const fn $name(self) -> Self {
            Self::new_wrapping(self.as_i64().$name())
        })+
    };
}


#[macro_export]
macro_rules! i48 {
    ($expr: expr) => {
        const {
            $crate::i48_int::i48::new($expr)
                .expect(concat!(stringify!($expr), " evaluated out of the bounds of an i48"))
        }
    };
}

impl i48 {
    pub const BITS: u32 = 48;
    pub const MAX: Self = i48!(MAX);
    pub const MIN: Self = i48!(MIN);

    #[inline(always)]
    pub const fn from_bits(x: u64) -> Option<Self> {
        match Repr::from_bits(x) {
            Some(x) => Some(Self(x)),
            None => None
        }
    }

    #[inline(always)]
    pub const unsafe fn from_bits_unchecked(x: u64) -> Self {
        match cfg!(debug_assertions) {
            true => Self::from_bits(x).expect("`i48::new_unchecked` contract violated"),
            false => {
                // Safety: up to caller
                Self(unsafe { Repr::from_bits_unchecked(x) })
            }
        }
    }


    #[inline(always)]
    pub const fn from_bits_wrapping(x: u64) -> Self {
        Self(Repr::from_bits_wrapping(x))
    }


    #[inline(always)]
    pub const fn new(x: i64) -> Option<Self> {
        if x > MAX || x < MIN {
            return None
        }

        Some(unsafe { Self::new_unchecked(x) })
    }

    #[inline(always)]
    pub const unsafe fn new_unchecked(x: i64) -> Self {
        // Safety: up to caller
        unsafe { assert_unchecked(x <= MAX && x >= MIN) }

        Self::new_wrapping(x)
    }

    #[inline(always)]
    pub const fn new_wrapping(x: i64) -> Self {
        Self::from_bits_wrapping(x as u64)
    }

    #[inline(always)]
    pub const fn as_i64(self) -> i64 {
        self.0.as_i64()
    }

    pub const fn to_bits(self) -> u64 {
        self.0.to_bits()
    }


    impl_checked_binop! {
        checked_add
        checked_sub
        checked_mul
        checked_div
        checked_div_euclid
        checked_rem
        checked_rem_euclid
    }

    impl_checked_unop! {
        checked_abs
        checked_neg
        checked_isqrt
    }

    impl_wrapping_binop! {
        wrapping_add
        wrapping_sub
        wrapping_mul
        wrapping_div
        wrapping_div_euclid
        wrapping_rem
        wrapping_rem_euclid
    }

    impl_wrapping_unop! {
        wrapping_abs
    }

    #[inline(always)]
    pub const fn wrapping_neg(self) -> Self {
        Self::from_bits_wrapping((!self.to_bits()) + 1)
    }
}



macro_rules! lossless_signed_from {
    ($($ty: ty),+ $(,)?) => {
        $(impl From<$ty> for i48 {
            #[inline(always)]
            fn from(value: $ty) -> Self {
                const { assert!(<$ty>::BITS < i48::BITS && <$ty>::MIN < 0) }
                unsafe { Self::new_unchecked(value as i64) }
            }
        })+
    };
}

macro_rules! lossless_unsigned_from {
    ($($ty: ty),+ $(,)?) => {
        $(impl From<$ty> for i48 {
            #[inline(always)]
            fn from(value: $ty) -> Self {
                const { assert!(<$ty>::BITS < i48::BITS && <$ty>::MIN == 0) }
                unsafe { Self::from_bits_unchecked(value as u64) }
            }
        })+
    };
}

lossless_signed_from! { i8, i16, i32 }
lossless_unsigned_from! { u8, u16, u32 }



macro_rules! lossy_signed_from {
    ($($ty: ty),+ $(,)?) => {
        $(impl TryFrom<$ty> for i48 {
            type Error = <i8 as TryFrom<i128>>::Error;

            #[inline(always)]
            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                const { assert!(<$ty>::BITS > i48::BITS && <$ty>::MIN < 0) }

                if value > const { MAX as $ty } || value < const { MIN as $ty } {
                    return Err(i8::try_from(i128::MAX).unwrap_err())
                }

                Ok(unsafe { Self::new_unchecked(value as i64) })
            }
        })+
    };
}


macro_rules! lossy_unsigned_from {
    ($($ty: ty),+ $(,)?) => {
        $(impl TryFrom<$ty> for i48 {
            type Error = <u8 as TryFrom<u128>>::Error;

            #[inline(always)]
            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                const { assert!(<$ty>::BITS > i48::BITS && <$ty>::MIN == 0) }

                if value > const { MAX as $ty } {
                    return Err(u8::try_from(u128::MAX).unwrap_err())
                }

                Ok(unsafe { Self::from_bits_unchecked(value as u64) })
            }
        })+
    };
}

lossy_signed_from! { i64, i128, isize }
lossy_unsigned_from! { u64, u128, usize }

macro_rules! impl_bin_op {
    (
        $trait: ident; fn $name: ident;
        $assign_trait: ident; fn $assign_name: ident;
        $checked: ident,
        $wrapping: ident
    ) => {
impl $trait for i48 {
    type Output = Self;

    #[inline(always)]
    fn $name(self, rhs: Self) -> Self::Output {
        match cfg!(debug_assertions) {
            true => self.$checked(rhs).expect(concat!("overflow on ", stringify!($name))),
            false => self.$wrapping(rhs)
        }
    }
}

impl $trait<&i48> for i48 {
    type Output = i48;

    #[inline(always)]
    fn $name(self, &rhs: &i48) -> Self::Output {
        $trait::$name(self, rhs)
    }
}

impl $assign_trait for i48 {
    #[inline(always)]
    fn $assign_name(&mut self, rhs: Self) {
        *self = $trait::$name(*self, rhs)
    }
}

impl $assign_trait<&i48> for i48 {
    #[inline(always)]
    fn $assign_name(&mut self, rhs: &i48) {
        $assign_trait::$assign_name(self, *rhs)
    }
}

    };
}

impl_bin_op! {
    Add; fn add;
    AddAssign; fn add_assign;
    checked_add,
    wrapping_add
}


impl_bin_op! {
    Sub; fn sub;
    SubAssign; fn sub_assign;
    checked_sub,
    wrapping_sub
}


impl_bin_op! {
    Mul; fn mul;
    MulAssign; fn mul_assign;
    checked_mul,
    wrapping_mul
}

impl_bin_op! {
    Div; fn div;
    DivAssign; fn div_assign;
    checked_div,
    wrapping_div
}


impl_bin_op! {
    Rem; fn rem;
    RemAssign; fn rem_assign;
    checked_rem,
    wrapping_rem
}


macro_rules! impl_fmt {
    ($trait:path) => {
        impl $trait for i48 {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                <i64 as $trait>::fmt(&self.as_i64(), f)
            }
        }
    };
}

impl_fmt!(Debug);
impl_fmt!(Display);
impl_fmt!(Binary);
impl_fmt!(Octal);
impl_fmt!(LowerHex);
impl_fmt!(UpperHex);

impl Not for i48 {
    type Output = i48;

    fn not(self) -> Self::Output {
        Self::from_bits_wrapping(!self.to_bits())
    }
}

impl Neg for i48 {
    type Output = i48;

    fn neg(self) -> Self::Output {
        match cfg!(debug_assertions) {
            true => self.checked_neg().expect("overflow on negate"),
            false => self.wrapping_neg()
        }
    }
}

impl PartialEq for i48 {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }


    #[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        self.to_bits() != other.to_bits()
    }
}

impl Eq for i48 {}

impl PartialOrd for i48 {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    #[inline(always)]
    fn lt(&self, other: &Self) -> bool {
        self.as_i64() < other.as_i64()
    }

    #[inline(always)]
    fn le(&self, other: &Self) -> bool {
        self.as_i64() <= other.as_i64()
    }

    #[inline(always)]
    fn gt(&self, other: &Self) -> bool {
        self.as_i64() > other.as_i64()
    }

    #[inline(always)]
    fn ge(&self, other: &Self) -> bool {
        self.as_i64() >= other.as_i64()
    }
}

impl Ord for i48 {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_i64().cmp(&other.as_i64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;
    use std::mem::{size_of, align_of};
    use proptest::proptest;

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(size_of::<i48>(), size_of::<i64>());
        assert_eq!(align_of::<i48>(), align_of::<i64>());
    }

    #[test]
    fn test_constants() {
        assert_eq!(i48::BITS, 48);
        assert_eq!(i48::MAX.as_i64(), 0x7FFF_FFFF_FFFF);
        assert_eq!(i48::MIN.as_i64(), -0x8000_0000_0000);
    }

    #[test]
    fn test_new_valid() {
        assert_eq!(i48::new(0).unwrap().as_i64(), 0);
        assert_eq!(i48::new(100).unwrap().as_i64(), 100);
        assert_eq!(i48::new(-100).unwrap().as_i64(), -100);
        assert_eq!(i48::new(i48::MAX.as_i64()).unwrap().as_i64(), i48::MAX.as_i64());
        assert_eq!(i48::new(i48::MIN.as_i64()).unwrap().as_i64(), i48::MIN.as_i64());
    }

    #[test]
    fn test_new_invalid() {
        assert!(i48::new(i48::MAX.as_i64() + 1).is_none());
        assert!(i48::new(i48::MIN.as_i64() - 1).is_none());
    }

    #[test]
    fn test_new_wrapping() {
        let max_plus_one = i48::new_wrapping(i48::MAX.as_i64() + 1);
        let min_minus_one = i48::new_wrapping(i48::MIN.as_i64() - 1);

        // When MAX + 1 wraps, it should become MIN
        assert_eq!(max_plus_one.as_i64(), i48::MIN.as_i64());

        // When MIN - 1 wraps, it should become MAX
        assert_eq!(min_minus_one.as_i64(), i48::MAX.as_i64());
    }

    #[test]
    fn test_from_bits() {
        assert_eq!(i48::from_bits(0).unwrap().as_i64(), 0);
        assert_eq!(i48::from_bits(100).unwrap().as_i64(), 100);

        // Test max positive value
        assert_eq!(i48::from_bits(0x7FFF_FFFF_FFFF).unwrap().as_i64(), 0x7FFF_FFFF_FFFF);

        // Test negative numbers (two's complement)
        assert_eq!(i48::from_bits(0xFFFF_FFFF_FFFF).unwrap().as_i64(), -1);
        assert_eq!(i48::from_bits(0x8000_0000_0000).unwrap().as_i64(), i48::MIN.as_i64());

        // Test out of bounds
        assert!(i48::from_bits(0x1_0000_0000_0000).is_none());
    }

    #[test]
    fn test_to_bits() {
        assert_eq!(i48::new(0).unwrap().to_bits(), 0);
        assert_eq!(i48::new(100).unwrap().to_bits(), 100);
        assert_eq!(i48::new(-1).unwrap().to_bits(), 0xFFFF_FFFF_FFFF);
        assert_eq!(i48::MAX.to_bits(), 0x7FFF_FFFF_FFFF);
        assert_eq!(i48::MIN.to_bits(), 0x8000_0000_0000);
    }

    #[test]
    fn test_checked_operations() {
        // Addition
        assert_eq!(i48::new(5).unwrap().checked_add(i48::new(10).unwrap()).unwrap().as_i64(), 15);
        assert!(i48::MAX.checked_add(i48::new(1).unwrap()).is_none());

        // Subtraction
        assert_eq!(i48::new(15).unwrap().checked_sub(i48::new(10).unwrap()).unwrap().as_i64(), 5);
        assert!(i48::MIN.checked_sub(i48::new(1).unwrap()).is_none());

        // Multiplication
        assert_eq!(i48::new(5).unwrap().checked_mul(i48::new(10).unwrap()).unwrap().as_i64(), 50);
        assert!(i48::MAX.checked_mul(i48::new(2).unwrap()).is_none());

        // Division
        assert_eq!(i48::new(50).unwrap().checked_div(i48::new(10).unwrap()).unwrap().as_i64(), 5);
        assert!(i48::new(10).unwrap().checked_div(i48::new(0).unwrap()).is_none());

        // Division Euclid
        assert_eq!(i48::new(50).unwrap().checked_div_euclid(i48::new(10).unwrap()).unwrap().as_i64(), 5);
        assert!(i48::new(10).unwrap().checked_div_euclid(i48::new(0).unwrap()).is_none());

        // Remainder
        assert_eq!(i48::new(13).unwrap().checked_rem(i48::new(5).unwrap()).unwrap().as_i64(), 3);
        assert!(i48::new(10).unwrap().checked_rem(i48::new(0).unwrap()).is_none());

        // Remainder Euclid
        assert_eq!(i48::new(13).unwrap().checked_rem_euclid(i48::new(5).unwrap()).unwrap().as_i64(), 3);
        assert!(i48::new(10).unwrap().checked_rem_euclid(i48::new(0).unwrap()).is_none());
    }

    #[test]
    fn test_checked_unary_operations() {
        // Absolute value
        assert_eq!(i48::new(-5).unwrap().checked_abs().unwrap().as_i64(), 5);
        assert!(i48::MIN.checked_abs().is_none()); // MIN absolute value overflows

        // Negation
        assert_eq!(i48::new(5).unwrap().checked_neg().unwrap().as_i64(), -5);
        assert!(i48::MIN.checked_neg().is_none()); // Negating MIN overflows

        // Integer square root
        assert_eq!(i48::new(16).unwrap().checked_isqrt().unwrap().as_i64(), 4);
        assert!(i48::new(-1).unwrap().checked_isqrt().is_none()); // Can't sqrt negative
    }

    #[test]
    fn test_from_smaller_types() {
        // Test lossless conversions
        assert_eq!(i48::from(5i8).as_i64(), 5);
        assert_eq!(i48::from(-5i8).as_i64(), -5);
        assert_eq!(i48::from(30000i16).as_i64(), 30000);
        assert_eq!(i48::from(-30000i16).as_i64(), -30000);
        assert_eq!(i48::from(2000000000i32).as_i64(), 2000000000);
        assert_eq!(i48::from(-2000000000i32).as_i64(), -2000000000);

        // Unsigned
        assert_eq!(i48::from(5u8).as_i64(), 5);
        assert_eq!(i48::from(30000u16).as_i64(), 30000);
        assert_eq!(i48::from(2000000000u32).as_i64(), 2000000000);
    }

    #[test]
    fn test_try_from_larger_types() {
        // Valid conversions
        assert_eq!(i48::try_from(5i64).unwrap().as_i64(), 5);
        assert_eq!(i48::try_from(-5i64).unwrap().as_i64(), -5);
        assert_eq!(i48::try_from(5i128).unwrap().as_i64(), 5);
        assert_eq!(i48::try_from(-5i128).unwrap().as_i64(), -5);

        // Unsigned
        assert_eq!(i48::try_from(5u64).unwrap().as_i64(), 5);
        assert_eq!(i48::try_from(5u128).unwrap().as_i64(), 5);

        // Out of bounds
        assert!(i48::try_from(i48::MAX.as_i64() + 1).is_err());
        assert!(i48::try_from(i48::MIN.as_i64() - 1).is_err());
        assert!(i48::try_from(u64::MAX).is_err());
    }

    #[test]
    fn test_binary_operators() {
        let a = i48::new(10).unwrap();
        let b = i48::new(5).unwrap();

        // Add
        assert_eq!((a + b).as_i64(), 15);

        // Sub
        assert_eq!((a - b).as_i64(), 5);

        // Mul
        assert_eq!((a * b).as_i64(), 50);

        // Div
        assert_eq!((a / b).as_i64(), 2);

        // Rem
        assert_eq!((a % b).as_i64(), 0);

        // Test with reference on right side
        let b_ref = &b;
        assert_eq!((a + b_ref).as_i64(), 15);
    }

    proptest! {
        #[test]
        fn test_i48_new(a in MIN..=MAX) {
            assert_eq!(i48::new(a).unwrap().as_i64(), a)
        }
    }

    #[test]
    fn test_compound_assignment() {
        let mut a = i48::new(10).unwrap();
        let b = i48::new(5).unwrap();

        // AddAssign
        a += b;
        assert_eq!(a.as_i64(), 15);

        // SubAssign
        a -= b;
        assert_eq!(a.as_i64(), 10);

        // MulAssign
        a *= b;
        assert_eq!(a.as_i64(), 50);

        // DivAssign
        a /= b;
        assert_eq!(a.as_i64(), 10);

        // RemAssign
        a %= b;
        assert_eq!(a.as_i64(), 0);

        // Test with reference on right side
        let mut c = i48::new(10).unwrap();
        let b_ref = &b;
        c += b_ref;
        assert_eq!(c.as_i64(), 15);
    }

    #[test]
    fn test_eq_and_ord() {
        let a = i48::new(10).unwrap();
        let b = i48::new(10).unwrap();
        let c = i48::new(20).unwrap();
        let d = i48::new(-10).unwrap();

        // Equality
        assert_eq!(a, b);
        assert_ne!(a, c);

        // Ordering
        assert!(a < c);
        assert!(a <= b);
        assert!(c > a);
        assert!(c >= a);
        assert!(d < a);

        // Test cmp
        assert_eq!(a.cmp(&b), Ordering::Equal);
        assert_eq!(a.cmp(&c), Ordering::Less);
        assert_eq!(c.cmp(&a), Ordering::Greater);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(i48::new(10).unwrap());
        set.insert(i48::new(20).unwrap());

        assert!(set.contains(&i48::new(10).unwrap()));
        assert!(set.contains(&i48::new(20).unwrap()));
        assert!(!set.contains(&i48::new(30).unwrap()));
    }

    #[test]
    fn test_debug_and_display() {
        let a = i48::new(42).unwrap();

        assert_eq!(format!("{:?}", a), "42");
        assert_eq!(format!("{}", a), "42");

        let b = i48::new(-42).unwrap();
        assert_eq!(format!("{:?}", b), "-42");
        assert_eq!(format!("{}", b), "-42");
    }

    #[test]
    fn test_i48_macro() {
        const A: i48 = i48!(42);
        assert_eq!(A.as_i64(), 42);

        const B: i48 = i48!(-42);
        assert_eq!(B.as_i64(), -42);
    }
}