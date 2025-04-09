//! egui UI

use tracing::info;

use super::AppState;

impl AppState {
    pub fn draw_egui(&mut self) -> impl FnMut(&egui::Context) -> () {
        |context| {
            self.hello_world(context);
            self.hello_world2(context);
        }
    }

    pub fn hello_world(&mut self, context: &egui::Context) {
        egui::Window::new("winit + egui + wgpu says hello!")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(context, |ui| {
                ui.label("Label!");

                if ui.button("Button!").clicked() {
                    println!("boom!")
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(format!("Pixels per point: {}", context.pixels_per_point()));

                    if ui.button("-").clicked() {
                        // state.scale_factor = (state.scale_factor - 0.1).max(0.3);

                        info!("hello minus");
                    }

                    if ui.button("+").clicked() {
                        info!("hello plus");
                        self.debug += 1;

                        // state.scale_factor = (state.scale_factor + 0.1).min(3.0);
                    }
                });
            });
    }

    pub fn hello_world2(&mut self, context: &egui::Context) {
        egui::Window::new("hello world2")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(context, |ui| {
                ui.label("Label!");

                if ui.button("Button!").clicked() {
                    println!("boom!")
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(format!("Pixels per point: {}", context.pixels_per_point()));

                    if ui.button("-").clicked() {
                        // state.scale_factor = (state.scale_factor - 0.1).max(0.3);

                        info!("hello minus");
                    }

                    if ui.button("+").clicked() {
                        info!("hello plus");
                        self.debug += 1;

                        // state.scale_factor = (state.scale_factor + 0.1).min(3.0);
                    }
                });
            });
    }
}
