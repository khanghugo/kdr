use tracing::warn;

use crate::app::{
    CustomEvent,
    constants::{DEFAULT_FRAMETIME, DEFAULT_NOCLIP_SPEED, DEFAULT_SENSITIVITY},
    state::{AppState, replay},
};

// awkward......
// directly affecting the rendering state, welp
// need to maintain the state in the UI and then send that to the renderer
// and then the renderer will send the state back to the UI
#[derive(Clone, Copy, Default)]
pub struct PostProcessingControlState {
    pub kuwahara: bool,
    pub bloom: bool,
    pub chromatic_aberration: bool,
    pub gray_scale: bool,
}

pub struct ControlPanelUIState {
    pub crosshair: bool,
}

impl Default for ControlPanelUIState {
    fn default() -> Self {
        Self { crosshair: true }
    }
}

impl AppState {
    pub fn control_panel(&mut self, ctx: &egui::Context) {
        const MAX_TITLE_LENGTH: usize = 44;
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

                    if ui.button("buss").clicked() {
                        self.play_audio_test();
                    }

                    if ui.button("buss2").clicked() {
                        self.play_audio_test2();
                    }
                });

                // replay info
                if let Some(replay) = &self.replay {
                    ui.separator();

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
                    let volume_slider = egui::DragValue::new(&mut self.audio_state.volume)
                        // goes to 2 so it can be a lot louder
                        .range(0.0..=2.)
                        .speed(0.025)
                        .fixed_decimals(2);

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

                    // volume
                    ui.label("Volume:");
                    ui.add(volume_slider);
                });

                // other settings but in a different row
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.input_state.free_cam, "Freecam");

                    ui.checkbox(&mut self.ui_state.control_panel.crosshair, "Crosshair");
                });

                // post processing settings
                ui.separator();
                ui.label("Effects");

                ui.horizontal(|ui| {
                    let kuwahara_response =
                        ui.checkbox(&mut self.ui_state.pp_control.kuwahara, "Kuwahara");
                    let bloom_response = ui.checkbox(&mut self.ui_state.pp_control.bloom, "Bloom");
                    let cr_response = ui.checkbox(
                        &mut self.ui_state.pp_control.chromatic_aberration,
                        "C. Aberration",
                    );
                    let gs_response =
                        ui.checkbox(&mut self.ui_state.pp_control.gray_scale, "Gray Scale");

                    if kuwahara_response.clicked()
                        || bloom_response.clicked()
                        || cr_response.clicked()
                        || gs_response.clicked()
                    {
                        self.event_loop_proxy
                            .send_event(CustomEvent::ReceivePostProcessingUpdate(
                                self.ui_state.pp_control,
                            ))
                            .unwrap_or_else(|_| {
                                warn!("Failed to send ReceivePostProcessingUpdate")
                            });
                    }
                });

                // render options
                ui.separator();
                ui.label("Render options");

                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.render_state.render_options.render_nodraw,
                        "NoDraw Textures",
                    );
                    ui.checkbox(
                        &mut self.render_state.render_options.render_beyond_sky,
                        "Beyond Sky",
                    )
                    .on_hover_text("Currently not working");

                    ui.checkbox(
                        &mut self.render_state.render_options.full_bright,
                        "Full Bright",
                    );
                });

                ui.horizontal(|ui| {
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
