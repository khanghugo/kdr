use bitflags::bitflags;
use cgmath::Deg;

use crate::app::constants::{CAM_SPEED, CAM_TURN};

use super::*;

#[derive(Debug, Clone, Copy)]
pub struct Key(u32);

bitflags! {
    impl Key: u32 {
        const Forward   = (1 << 0);
        const Back      = (1 << 1);
        const MoveLeft  = (1 << 2);
        const MoveRight = (1 << 3);
        const Left      = (1 << 4);
        const Right     = (1 << 5);
        const Up        = (1 << 6);
        const Down      = (1 << 7);
        const Shift     = (1 << 8);
        const Control   = (1 << 9);
    }
}

impl AppState {
    fn forward(&mut self) {
        self.render_state
            .camera
            .move_along_view(self.get_move_displacement());
    }

    fn back(&mut self) {
        self.render_state
            .camera
            .move_along_view(-self.get_move_displacement());
    }

    fn moveleft(&mut self) {
        self.render_state
            .camera
            .move_along_view_orthogonal(-self.get_move_displacement());
    }

    fn moveright(&mut self) {
        self.render_state
            .camera
            .move_along_view_orthogonal(self.get_move_displacement());
    }

    fn up(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_pitch(self.get_camera_displacement());
    }

    fn down(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_pitch(-self.get_camera_displacement());
    }

    fn left(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_yaw(self.get_camera_displacement());
    }

    fn right(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_yaw(-self.get_camera_displacement());
    }

    fn get_move_displacement(&self) -> f32 {
        CAM_SPEED * self.frame_time * self.get_multiplier()
    }

    fn get_camera_displacement(&self) -> Deg<f32> {
        Deg(CAM_TURN * self.frame_time) * self.get_multiplier()
    }

    fn get_multiplier(&self) -> f32 {
        if self.keys.contains(Key::Shift) {
            2.0
        } else if self.keys.contains(Key::Control) {
            0.5
        } else {
            1.0
        }
    }

    pub fn interaction_tick(&mut self) {
        if self.keys.contains(Key::Forward) {
            self.forward();
        }
        if self.keys.contains(Key::Back) {
            self.back();
        }
        if self.keys.contains(Key::MoveLeft) {
            self.moveleft();
        }
        if self.keys.contains(Key::MoveRight) {
            self.moveright();
        }
        if self.keys.contains(Key::Left) {
            self.left();
        }
        if self.keys.contains(Key::Right) {
            self.right();
        }
        if self.keys.contains(Key::Up) {
            self.up();
        }
        if self.keys.contains(Key::Down) {
            self.down();
        }
    }
}
