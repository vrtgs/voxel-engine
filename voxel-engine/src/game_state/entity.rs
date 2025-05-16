use glam::vec3;
use voxel_maths::FixedPointVec3;
use crate::game_state::coords::AbsoluteCoord;

#[derive(Copy, Clone)]
pub struct Camera {
    /// measured in radians
    pub yaw: f32,
    /// measured in radians
    pub pitch: f32
}

pub struct Player {
    pub(super) camera: Camera,
    pub(super) position: AbsoluteCoord,
}

pub trait Entity {
    fn camera(&self) -> Camera;

    fn position(&self) -> AbsoluteCoord;

    fn eye(&self) -> AbsoluteCoord {
        self.position()
    }

    fn camera_direction(&self) -> FixedPointVec3 {
        let camera = self.camera();

        let (sin_yaw, cos_yaw) = camera.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = camera.pitch.sin_cos();

        let x = cos_pitch * cos_yaw;
        let y = sin_pitch;
        let z = cos_pitch * sin_yaw;

        FixedPointVec3::from_f32(vec3(x, y, z))
    }

    // visualization of axis
    // https://sotrh.github.io/learn-wgpu/assets/img/left_right_hand.ccabf5d0.gif

    fn forwards(&self) -> FixedPointVec3 {
        let (yaw_sin, yaw_cos) = self.camera().yaw.sin_cos();
        let forward = vec3(yaw_cos, 0.0, yaw_sin).normalize();
        FixedPointVec3::from_f32(forward)
    }

    fn right(&self) -> FixedPointVec3 {
        let (yaw_sin, yaw_cos) = self.camera().yaw.sin_cos();
        let right = vec3(-yaw_sin, 0.0, yaw_cos).normalize();
        FixedPointVec3::from_f32(right)
    }
}

impl Entity for Player {
    fn camera(&self) -> Camera {
        self.camera
    }

    fn position(&self) -> AbsoluteCoord {
        self.position
    }
}