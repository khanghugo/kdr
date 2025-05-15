use cgmath::Deg;

use crate::app::state::AppState;

impl AppState {
    pub(super) fn puppet_player_list(&mut self, ctx: &egui::Context) {
        let Some(puppet) = self.playback_state.get_puppet_mut() else {
            return;
        };

        let Some(current_frame) = puppet.frames.get(puppet.current_frame) else {
            return;
        };

        let viewinfo_count = current_frame.frame.len();

        egui::Window::new("Player list")
            .resizable(false)
            .default_open(false)
            .collapsible(true)
            .show(ctx, |ui| {
                let text_style = egui::TextStyle::Body;
                let row_height = ui.text_style_height(&text_style);

                egui::ScrollArea::vertical().show_rows(
                    ui,
                    row_height,
                    viewinfo_count,
                    |ui, row_range| {
                        for row in row_range {
                            let player_name = &current_frame.frame[row].player.name;

                            if ui.selectable_label(false, player_name).clicked() {
                                puppet.selected_player = player_name.to_string();

                                // update the camera to point there
                                self.render_state.camera.pos =
                                    current_frame.frame[row].vieworg.into();
                                self.render_state
                                    .camera
                                    .set_pitch(Deg(current_frame.frame[row].viewangles[0]));
                                self.render_state
                                    .camera
                                    .set_yaw(Deg(current_frame.frame[row].viewangles[1]));
                                self.render_state.camera.rebuild_orientation();
                            }
                        }
                    },
                );
            });
    }
}
