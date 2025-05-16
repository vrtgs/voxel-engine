use std::ops::{Add, AddAssign};
use glam::{u8vec3, U8Vec3};
use voxel_maths::fixed_point::FixedPoint;
use voxel_maths::{i48, FixedPointVec3}; 
use voxel_maths::i48_int::i48;


#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[repr(C, align(8))]
pub struct ChunkCoord {
    x: i32,
    z: i32
}

impl ChunkCoord {
    pub const ZERO: Self = Self::from_xz(0, 0);
    
    pub const fn from_xz(x: i32, z: i32) -> Self {
        Self { x, z }
    }
    
    #[inline(always)]
    pub fn x(&self) -> i48 {
        i48::from(self.x) * i48!(16)
    }
    
    pub fn z(&self) -> i48 {
        i48::from(self.z) * i48!(16)
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[repr(transparent)]
pub struct ChunkRelativeXZ {
    // x in 4 msb
    // z in 4 lsb
    xz: u8,
}

impl ChunkRelativeXZ {
    pub const ZERO: Self = Self::from_xz(0, 0);

    pub const fn from_xz(x: u8, z: u8) -> Self {
        debug_assert!(x < 16 && z < 16);
        let xz = (x << 4) | z;
        Self { xz }
    }

    #[inline(always)]
    pub fn x(self) -> u8 {
        self.xz >> 4
    }

    #[inline(always)]
    pub fn z(self) -> u8 {
        self.xz & ((1 << 4) - 1)
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[repr(C, align(2))]
pub struct BlockCoord {
    xz: ChunkRelativeXZ,
    y: u8
}

impl BlockCoord {
    pub const ZERO: Self = Self::from_xyz(0, 0, 0);

    #[inline(always)]
    pub const fn from_xyz(x: u8, y: u8, z: u8) -> Self {
        Self {
            xz: ChunkRelativeXZ::from_xz(x, z),
            y
        }
    }
    
    #[inline(always)]
    pub fn x(self) -> u8 {
        self.xz.x()
    }

    #[inline(always)]
    pub fn z(self) -> u8 {
        self.xz.z()
    }

    #[inline(always)]
    pub fn y(self) -> u8 {
        self.y
    }

    #[inline(always)]
    pub fn xyz(self) -> U8Vec3 {
        u8vec3(
            self.x(),
            self.y(),
            self.z()
        )
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq)]
pub struct AbsoluteBlockCoord {
    chunk: ChunkCoord,
    block_coord: BlockCoord
}

impl AbsoluteBlockCoord {
    pub const ZERO: Self = Self::from_xyz(i48!(0), 0, i48!(0));
    
    pub const fn from_xyz(x: i48, y: u8, z: i48) -> Self {
        const { assert!(size_of::<Self>() < size_of::<(i48, u8, i48)>()) }
        
        const fn separate(coord: i48) -> (i32, u8) {
            let num = coord.as_i64();
            let (chunk, rel) = (num.div_euclid(16), num.rem_euclid(16) as u8);
            
            if chunk > i32::MAX as i64 {
                return (i32::MAX, 15)
            }
            
            if chunk < i32::MIN as i64 { 
                return (i32::MIN, 0)
            }

            (chunk as i32, rel)
        }
        
        let (x, x_rest) = separate(x);
        let (z, z_rest) = separate(z);
        
        Self {
            chunk: ChunkCoord::from_xz(x, z),
            block_coord: BlockCoord::from_xyz(x_rest, y, z_rest)
        }
    }

    #[inline(always)]
    pub fn x(&self) -> i48 {
        self.chunk.x() + i48::from(self.block_coord.x())
    }

    #[inline(always)]
    pub fn z(&self) -> i48 {
        self.chunk.z() + i48::from(self.block_coord.z())
    }

    #[inline(always)]
    pub fn y(&self) -> u8 {
        self.block_coord.y
    }

    #[inline(always)]
    pub fn xyz(&self) -> (i48, u8, i48) {
        (self.x(), self.y(), self.z())
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct AbsoluteCoord {
    x: FixedPoint,
    y: FixedPoint,
    z: FixedPoint
}

impl AbsoluteCoord {
    pub const ZERO: Self = Self::from_xyz(FixedPoint::ZERO, FixedPoint::ZERO, FixedPoint::ZERO);
    
    #[inline(always)]
    pub const fn from_xyz_vec(xyz: FixedPointVec3) -> Self {
        Self::from_xyz(xyz.x, xyz.y, xyz.z)
    }

    #[inline(always)]
    pub const fn from_xyz(x: FixedPoint, y: FixedPoint, z: FixedPoint) -> Self {
        Self {
            y,
            x,
            z
        }
    }
    
    #[inline(always)]
    pub fn x(&self) -> FixedPoint {
        self.x
    }

    #[inline(always)]
    pub fn z(&self) -> FixedPoint {
        self.z
    }

    #[inline(always)]
    pub fn y(&self) -> FixedPoint {
        self.y
    }

    #[inline(always)]
    pub fn xyz(self) -> FixedPointVec3 {
        FixedPointVec3 {
            x: self.x(),
            y: self.y(),
            z: self.z()
        }
    }
}

impl Add for AbsoluteCoord {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut vec = self.xyz();
        vec.x += rhs.x();
        vec.y += rhs.y();
        vec.z += rhs.z();
        
        Self::from_xyz(vec.x, vec.y, vec.z)
    }
}

impl AddAssign for AbsoluteCoord {
    fn add_assign(&mut self, rhs: Self) {
        *self = (*self) + rhs
    }
}