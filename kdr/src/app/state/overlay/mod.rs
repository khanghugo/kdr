use control_panel::{ControlPanelUIState, PostProcessingControlState};
use map_list::MapListUIState;
use replay_list::ReplayListUIState;
use unknown_format_modal::UnknownFormatModalUIState;

use super::AppState;

pub mod control_panel;
mod crosshair;
mod loading_spinner;
mod map_list;
mod puppet_player_info;
mod puppet_player_list;
mod replay_list;
mod seekbar;
pub mod text;
mod unknown_format_modal;

pub struct UIState {
    /// Main UI includes: control panel, seekbar, demo list, map list.
    pub is_main_ui_enabled: bool,
    pub control_panel: ControlPanelUIState,
    pub pp_control: PostProcessingControlState,
    pub map_list: MapListUIState,
    pub replay_list: ReplayListUIState,
    pub unknown_format_modal: UnknownFormatModalUIState,
    pub toaster: egui_notify::Toasts,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_main_ui_enabled: true,
            control_panel: ControlPanelUIState::default(),
            pp_control: PostProcessingControlState::default(),
            map_list: MapListUIState::default(),
            replay_list: ReplayListUIState::default(),
            unknown_format_modal: UnknownFormatModalUIState::default(),
            toaster: egui_notify::Toasts::default(),
        }
    }
}

impl AppState {
    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |ctx| {
            if !self.loading_spinner(ctx) {
                // only draws crosshair when there is no spinner
                if self.ui_state.control_panel.crosshair {
                    self.crosshair(ctx);
                }
            }

            // need to draw these guys early so they don't stack on top of each other because of area from the saytext
            if self.ui_state.is_main_ui_enabled {
                if !self.playback_state.is_none() {
                    self.seek_bar(ctx);
                }

                self.ui_state.toaster.show(ctx);

                self.control_panel(ctx);
                self.replay_list(ctx);
                self.map_list(ctx);
                self.puppet_player_list(ctx);
            }

            self.draw_entity_text(ctx);
            self.draw_say_text(ctx);
            self.puppet_player_info(ctx);

            self.draw_unknown_format_modal(ctx);
        }
    }
}
