use crate::app::state::AppState;

impl AppState {
    pub fn puppet_player_list(&mut self, ctx: &egui::Context) {
        let Some(puppet_state) = self.puppet_state.as_mut() else {
            return;
        };

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
                    puppet_state.player_list.len(),
                    |ui, row_range| {
                        for row in row_range {
                            let player_name = &puppet_state.player_list[row];

                            if ui.selectable_label(false, player_name).clicked() {
                                puppet_state.selected_player = row;
                            }
                        }
                    },
                );
            });
    }
}
