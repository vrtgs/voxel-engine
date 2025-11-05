use std::cell::Cell;
use std::time::Instant;
use glam::Vec2;
use voxel_maths::fixed_point::FixedPoint;
use voxel_maths::FixedPointVec3;
use crate::controls::{Controls, InputMethod, KeyMapping};
use crate::game_state::coords::AbsoluteCoord;
use crate::game_state::entity::{Camera, Entity, Player};

pub mod entity;

pub mod coords;

pub struct GameState {
    player: Player,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            player: Player {
                camera: Camera {
                    yaw: 0.0,
                    pitch: 0.0,
                },
                position: AbsoluteCoord::ZERO
            } 
        }
    }
    
    pub fn player(&self) -> &Player {
        &self.player
    }

    fn run_player_movement(&mut self, controls: &Controls) {
        thread_local! {
            static LAST: Cell<Instant> = Cell::new(Instant::now());
        }

        let now = Instant::now();
        let delta_frame = (now - LAST.replace(now)).as_secs_f32();

        let delta_mouse = controls.cursor_delta();
        
        const MAX_YAW_DIF: f32 = std::f32::consts::FRAC_2_PI - (0.1_f32.to_radians());
        const MAX_PITCH: f32 = MAX_YAW_DIF;
        const MIN_PITCH: f32 = -MAX_YAW_DIF;
        
        if delta_mouse != Vec2::ZERO {
            let sensitivity = 0.15;
            let yaw = delta_mouse.x * sensitivity * delta_frame;
            let pitch = -delta_mouse.y * sensitivity * delta_frame;
        
            let camera = &mut self.player.camera;
            camera.yaw = (camera.yaw + yaw).rem_euclid(const { 2.0 * std::f32::consts::PI });
            camera.pitch = (camera.pitch + pitch).clamp(MIN_PITCH, MAX_PITCH);
        }

        // FIXME not actually fixed point
        let delta_frame = FixedPoint::from_f32(delta_frame);

        let mut delta = FixedPointVec3::ZERO;
        
        // this float is fine, its in a very fine grained and rigid range
        let mut speed = 2.0_f32.exp();
        
        if controls.held_down(KeyMapping::Sprint) {
            speed *= 2.0
        }
        
        if controls.held_down(KeyMapping::Jump) {
            delta += FixedPointVec3::Y
        }

        if controls.held_down(KeyMapping::Sneak) {
            speed /= 2.0;
            delta -= FixedPointVec3::Y
        }

        let speed = FixedPoint::from_f32(speed);
        
        let forward = self.player.forwards();
        let right = self.player.right();
        
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

        
        let pos_delta = delta.normalize_or_zero() * speed * delta_frame;
        self.player.position += AbsoluteCoord::from_xyz_vec(pos_delta);

        
        if controls.triggered(KeyMapping::MainMenu) {
            self.player.position = AbsoluteCoord::ZERO
        }
    }

    /// # Returns
    /// true if the event was `consumed`
    /// false otherwise
    pub fn frame_update(&mut self, controls: &Controls) {
        self.run_player_movement(controls)
    }
}