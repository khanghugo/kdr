use control_panel::{ControlPanelUIState, PostProcessingControlState};
use map_list::MapListUIState;

use super::AppState;

pub mod control_panel;
mod crosshair;
mod map_list;
mod seekbar;
pub mod text;

pub struct UIState {
    /// Main UI includes: control panel, seekbar, demo list, map list.
    pub is_main_ui_enabled: bool,
    pub control_panel: ControlPanelUIState,
    pub pp_control: PostProcessingControlState,
    pub map_list: MapListUIState,
}

impl Default for UIState {
    fn default() -> Self {
        Self {
            is_main_ui_enabled: true,
            control_panel: ControlPanelUIState::default(),
            pp_control: PostProcessingControlState::default(),
            map_list: MapListUIState::default(),
        }
    }
}

impl AppState {
    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |ctx| {
            if self.ui_state.control_panel.crosshair {
                self.crosshair(ctx);
            }

            self.draw_entity_text(ctx);

            // anything interactive start from here
            if !self.ui_state.is_main_ui_enabled {
                return;
            }

            self.control_panel(ctx);

            if self.replay.is_some() {
                self.seek_bar(ctx);
            }

            self.map_list(ctx);
        }
    }
}
