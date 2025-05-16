use std::{
    f32::{self, consts::PI},
    path::Path,
};

use cgmath::EuclideanSpace;
use loader::ResourceMap;
use mdl::Mdl;
use tracing::warn;

use crate::{
    app::{App, state::AppState},
    renderer::world_buffer::WorldLoader,
};

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
    pub current_sequence: usize,
    pub time: f32,
    pub should_draw: bool,
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
            current_sequence: 0,
            should_draw: false,
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

        // only draw if we are not in free cam
        entity_state.viewmodel_state.should_draw = !self.input_state.free_cam;

        if !entity_state.viewmodel_state.should_draw {
            return;
        }

        let skeletal = &mut viewmodel_buffer.transformations;

        // move vieworigin z down 1, this seems pretty smart
        // """pushing the view origin down off of the same X/Z plane as the ent's origin will give the
        // gun a very nice 'shifting' effect when the player looks up/down. If there is a problem
        // with view model distortion, this may be a cause. (SJB)."""
        let view_origin = self.render_state.camera.pos.to_vec() - cgmath::Vector3::unit_z();

        skeletal.world_transformation = (view_origin, self.render_state.camera.orientation);
        skeletal.current_sequence_index = entity_state.viewmodel_state.current_sequence;

        let mvps = skeletal.build_mvp(entity_state.viewmodel_state.time);

        viewmodel_buffer.mvp_buffer.update_mvp_buffer_many(mvps, 0);

        // update sequence time
        entity_state.viewmodel_state.time += self.frame_time * self.playback_speed;
    }
}

impl App {
    pub fn load_viewmodels(&mut self, resource_map: &ResourceMap) {
        resource_map.iter().for_each(|(file_path, file_bytes)| {
            let is_viewmodel = file_path.starts_with("models/v_") && file_path.ends_with(".mdl");

            if !is_viewmodel {
                return;
            }

            let Ok(mdl) = Mdl::open_from_bytes(file_bytes) else {
                warn!("Cannot parse model {}", file_path);
                return;
            };

            let actual_file_name = Path::new(file_path).file_stem().unwrap().to_str().unwrap();

            // we should already have render context by this point
            let Some(render_context) = self.render_context.as_ref() else {
                warn!("Trying to create dynamic buffer without render context");
                return;
            };

            let dynamic_buffer = WorldLoader::load_dynamic_world(
                render_context.device(),
                render_context.queue(),
                actual_file_name,
                &mdl,
                0,
            );

            self.state
                .render_state
                .viewmodel_buffers
                .push(dynamic_buffer);
        });
    }
}
