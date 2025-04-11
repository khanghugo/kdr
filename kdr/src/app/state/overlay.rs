//! egui UI
use crate::app::constants::{DEFAULT_FRAMETIME, DEFAULT_NOCLIP_SPEED, DEFAULT_SENSITIVITY};

use super::*;

pub struct UIState {
    pub enabled: bool,
}

// awkward......
// directly affecting the rendering state, welp
// need to maintain the state in the UI and then send that to the renderer
// and then the renderer will send the state back to the UI
#[derive(Clone, Copy, Default)]
pub struct PostProcessingState {
    pub kuwahara: bool,
    pub bloom: bool,
    pub chromatic_aberration: bool,
    pub gray_scale: bool,
}

impl AppState {
    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |ctx| {
            if !self.ui_state.enabled {
                return;
            }

            self.main_ui(ctx);

            if self.replay.is_some() {
                self.seek_bar(ctx);
            }
        }
    }

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

                            ui.add(timeline_slider);

                            // current_time
                            ui.horizontal_centered(|ui| {
                                ui.add_space(2.0);
                                let time_text = egui::RichText::new(format!("{:>6.02}", self.time))
                                    .size(14.0)
                                    .monospace();
                                ui.label(time_text);
                                ui.add_space(2.0);
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

    pub fn main_ui(&mut self, ctx: &egui::Context) {
        const MAX_TITLE_LENGTH: usize = 40;
        let title_name = self.selected_file.clone().unwrap_or("kdr".to_string());
        let title_name = if title_name.len() > MAX_TITLE_LENGTH {
            format!(
                "...{}",
                &title_name[title_name.len().saturating_sub(MAX_TITLE_LENGTH - 3)..]
            )
        } else {
            title_name
        };

        egui::Window::new(title_name)
            .resizable(false)
            .vscroll(false)
            .default_open(true)
            .collapsible(true)
            .show(ctx, |ui| {
                // file
                ui.horizontal(|ui| {
                    ui.label("File: ");

                    let mut read_only = self.selected_file.clone().unwrap_or("".to_string());

                    // need to do like this so it cant be editted and it looks cool
                    ui.add_enabled_ui(false, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut read_only)
                                .hint_text("Choose .bsp or .dem"),
                        );
                    });

                    if ui.button("Select File").clicked() {
                        self.trigger_file_dialogue();
                    }
                });

                // replay info
                if let Some(replay) = &self.replay {
                    ui.separator();

                    ui.label("Demo Info: ");

                    ui.vertical(|ui| {
                        let playback_mode = match replay.playback_mode {
                            replay::ReplayPlaybackMode::Immediate(_) => "Immediate",
                            replay::ReplayPlaybackMode::Interpolated => "Interpolated",
                            replay::ReplayPlaybackMode::FrameAccurate => "Frame Accurate",
                        };

                        ui.label(format!("Ghost name: {}", replay.ghost.ghost_name));
                        ui.label(format!("Map: {}", replay.ghost.map_name));
                        ui.label(format!("Game mod: {}", replay.ghost.game_mod));
                        ui.label(format!(
                            "Length: {:.2} seconds over {} frames",
                            replay.ghost.get_ghost_length()(DEFAULT_FRAMETIME),
                            replay.ghost.frames.len(),
                        ));
                        ui.label(format!("Playback mode: {}", playback_mode));
                    });
                }

                // settings
                ui.separator();

                // usual movement settings
                ui.horizontal(|ui| {
                    let sensitivity_slider =
                        egui::DragValue::new(&mut self.input_state.sensitivity)
                            .range(0.05..=6.0)
                            .speed(0.0125)
                            .max_decimals(4);
                    let noclip_speed_slider =
                        egui::DragValue::new(&mut self.input_state.noclip_speed)
                            .range(0.0..=4000.0)
                            .speed(10);

                    ui.label("Sensitivity:");
                    let response = ui.add(sensitivity_slider);

                    if response.clicked_by(egui::PointerButton::Middle) {
                        self.input_state.sensitivity = DEFAULT_SENSITIVITY;
                    }

                    ui.label("Speed:");
                    let response = ui.add(noclip_speed_slider).on_hover_text(
                        "You can press CTRL/SHIFT while moving to go slower/faster!!",
                    );

                    if response.clicked_by(egui::PointerButton::Middle) {
                        self.input_state.noclip_speed = DEFAULT_NOCLIP_SPEED;
                    }

                    ui.checkbox(&mut self.input_state.free_cam, "Free cam");
                });

                // post processing settings
                ui.horizontal(|ui| {
                    ui.label("Effects: ");

                    let kuwahara_response =
                        ui.checkbox(&mut self.post_processing_state.kuwahara, "Kuwahara");
                    let bloom_response =
                        ui.checkbox(&mut self.post_processing_state.bloom, "Bloom");
                    let cr_response = ui.checkbox(
                        &mut self.post_processing_state.chromatic_aberration,
                        "Chromatic Aberration",
                    );
                    let gs_response =
                        ui.checkbox(&mut self.post_processing_state.gray_scale, "Gray Scale");

                    if kuwahara_response.clicked()
                        || bloom_response.clicked()
                        || cr_response.clicked()
                        || gs_response.clicked()
                    {
                        self.event_loop_proxy
                            .send_event(CustomEvent::ReceivePostProcessingUpdate(
                                self.post_processing_state,
                            ))
                            .unwrap_or_else(|_| {
                                warn!("Failed to send ReceivePostProcessingUpdate")
                            });
                    }
                });

                // render options
                ui.horizontal(|ui| {
                    // pub render_nodraw: bool,
                    // // TODO, eh, make it better?
                    // pub render_beyond_sky: bool,
                    // pub render_skybox: bool,
                    // pub render_transparent: bool,
                    ui.label("Render:");

                    ui.checkbox(
                        &mut self.render_state.render_options.render_nodraw,
                        "No Draw Textures",
                    );
                    ui.checkbox(
                        &mut self.render_state.render_options.render_beyond_sky,
                        "Beyond Sky",
                    )
                    .on_hover_text("Currently not working");
                    ui.checkbox(
                        &mut self.render_state.render_options.render_skybox,
                        "Skybox",
                    );
                    ui.checkbox(
                        &mut self.render_state.render_options.render_transparent,
                        "Transparency",
                    );
                });

                // watermark
                ui.separator();
                ui.vertical_centered(|ui| {
                    ui.hyperlink_to("kdr on GitHub", "https://github.com/khanghugo/kdr");
                });
            });
    }
}
