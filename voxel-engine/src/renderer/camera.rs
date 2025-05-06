use glam::{Mat4, Vec3};
use crate::game_state::Transform;
use crate::settings::Fov;

pub struct Camera {
    pub(super) eye: Vec3,
    pub(super) target: Vec3,
    pub(super) aspect: f32,
    pub(super) fov: Fov,
}


impl Camera {
    pub fn new(eye: Vec3, target: Vec3, display_size: winit::dpi::PhysicalSize<u32>, fov: Fov) -> Self {
        Self {
            eye,
            target,
            aspect: display_size.width as f32 / display_size.height as f32,
            fov
        }
    }

    pub fn update_from_player(&mut self, player_position: Transform) {
        let pos = player_position.position;
        let forwards = player_position.forwards();
        self.eye = pos;
        self.target = pos + forwards;
    }

    pub fn build_view_projection_matrix(&self) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye, self.target, Vec3::Y);

        let projection = Mat4::perspective_rh(
            (self.fov.get() as f32).to_radians(),
            self.aspect,
            0.1,
            100.0
        );

        projection *  view
    }
}