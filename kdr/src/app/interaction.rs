use bitflags::bitflags;
use cgmath::Deg;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
    window::CursorGrabMode,
};

use super::{
    App,
    constants::{CAM_SPEED, CAM_TURN, SENSITIVITY},
};

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

impl App {
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

    pub fn handle_keyboard_input(&mut self, physical_key: PhysicalKey, state: ElementState) {
        match physical_key {
            winit::keyboard::PhysicalKey::Code(key_code) => match key_code {
                KeyCode::KeyW => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Forward);
                    } else {
                        self.keys = self.keys.intersection(Key::Forward.complement());
                    }
                }
                KeyCode::KeyS => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Back);
                    } else {
                        self.keys = self.keys.intersection(Key::Back.complement());
                    }
                }
                KeyCode::KeyA => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::MoveLeft);
                    } else {
                        self.keys = self.keys.intersection(Key::MoveLeft.complement());
                    }
                }
                KeyCode::KeyD => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::MoveRight);
                    } else {
                        self.keys = self.keys.intersection(Key::MoveRight.complement());
                    }
                }
                KeyCode::ArrowLeft => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Left);
                    } else {
                        self.keys = self.keys.intersection(Key::Left.complement());
                    }
                }
                KeyCode::ArrowRight => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Right);
                    } else {
                        self.keys = self.keys.intersection(Key::Right.complement());
                    }
                }
                KeyCode::ArrowUp => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Up);
                    } else {
                        self.keys = self.keys.intersection(Key::Up.complement());
                    }
                }
                KeyCode::ArrowDown => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Down);
                    } else {
                        self.keys = self.keys.intersection(Key::Down.complement());
                    }
                }
                KeyCode::ShiftLeft => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Shift);
                    } else {
                        self.keys = self.keys.intersection(Key::Shift.complement());
                    }
                }
                KeyCode::ControlLeft => {
                    if state.is_pressed() {
                        self.keys = self.keys.union(Key::Control);
                    } else {
                        self.keys = self.keys.intersection(Key::Control.complement());
                    }
                }
                KeyCode::KeyQ => {
                    if state.is_pressed() {
                        panic!()
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }

    pub fn handle_mouse_input(&mut self, mouse_button: MouseButton, state: ElementState) {
        match mouse_button {
            MouseButton::Right => self.mouse_right_hold = state.is_pressed(),
            _ => (),
        }

        self.window.as_mut().map(|window| {
            if self.mouse_right_hold {
                // behave like bspguy
                window.set_cursor_visible(false);

                // lock mode is better than confined
                // lock mode doesnt allow cursor to move
                // confined clamps the position
                let _ = window.set_cursor_grab(CursorGrabMode::Locked);
            } else {
                window.set_cursor_visible(true);
                let _ = window.set_cursor_grab(CursorGrabMode::None);
            }
        });
    }

    pub fn handle_mouse_movement(&mut self, (x, y): (f64, f64)) {
        // behave like bspguy
        if !self.mouse_right_hold {
            return;
        }

        self.render_state
            .camera
            .set_pitch(self.render_state.camera.pitch() + Deg(-y as f32 * SENSITIVITY));
        self.render_state
            .camera
            .set_yaw(self.render_state.camera.yaw() + Deg(-x as f32 * SENSITIVITY));

        self.render_state.camera.rebuild_orientation();
    }
}
