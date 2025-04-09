use tracing::info;

use super::EguiRenderer;

impl EguiRenderer {
    pub fn hello_world(&self) {
        egui::Window::new("winit + egui + wgpu says hello!")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(self.context(), |ui| {
                ui.label("Label!");

                if ui.button("Button!").clicked() {
                    println!("boom!")
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Pixels per point: {}",
                        self.context().pixels_per_point()
                    ));
                    if ui.button("-").clicked() {
                        // state.scale_factor = (state.scale_factor - 0.1).max(0.3);
                        info!("hello minus");
                    }
                    if ui.button("+").clicked() {
                        info!("hello plus");
                        // state.scale_factor = (state.scale_factor + 0.1).min(3.0);
                    }
                });
            });
    }
}
