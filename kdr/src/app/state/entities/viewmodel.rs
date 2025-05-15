use std::f32::{self, consts::PI};

use cgmath::EuclideanSpace;

use crate::app::state::AppState;

pub struct ViewModelState {
    // settings
    pub _bob: f32,
    pub _bob_cycle: f32,
    pub _bob_up: f32,
    // result
    pub cycle: f32,
    pub bob: f32,
    pub bob_time: f32,
    pub active_viewmodel: String,
    pub time: f32,
}

impl Default for ViewModelState {
    fn default() -> Self {
        Self {
            _bob: 0.01,
            _bob_cycle: 0.8,
            _bob_up: 0.5,
            cycle: 0.,
            bob: 0.,
            bob_time: 0.,
            active_viewmodel: "usp".to_string(),
            time: 0.,
        }
    }
}

impl ViewModelState {
    /// Takes delta time, not actual time :()
    pub(super) fn calculate_bob(&mut self, dt: f32) {
        self.bob_time += dt;
        self.cycle = self.bob_time - (self.bob_time / self._bob_cycle).round() * self._bob_cycle;
        self.cycle /= self._bob_cycle;

        if self.cycle < self._bob_up {
            self.cycle = PI * self.cycle / self._bob_up;
        } else {
            self.cycle = PI + PI * (self.cycle - self._bob_up) / (1. - self._bob_up);
        }
    }
}

impl AppState {
    pub(super) fn viewmodel_tick(&mut self) {
        let Some(entity_state) = self.entity_state.as_mut() else {
            return;
        };

        let Some(viewmodel_buffer) = self.render_state.viewmodel_buffers.iter_mut().find(|x| {
            x.name
                .contains(&entity_state.viewmodel_state.active_viewmodel)
        }) else {
            return;
        };

        let skeletal = &mut viewmodel_buffer.transformations;

        // move vieworigin z down 1, this seems pretty smart
        // """pushing the view origin down off of the same X/Z plane as the ent's origin will give the
        // gun a very nice 'shifting' effect when the player looks up/down. If there is a problem
        // with view model distortion, this may be a cause. (SJB)."""
        let view_origin = self.render_state.camera.pos.to_vec() - cgmath::Vector3::unit_z();

        skeletal.world_transformation = (view_origin, self.render_state.camera.orientation);

        let mvps = skeletal.build_mvp(entity_state.viewmodel_state.time);

        viewmodel_buffer.mvp_buffer.update_mvp_buffer_many(mvps, 0);
    }
}
