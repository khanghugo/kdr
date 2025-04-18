use crate::app::{constants::DEFAULT_FRAMETIME, state::AppState};

impl AppState {
    pub fn seek_bar(&mut self, ctx: &egui::Context) {
        const SLIDER_WIDTH_PERC: f32 = 0.75;

        let (width, height) = self.window_dimensions().unwrap();
        let slider_width = width as f32 * SLIDER_WIDTH_PERC;
        // going up more
        let height_offset = height as f32 * 0.04;
        let slider_max = self
            .replay
            .as_ref()
            .map(|ghost| ghost.ghost.get_ghost_length()(DEFAULT_FRAMETIME))
            .unwrap_or(1.0);

        egui::Area::new(egui::Id::new("seekbar-area"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0., -height_offset])
            .show(ctx, |ui| {
                let frame_margin = width as f32 * 0.01;

                egui::Frame::default()
                    .fill(egui::Color32::from_gray(30))
                    .inner_margin(frame_margin)
                    .outer_margin(frame_margin)
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(0.5, egui::Color32::GRAY))
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().slider_width = slider_width;

                            // timeline slider
                            let timeline_slider =
                                egui::Slider::new(&mut self.time, 0.0..=slider_max)
                                    .show_value(false);

                            let response = ui.add(timeline_slider);

                            if response.changed() {
                                // if seekbar is used, reset text states and alike
                                self.text_state.entity_text.clear();
                            }

                            // current_time
                            ui.horizontal_centered(|ui| {
                                let time_text = egui::RichText::new(format_time(self.time))
                                    .size(14.0)
                                    .monospace();
                                let time_label = egui::Label::new(time_text).selectable(false);

                                ui.add(time_label);
                            });

                            // pause button
                            let pause_button_size = height as f32 * 0.01;
                            let pause_icon = egui::RichText::new("⏸").size(24.0);
                            let pause_button = egui::Button::new(pause_icon)
                                .min_size([pause_button_size, pause_button_size].into())
                                .selected(self.paused);

                            if ui.add(pause_button).clicked() {
                                self.paused = !self.paused;
                            }

                            // playback speed slider/drag value
                            let drag_size = height as f32 * 0.03;
                            let speed_slider = egui::DragValue::new(&mut self.playback_speed)
                                .range(0.0..=16.0)
                                .speed(0.125)
                                .max_decimals(3)
                                .suffix("x");

                            let response = ui.add_sized([drag_size, drag_size], speed_slider);

                            // middle click to reset
                            if response.clicked_by(egui::PointerButton::Middle) {
                                self.playback_speed = 1.0;
                            }
                        });
                    });
            });
    }
}

fn format_time(time_in_secs: f32) -> String {
    let minutes = time_in_secs.div_euclid(60.);
    let seconds = (time_in_secs % 60.0).floor();
    let fract = (time_in_secs.fract() * 100.0).floor();

    format!(
        "{:02}:{:02}.{:02}",
        minutes as i32, seconds as i32, fract as i32
    )
}
