use cgmath::Deg;

use crate::app::constants::CAM_TURN;

use super::{input::Key, *};

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
        self.input_state.noclip_speed * self.frame_time * self.get_multiplier()
    }

    fn get_camera_displacement(&self) -> Deg<f32> {
        Deg(CAM_TURN * self.frame_time) * self.get_multiplier()
    }

    fn get_multiplier(&self) -> f32 {
        if self.input_state.keys.contains(Key::Shift) {
            2.0
        } else if (Key::Control | Key::Alt).intersects(self.input_state.keys) {
            0.5
        } else {
            1.0
        }
    }

    pub fn interaction_tick(&mut self) {
        // shouldnt be able to move if we are not in free cam
        if !self.input_state.free_cam {
            return;
        }

        if self.input_state.keys.contains(Key::Forward) {
            self.forward();
        }
        if self.input_state.keys.contains(Key::Back) {
            self.back();
        }
        if self.input_state.keys.contains(Key::MoveLeft) {
            self.moveleft();
        }
        if self.input_state.keys.contains(Key::MoveRight) {
            self.moveright();
        }
        if self.input_state.keys.contains(Key::Left) {
            self.left();
        }
        if self.input_state.keys.contains(Key::Right) {
            self.right();
        }
        if self.input_state.keys.contains(Key::Up) {
            self.up();
        }
        if self.input_state.keys.contains(Key::Down) {
            self.down();
        }
    }

    pub fn handle_mouse_movement(&mut self, (x, y): (f64, f64)) {
        if !self.input_state.free_cam {
            return;
        }

        // behave like bspguy
        if !self.input_state.mouse_right_hold {
            return;
        }

        self.render_state.camera.set_pitch(
            self.render_state.camera.pitch() + Deg(-y as f32 * self.input_state.sensitivity),
        );
        self.render_state.camera.set_yaw(
            self.render_state.camera.yaw() + Deg(-x as f32 * self.input_state.sensitivity),
        );

        self.render_state.camera.rebuild_orientation();
    }
}
