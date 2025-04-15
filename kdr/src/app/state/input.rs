use bitflags::bitflags;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::{KeyCode, PhysicalKey},
    window::CursorGrabMode,
};

use crate::app::constants::{DEFAULT_NOCLIP_SPEED, DEFAULT_SENSITIVITY};

use super::AppState;

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
        const Alt       = (1 << 10);
    }
}

#[derive(Debug)]
pub struct InputState {
    pub keys: Key,
    pub mouse_right_hold: bool,
    pub sensitivity: f32,
    pub noclip_speed: f32,
    pub free_cam: bool,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys: Key::empty(),
            mouse_right_hold: false,
            sensitivity: DEFAULT_SENSITIVITY,
            noclip_speed: DEFAULT_NOCLIP_SPEED,
            free_cam: true,
        }
    }
}

impl AppState {
    pub fn handle_keyboard_input(&mut self, physical_key: PhysicalKey, state: ElementState) {
        match physical_key {
            winit::keyboard::PhysicalKey::Code(key_code) => match key_code {
                KeyCode::KeyW => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Forward);
                    } else {
                        self.input_state.keys = self
                            .input_state
                            .keys
                            .intersection(Key::Forward.complement());
                    }
                }
                KeyCode::KeyS => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Back);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Back.complement());
                    }
                }
                KeyCode::KeyA => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::MoveLeft);
                    } else {
                        self.input_state.keys = self
                            .input_state
                            .keys
                            .intersection(Key::MoveLeft.complement());
                    }
                }
                KeyCode::KeyD => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::MoveRight);
                    } else {
                        self.input_state.keys = self
                            .input_state
                            .keys
                            .intersection(Key::MoveRight.complement());
                    }
                }
                KeyCode::ArrowLeft => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Left);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Left.complement());
                    }
                }
                KeyCode::ArrowRight => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Right);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Right.complement());
                    }
                }
                KeyCode::ArrowUp => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Up);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Up.complement());
                    }
                }
                KeyCode::ArrowDown => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Down);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Down.complement());
                    }
                }
                KeyCode::ShiftLeft => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Shift);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Shift.complement());
                    }
                }
                KeyCode::ControlLeft => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Control);
                    } else {
                        self.input_state.keys = self
                            .input_state
                            .keys
                            .intersection(Key::Control.complement());
                    }
                }
                KeyCode::AltLeft => {
                    if state.is_pressed() {
                        self.input_state.keys = self.input_state.keys.union(Key::Alt);
                    } else {
                        self.input_state.keys =
                            self.input_state.keys.intersection(Key::Alt.complement());
                    }
                }
                KeyCode::Escape => {
                    if state.is_pressed() {
                        self.ui_state.is_main_ui_enabled = !self.ui_state.is_main_ui_enabled;
                    }
                }
                KeyCode::Space | KeyCode::KeyK => {
                    if state.is_pressed() {
                        self.paused = !self.paused;
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }

    pub fn handle_mouse_input(&mut self, mouse_button: &MouseButton, state: &ElementState) {
        let Some(window) = &self.window else { return };

        match mouse_button {
            MouseButton::Right => self.input_state.mouse_right_hold = state.is_pressed(),
            _ => (),
        }

        if self.input_state.mouse_right_hold {
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
    }
}
