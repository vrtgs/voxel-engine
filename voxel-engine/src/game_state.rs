use std::cell::Cell;
use std::time::Instant;
use glam::{vec3, Quat, Vec2, Vec3};
use crate::controls::{Controls, InputMethod, KeyMapping};

pub enum Shape {
    Pentagon,
    Trapezoid
}

#[derive(Copy, Clone, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub facing: Quat,
}

impl Transform {
    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: vec3(x, y, z),
            facing: Quat::IDENTITY
        }
    }

    pub fn local_z(&self) -> Vec3 {
        self.facing * Vec3::Z
    }

    pub fn local_x(&self) -> Vec3 {
        self.facing * Vec3::X
    }

    pub fn forwards(&self) -> Vec3 {
        -self.local_z()
    }

    pub fn right(&self) -> Vec3 {
        self.local_x()
    }

    pub fn rotate(&mut self, rotation: Quat) {
        self.facing = rotation * self.facing
    }
}

pub struct GameState {
    background_color: wgpu::Color,
    shape: Shape,
    player_transform: Transform,
}

pub enum UpdateResult {
    Rerender,
    Continue,
}



impl GameState {
    pub fn new() -> Self {
        Self {
            background_color: wgpu::Color::BLACK,
            shape: Shape::Pentagon,
            player_transform: Transform::from_xyz(0.0, 1.0, 2.0)
        }
    }

    pub fn background_color(&self) -> wgpu::Color {
        self.background_color
    }

    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    pub fn transform(&self) -> Transform {
        self.player_transform
    }

    fn run_player_movement(&mut self, controls: &Controls) {
        thread_local! {
            static LAST: Cell<Instant> = Cell::new(Instant::now());
        }

        let now = Instant::now();
        let delta_frame = (now - LAST.replace(now)).as_secs_f32();


        let delta_mouse = controls.cursor_delta();

        if delta_mouse != Vec2::ZERO {
            let sensitivity = 0.1;
            let yaw = -delta_mouse.x * sensitivity * delta_frame;
            let pitch = -delta_mouse.y * sensitivity * delta_frame;

            let rotation = Quat::from_axis_angle(Vec3::Y, yaw)
                * Quat::from_axis_angle(self.player_transform.local_x(), pitch);

            self.player_transform.rotate(rotation);
        }

        let mut speed = 2.0_f32.sqrt().exp();

        if controls.held_down(KeyMapping::Sneak) {
            speed /= 2.0
        }

        if controls.held_down(KeyMapping::Sprint) {
            speed *= 2.0
        }

        let mut delta = Vec3::ZERO;
        let forward = self.player_transform.forwards();
        let right = self.player_transform.right();

        if controls.held_down(KeyMapping::WalkForwards) {
            delta += forward
        }

        if controls.held_down(KeyMapping::WalkBackwards) {
            delta -= forward
        }

        if controls.held_down(KeyMapping::WalkRight) {
            delta += right
        }

        if controls.held_down(KeyMapping::WalkLeft) {
            delta -= right
        }

        self.player_transform.position += delta.normalize_or_zero() * speed * delta_frame;

        if controls.triggered(KeyMapping::Jump) {
            self.shape = match self.shape {
                Shape::Trapezoid => Shape::Pentagon,
                Shape::Pentagon => Shape::Trapezoid
            }
        }
    }

    /// # Returns
    /// true if the event was `consumed`
    /// false otherwise
    pub fn frame_update(&mut self, controls: &Controls) {
        self.run_player_movement(controls)
    }
}