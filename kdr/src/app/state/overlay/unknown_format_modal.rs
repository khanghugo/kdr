use ghost::{GhostBlobType, get_ghost_blob_from_bytes};
use tracing::warn;

use crate::app::{AppEvent, state::AppState};

pub struct UnknownFormatModalUIState {
    pub enabled: bool,
    pub selected: String,
}

impl Default for UnknownFormatModalUIState {
    fn default() -> Self {
        let selected: &str = GhostBlobType::Demo.into();

        Self {
            enabled: false,
            selected: selected.to_string(),
        }
    }
}

impl AppState {
    pub(super) fn draw_unknown_format_modal(&mut self, ctx: &egui::Context) {
        if !self.ui_state.unknown_format_modal.enabled {
            return;
        }

        egui::Modal::new("unknown replay format modal".into()).show(ctx, |ui| {
            ui.heading("Unknown replay format");
            ui.separator();

            ui.label(self.file_state.selected_file.as_ref().unwrap());

            egui::ComboBox::from_label("Select replay format")
                .selected_text(&self.ui_state.unknown_format_modal.selected)
                .show_ui(ui, |ui| {
                    let demo_code: &str = GhostBlobType::Demo.into();
                    let simen_code: &str = GhostBlobType::Simen.into();
                    let sg_code: &str = GhostBlobType::SurfGateway.into();
                    let rj_code: &str = GhostBlobType::RomanianJumpers.into();
                    let hlkz_code: &str = GhostBlobType::SRHLKZ.into();

                    ui.selectable_value(
                        &mut self.ui_state.unknown_format_modal.selected,
                        demo_code.to_string(),
                        "Demo",
                    );

                    ui.selectable_value(
                        &mut self.ui_state.unknown_format_modal.selected,
                        simen_code.to_string(),
                        "Simen",
                    );

                    ui.selectable_value(
                        &mut self.ui_state.unknown_format_modal.selected,
                        sg_code.to_string(),
                        "Surf Gateway",
                    );

                    ui.selectable_value(
                        &mut self.ui_state.unknown_format_modal.selected,
                        rj_code.to_string(),
                        "Romanian-Jumpers",
                    );

                    ui.selectable_value(
                        &mut self.ui_state.unknown_format_modal.selected,
                        hlkz_code.to_string(),
                        "SourceRuns HLKZ",
                    );
                });

            if ui.button("Go").clicked() {
                self.ui_state.unknown_format_modal.enabled = false;

                let blob_type =
                    GhostBlobType::try_from(self.ui_state.unknown_format_modal.selected.as_str())
                        .unwrap();

                let file_name = self.file_state.selected_file.clone().unwrap();
                let file_bytes = self.file_state.selected_file_bytes.clone().unwrap();

                let Ok(ghost_blob) =
                    get_ghost_blob_from_bytes(file_name.as_str(), file_bytes, Some(blob_type))
                else {
                    self.event_loop_proxy
                        .send_event(AppEvent::ErrorEvent(crate::app::AppError::UnknownFile {
                            file_name,
                        }))
                        .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"));

                    return;
                };

                self.event_loop_proxy
                    .send_event(AppEvent::ReceiveReplayBlob {
                        replay_name: file_name.into(),
                        replay_blob: ghost_blob,
                    })
                    .unwrap_or_else(|_| warn!("Cannot send ReceiveReplayBlob"));

                // self.event_loop_proxy.send_event()
            }
        });
    }
}
