use cgmath::InnerSpace;

use crate::app::state::AppState;

impl AppState {
    pub(super) fn misc_tick(&mut self) {
        let Some(bsp) = &self.other_resources.bsp else {
            return;
        };

        let start = self.render_state.camera.pos;
        // i should better rotate the vector myself but whatever
        let end = self.render_state.camera.target;
        let trace_end = start + (end - start).normalize() * 8192.0;

        let tr = bsp.trace_line_hull(
            self.ui_state.debug_panel.trace_hull_type,
            [start.x, start.y, start.z].into(),
            [trace_end.x, trace_end.y, trace_end.z].into(),
        );

        self.ui_state.debug_panel.trace_result.end_pos = tr.end_pos;
    }
}
