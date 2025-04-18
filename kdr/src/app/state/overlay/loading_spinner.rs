use crate::app::state::{AppState, file::LoadingState};

impl AppState {
    // returns a boolean just to see if something is loading
    pub fn loading_spinner(&mut self, ctx: &egui::Context) -> bool {
        match &self.file_state.loading_state {
            LoadingState::Fetching { .. } | LoadingState::Loading { .. } => {
                egui::Area::new("loading spinner".into())
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            let text = match &self.file_state.loading_state {
                                LoadingState::Fetching {
                                    file_name,
                                    progress,
                                } => format!(
                                    "Fetching {} \n{}%",
                                    file_name,
                                    (progress * 100.).round() as i32
                                ),
                                LoadingState::Loading { file_name } => {
                                    format!("Loading {}", file_name)
                                }
                                LoadingState::Idle => unreachable!(),
                            };

                            let rich_text = egui::RichText::new(text)
                            .strong()
                            .code()
                            // .size(32.)
                            .color(egui::Color32::WHITE)
                            // .background_color(egui::Color32::LIGHT_GRAY)
                            ;
                            let text_label =
                                egui::Label::new(rich_text).wrap_mode(egui::TextWrapMode::Extend);

                            ui.add(text_label);
                            ui.spinner();
                        });
                    });
                return true;
            }
            LoadingState::Idle => {
                return false;
            }
        }
    }
}
