use crate::{app::state::AppState, utils::format_time};

impl AppState {
    pub(super) fn puppet_player_info(&mut self, ctx: &egui::Context) {
        let Some(puppet) = self.playback_state.get_puppet() else {
            return;
        };

        // TODO maybe the interpolation can store the player name instead of us finding it here again
        let Some(curr_viewinfo) = puppet.frames.get(puppet.current_frame).and_then(|frame| {
            frame
                .frame
                .iter()
                .find(|viewinfo| puppet.selected_player == viewinfo.player.name)
        }) else {
            return;
        };

        let formatted_time = format_time(curr_viewinfo.timer_time);

        // need to hash player name as well so the size is dynamic
        // as for the timer, welp
        egui::Area::new(egui::Id::new(("puppet-timer", &curr_viewinfo.player.name)))
            .anchor(egui::Align2::CENTER_TOP, [0., 0.])
            .constrain(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_gray(30))
                    .inner_margin(8.0)
                    .outer_margin(8.0)
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(0.5, egui::Color32::GRAY))
                    .show(ui, |ui| {
                        ui.vertical_centered_justified(|ui| {
                            let my_label = |text: &str| {
                                let my_text = egui::RichText::new(text)
                                    .color(egui::Color32::from_rgba_premultiplied(
                                        255, 255, 255, 255,
                                    ))
                                    .font(egui::FontId::new(18., egui::FontFamily::Monospace));

                                egui::Label::new(my_text).selectable(false)
                            };

                            let player_label = my_label(&curr_viewinfo.player.name);
                            ui.add(player_label);

                            let timer_label = my_label(&formatted_time);
                            ui.add_enabled(curr_viewinfo.timer_time != 0., timer_label);
                        });
                    });
            });
    }
}
