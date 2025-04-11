use super::AppState;

pub mod text;
pub mod ui;

impl AppState {
    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |ctx| {
            if self.ui_state.crosshair {
                self.crosshair(ctx);
            }

            self.draw_entity_text(ctx);

            // anything interactive start from here
            if !self.ui_state.main_ui {
                return;
            }

            self.main_ui(ctx);

            if self.replay.is_some() {
                self.seek_bar(ctx);
            }
        }
    }
}
