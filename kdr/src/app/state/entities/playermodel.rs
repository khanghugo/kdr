use std::path::Path;

use cgmath::{Deg, Rotation3, Zero};
use common::WorldTransformationSkeletal;
use loader::ResourceMap;
use mdl::Mdl;
use tracing::warn;

use crate::{
    app::{App, constants::MAX_MVP, state::AppState},
    renderer::{mvp_buffer::MvpBuffer, world_buffer::WorldLoader},
};

pub struct PlayerModel {
    pub model_name: String,
    pub player_name: String,
    pub origin: cgmath::Vector3<f32>,
    pub yaw: f32,
    pub sequence: usize,
    pub gaitsequence: usize,
    pub sequence_time: f32,

    // player might be available but just don't draw them
    pub should_draw: bool,

    // kind of instance data without instance drawing
    pub mvp_buffer: MvpBuffer,
}

pub struct PlayerModelState {
    pub players: Vec<PlayerModel>,
}

impl PlayerModel {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            model_name: "leet".into(),
            player_name: "arte".into(),
            origin: cgmath::Vector3::zero(),
            yaw: 0.,
            sequence: 0,
            gaitsequence: 0,
            sequence_time: 0.,
            should_draw: false,
            // stupid
            // TODO not stupid
            mvp_buffer: MvpBuffer::create_mvp(
                device,
                queue,
                vec![cgmath::Matrix4::zero(); MAX_MVP],
            ),
        }
    }
}

impl PlayerModelState {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            players: vec![PlayerModel::new(device, queue)],
        }
    }

    pub fn toggle_off_draw(&mut self) {
        self.players
            .iter_mut()
            .for_each(|player| player.should_draw = false);
    }
}

impl PlayerModel {
    pub fn build_mvp(
        &self,
        skeletal: &mut WorldTransformationSkeletal,
    ) -> Vec<cgmath::Matrix4<f32>> {
        skeletal.current_sequence_index = self.sequence;
        skeletal.world_transformation.0 = self.origin;
        skeletal.world_transformation.1 = cgmath::Quaternion::from_angle_z(Deg(self.yaw));

        skeletal.build_mvp_with_gait_sequence(self.sequence_time, self.gaitsequence)
    }
}

impl AppState {
    pub(super) fn playermodel_tick(&mut self) {
        let Some(entity_state) = self.entity_state.as_mut() else {
            return;
        };

        // just update time here, nothing else
        // the rest is done inside the render function, LOL, fucking stupid
        entity_state
            .playermodel_state
            .players
            .iter_mut()
            .for_each(|player| player.sequence_time += self.frame_time);
    }
}

impl App {
    pub fn load_player_models(&mut self, resource_map: &ResourceMap) {
        resource_map.iter().for_each(|(file_path, file_bytes)| {
            let is_player_model =
                file_path.starts_with("models/player/") && file_path.ends_with(".mdl");

            if !is_player_model {
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
                .playermodel_buffers
                .push(dynamic_buffer);
        });
    }
}
