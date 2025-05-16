use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};
use bytemuck::Zeroable;
use glam::{Quat, Vec3, Vec3A};
use crate::fixed_point::{FixedPoint, Fract};
use crate::i48_int::i48;

pub mod i48_int;
pub mod fixed_point;


#[derive(Copy, Clone, PartialEq)]
pub struct Transform {
    // storing the Quaternion means were paying for the padding,
    // since we already pay for padding might as well use it for speed
    pub position: Vec3A,
    pub rotation: Quat,
}



#[derive(Copy, Clone, Hash, PartialEq, Eq, Zeroable, Debug)]
#[repr(C)]
pub struct I48Vec3 {
    pub x: i48,
    pub y: i48,
    pub z: i48
}

#[derive(Copy, Clone, PartialEq, Zeroable, Debug)]
#[repr(C)]
pub struct FractVec3 {
    pub x: Fract,
    pub y: Fract,
    pub z: Fract
}

impl FractVec3 {
    pub const ZERO: Self = Self {
        x: Fract::ZERO,
        y: Fract::ZERO,
        z: Fract::ZERO
    };
}


#[derive(Copy, Clone, PartialEq, Zeroable, Debug)]
#[repr(C)]
pub struct FixedPointVec3 {
    pub x: FixedPoint,
    pub y: FixedPoint,
    pub z: FixedPoint
}

impl FixedPointVec3 {
    pub const Y: Self = Self {
        x: FixedPoint::ZERO,
        y: FixedPoint::from_int(i48!(1)),
        z: FixedPoint::ZERO
    };
    
    pub const ZERO: Self = Self {
        x: FixedPoint::ZERO,
        y: FixedPoint::ZERO,
        z: FixedPoint::ZERO
    };
    
    pub const fn new(x: FixedPoint, y: FixedPoint, z: FixedPoint) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn from_f32(vec: Vec3) -> Self {
        Self::new(
            FixedPoint::from_f32(vec.x),
            FixedPoint::from_f32(vec.y),
            FixedPoint::from_f32(vec.z)
        )
    }

    #[inline]
    pub fn from_f32a(vec: Vec3A) -> Self {
        let [x, y, z] = <[f32; 3]>::from(vec).map(FixedPoint::from_f32);
        Self::new(x, y, z)
    }
    
    #[inline]
    pub const fn as_f32(self) -> Vec3 {
        Vec3::new(
            self.x.as_f32(),
            self.y.as_f32(),
            self.z.as_f32()
        )
    }
    
    // FIXME terrible impl ik
    pub fn normalize_or_zero(self) -> Self {
        Self::from_f32(self.as_f32().normalize_or_zero())
    }
}


impl Mul<FixedPoint> for FixedPointVec3 {
    type Output = FixedPointVec3;

    fn mul(self, rhs: FixedPoint) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
        }
    }
}

impl Add for FixedPointVec3 {
    type Output = FixedPointVec3;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl AddAssign for FixedPointVec3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = (*self) + rhs
    }
}


impl Sub for FixedPointVec3 {
    type Output = FixedPointVec3;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl SubAssign for FixedPointVec3 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = (*self) - rhs
    }
}
