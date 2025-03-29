use std::time::Instant;

use bitflags::bitflags;
use cgmath::Deg;
use winit::{event::KeyEvent, keyboard::KeyCode};

use super::App;

pub const CAM_SPEED: f32 = 1000.;
pub const CAM_TURN: f32 = 150.; // degrees

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

    fn movent_tick(&mut self) {
        let now = Instant::now();
        self.frame_time = now.duration_since(self.last_time).as_secs_f32();
        self.last_time = now;

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

    pub fn tick(&mut self) {
        self.movent_tick();
    }

    pub fn handle_keyboard_input(&mut self, event: KeyEvent) {
        match event.physical_key {
            winit::keyboard::PhysicalKey::Code(key_code) => match key_code {
                KeyCode::KeyW => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Forward);
                    } else {
                        self.keys = self.keys.intersection(Key::Forward.complement());
                    }
                }
                KeyCode::KeyS => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Back);
                    } else {
                        self.keys = self.keys.intersection(Key::Back.complement());
                    }
                }
                KeyCode::KeyA => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::MoveLeft);
                    } else {
                        self.keys = self.keys.intersection(Key::MoveLeft.complement());
                    }
                }
                KeyCode::KeyD => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::MoveRight);
                    } else {
                        self.keys = self.keys.intersection(Key::MoveRight.complement());
                    }
                }
                KeyCode::ArrowLeft => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Left);
                    } else {
                        self.keys = self.keys.intersection(Key::Left.complement());
                    }
                }
                KeyCode::ArrowRight => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Right);
                    } else {
                        self.keys = self.keys.intersection(Key::Right.complement());
                    }
                }
                KeyCode::ArrowUp => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Up);
                    } else {
                        self.keys = self.keys.intersection(Key::Up.complement());
                    }
                }
                KeyCode::ArrowDown => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Down);
                    } else {
                        self.keys = self.keys.intersection(Key::Down.complement());
                    }
                }
                KeyCode::ShiftLeft => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Shift);
                    } else {
                        self.keys = self.keys.intersection(Key::Shift.complement());
                    }
                }
                KeyCode::ControlLeft => {
                    if event.state.is_pressed() {
                        self.keys = self.keys.union(Key::Control);
                    } else {
                        self.keys = self.keys.intersection(Key::Control.complement());
                    }
                }
                KeyCode::KeyQ => {
                    if event.state.is_pressed() {
                        panic!()
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }
}
