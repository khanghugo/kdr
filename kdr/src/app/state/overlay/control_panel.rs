use tracing::warn;

use crate::{
    app::{
        AppEvent,
        constants::{
            DEFAULT_FRAMETIME, DEFAULT_NOCLIP_SPEED, DEFAULT_SENSITIVITY, WINDOW_MINIMUM_HEIGHT,
            WINDOW_MINIMUM_WIDTH,
        },
        state::{AppState, replay},
    },
    renderer::camera::{FOV_DEFAULT, FOV_MAX, FOV_MIN},
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
        let title_name = self
            .file_state
            .selected_file
            .clone()
            .unwrap_or("kdr".to_string());

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

                    let mut read_only = self
                        .file_state
                        .selected_file
                        .clone()
                        .unwrap_or("".to_string());

                    // need to do like this so it cant be editted and it looks cool
                    ui.add_enabled_ui(false, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut read_only)
                                .hint_text("Choose .bsp or .dem"),
                        );
                    });

                    if ui.button("Select File").clicked() {
                        self.file_state.trigger_file_dialogue();
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

                // resolutions
                ui.horizontal(|ui| {
                    {
                        let window_state = self.window_state.as_mut().unwrap();

                        let width_drag = egui::DragValue::new(&mut window_state.width)
                            .range(WINDOW_MINIMUM_WIDTH..=3840);
                        let height_drag = egui::DragValue::new(&mut window_state.height)
                            .range(WINDOW_MINIMUM_HEIGHT..=2160);

                        ui.add_enabled_ui(!window_state.is_fullscreen, |ui| {
                            ui.label("Width");
                            let width_response = ui.add(width_drag);
                            ui.label("Height");
                            let height_response = ui.add(height_drag);

                            if width_response.changed() || height_response.changed() {
                                self.event_loop_proxy
                                    .send_event(AppEvent::RequestResize)
                                    .unwrap_or_else(|_| warn!("Failed to send RequestResize"));
                            }
                        });

                        let response = ui
                            .checkbox(&mut window_state.is_fullscreen, "Fullscreen")
                            .on_hover_text("Press F11");

                        if response.changed() {
                            let _ = self
                                .event_loop_proxy
                                .send_event(AppEvent::RequestToggleFullScreen);
                        }
                    }

                    // fov slider
                    ui.label("FOV");
                    let fov_slider = egui::DragValue::new(&mut self.render_state.camera.fovx.0)
                        .range(FOV_MIN..=FOV_MAX);

                    let response = ui.add(fov_slider);

                    if response.changed() {
                        self.update_fov();
                    }

                    // middle click to reset
                    if response.middle_clicked() {
                        self.render_state.camera.fovx.0 = FOV_DEFAULT;
                        self.update_fov();
                    }
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
                            .send_event(AppEvent::ReceivePostProcessingUpdate(
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
