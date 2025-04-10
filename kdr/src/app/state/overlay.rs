//! egui UI

use futures::FutureExt;
use rfd::AsyncFileDialog;
use tracing::warn;

use super::*;

impl AppState {
    pub fn trigger_file_dialogue(&mut self) {
        let future = AsyncFileDialog::new()
            .add_filter("BSP/DEM", &["bsp", "dem"])
            .pick_file();

        self.file_dialogue_future = Some(Box::pin(future))
    }

    pub fn state_poll(&mut self) {
        // only read the file name, yet to have the bytes
        if let Some(future) = &mut self.file_dialogue_future {
            if let Some(file_handle) = future.now_or_never() {
                self.selected_file = file_handle.map(|f| {
                    #[cfg(not(target_arch = "wasm32"))]
                    let result = f.path().display().to_string();

                    #[cfg(target_arch = "wasm32")]
                    let result = f.file_name();

                    self.file_bytes_future = Some(Box::pin(async move {
                        let bytes = f.read().await;
                        bytes
                    }));

                    return result;
                });

                self.file_dialogue_future = None;
            }
        }

        // now have the bytes
        if let Some(future) = &mut self.file_bytes_future {
            if let Some(file_bytes) = future.now_or_never() {
                self.selected_file_bytes = file_bytes.into();
                self.file_bytes_future = None;

                // only new file when we have the bytes
                self.event_loop_proxy
                    .send_event(CustomEvent::NewFileSelected)
                    .unwrap_or_else(|_| warn!("Cannot send NewFileSelected"));
            }
        }
    }

    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |context| {
            self.main_ui(context);
        }
    }

    pub fn main_ui(&mut self, context: &egui::Context) {
        let title_name = self.selected_file.clone().unwrap_or("kdr".to_string());

        egui::Window::new(title_name)
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(context, |ui| {
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

                ui.separator();

                ui.add_enabled_ui(self.ghost.is_some(), |ui| {
                    ui.label("Demo Player Controls");

                    ui.horizontal(|ui| {
                        if ui.button("-5").clicked() {
                            self.time -= Duration::from_secs(5);
                        }

                        let pause_button = egui::Button::new("Pause").selected(self.paused);

                        if ui.add(pause_button).clicked() {
                            self.paused = !self.paused;
                        }

                        if ui.button("+5").clicked() {
                            self.time += Duration::from_secs(5);
                        }
                    });
                });
            });
    }
}
