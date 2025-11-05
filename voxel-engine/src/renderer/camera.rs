use glam::{Mat4, Vec3};
use crate::game_state::entity::Entity;
use crate::settings::Fov;

pub struct Camera<'a>(&'a dyn Entity);

impl<'a> Camera<'a> {
    pub fn new(entity: &'a dyn Entity) -> Self {
        Self(entity)
    }

    pub fn eye(&self) -> Vec3 {
        self.0.eye().xyz().as_f32()
    }
    
    pub fn calc_matrix(&self) -> Mat4 {
        let entity = self.0;
        let direction = entity.camera_direction().as_f32();
        let eye = entity.eye().xyz().as_f32();

        Mat4::look_to_rh(
            eye,
            direction,
            Vec3::Y
        )
    }
}

pub struct Projection {
    aspect: f32,
    fov: f32,
}

impl Projection {
    pub fn new(width: u32, height: u32, fov: Fov) -> Self {
        Self {
            aspect: (width as f64 / height as f64) as f32,
            fov: (fov.get_degrees() as f32).to_radians()
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn change_fov(&mut self, fov: Fov) {
        self.fov = (fov.get_degrees() as f32).to_radians()
    }

    pub fn calc_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(
            self.fov,
            self.aspect,
            0.1,
            100.0
        )
    }

}