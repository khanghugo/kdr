use crate::app::state::AppState;

pub struct DebugPanelUIState {
    pub trace_hull_type: bsp::HullType,
    pub trace_result: bsp::TraceResult,
}

impl Default for DebugPanelUIState {
    fn default() -> Self {
        Self {
            trace_hull_type: bsp::HullType::Point,
            trace_result: Default::default(),
        }
    }
}

impl AppState {
    pub(super) fn debug_panel(&mut self, ctx: &egui::Context) {
        if !self.ui_state.control_panel.enable_debug_panel {
            return;
        }

        egui::Window::new("Debug Panel")
            .resizable(false)
            .vscroll(false)
            .default_open(true)
            .collapsible(true)
            .show(ctx, |ui| {
                ui.separator();

                egui::ComboBox::from_label("Hull Type")
                    .selected_text(format!("{:?}", self.ui_state.debug_panel.trace_hull_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.ui_state.debug_panel.trace_hull_type,
                            bsp::HullType::Point,
                            "Point",
                        );

                        ui.selectable_value(
                            &mut self.ui_state.debug_panel.trace_hull_type,
                            bsp::HullType::Stand,
                            "Stand",
                        );

                        ui.selectable_value(
                            &mut self.ui_state.debug_panel.trace_hull_type,
                            bsp::HullType::Monster,
                            "Monster",
                        );

                        ui.selectable_value(
                            &mut self.ui_state.debug_panel.trace_hull_type,
                            bsp::HullType::Duck,
                            "Duck",
                        );
                    });

                ui.label(format!(
                    "Current position: {:.03} {:.03} {:.03}",
                    self.render_state.camera.pos.x,
                    self.render_state.camera.pos.y,
                    self.render_state.camera.pos.z,
                ));

                ui.label(format!(
                    "Viewangles (PY): {:03.03} {:03.03}",
                    self.render_state.camera.pitch().0,
                    self.render_state.camera.yaw().0,
                ));

                ui.label(format!(
                    "Trace position: {:.03}",
                    self.ui_state.debug_panel.trace_result.end_pos
                ));

                ui.separator()
            });
    }
}
