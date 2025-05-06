use crate::{
    app::{constants::DEFAULT_FRAMETIME, state::AppState},
    utils::format_time,
};

impl AppState {
    pub fn seek_bar(&mut self, ctx: &egui::Context) {
        const SLIDER_WIDTH_PERC: f32 = 0.65;

        // use winit_window_dimensions() becuase of some weird scaling bugs
        // and we don't need absolute positions anyway
        let (width, height) = self.winit_window_dimensions().unwrap();
        let slider_width = width as f32 * SLIDER_WIDTH_PERC;

        let height_offset = height as f32 * 0.04; // going up more

        let (slider_min, slider_max) = match &self.playback_state.playback_mode {
            crate::app::state::playback::PlaybackMode::Replay(replay) => {
                (0., replay.ghost.get_ghost_length()(DEFAULT_FRAMETIME))
            }
            crate::app::state::playback::PlaybackMode::Live(puppet) => {
                let min = puppet
                    .frames
                    .0
                    .front()
                    .map(|frame| frame.server_time)
                    .unwrap_or(0.);
                let max = puppet
                    .frames
                    .0
                    .iter()
                    .last()
                    .map(|frame| frame.server_time)
                    .unwrap_or(1.);

                (min, max)
            }
            crate::app::state::playback::PlaybackMode::None => unreachable!(),
        };

        egui::Area::new(egui::Id::new("seekbar-area"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0., -height_offset])
            .constrain(false)
            .movable(false)
            .show(ctx, |ui| {
                let frame_margin = width as f32 * 0.01;

                egui::Frame::new()
                    .fill(egui::Color32::from_gray(30))
                    .inner_margin(frame_margin)
                    .outer_margin(frame_margin)
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(0.5, egui::Color32::GRAY))
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().slider_width = slider_width / ctx.zoom_factor();

                            // timeline slider
                            // WHAT THE FUCK
                            // THIS IMPLICITLY CHANGE THE SELF.TIME VALUE
                            // SO, WE DONT NEED SERVER TIME OFFSET
                            let timeline_slider =
                                egui::Slider::new(&mut self.time, slider_min..=slider_max)
                                    .show_value(false);

                            let response = ui.add(timeline_slider);

                            if response.changed() {
                                // if seekbar is used, reset text states and alike
                                // usually this will just work. But if it is scrubbed back, then we need
                                // to change something else
                                // i dont want to make this too complicated
                                self.text_state.clear_text();
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
                            let pause_icon = egui::RichText::new("⏸")
                            .size(24.0) // AAAAAAA
                            ;
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
