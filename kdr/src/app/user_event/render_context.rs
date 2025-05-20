use std::sync::Arc;

use common::vec3;
use loader::bsp_resource::BspResource;
use tracing::{info, warn};
use winit::window::Window;

use crate::{
    app::{
        App, AppEvent,
        state::{
            entities::{EntityState, playermodel::PlayerModelState, viewmodel::ViewModelState},
            file::SelectedFileType,
            render::RenderOptions,
        },
    },
    renderer::{
        EguiRenderer, RenderContext, camera::Camera, skybox::SkyboxBuffer,
        world_buffer::WorldStaticBuffer,
    },
    utils::spawn_async,
};

impl App {
    pub(super) fn create_render_context(&mut self, window: Arc<Window>) {
        info!("Creating a render context");

        let render_context_future = RenderContext::new(window.clone());

        let event_loop_proxy = self.event_loop_proxy.clone();
        let send_message = move |render_context: RenderContext| {
            event_loop_proxy
                .send_event(AppEvent::FinishCreateRenderContext(render_context))
                .unwrap_or_else(|_| warn!("Failed to send FinishCreateRenderContext"));
        };

        spawn_async(async move {
            let render_context = render_context_future.await;
            send_message(render_context);
        });
    }

    pub(super) fn finish_create_render_context(&mut self, render_context: RenderContext) {
        info!("Finished creating a render context");

        self.render_context = render_context.into();

        // parsing query (first?) if possible
        #[cfg(target_arch = "wasm32")]
        {
            self.event_loop_proxy
                .send_event(AppEvent::CheckHostConfiguration)
                .unwrap_or_else(|_| warn!("Failed to send CheckHostConfiguration"));
        }

        // create egui after render context is done initializing
        self.event_loop_proxy
            .send_event(AppEvent::CreateEgui)
            .unwrap_or_else(|_| warn!("Failed to send CreateEgui"));

        // request common resource at the same time as well because why not
        self.event_loop_proxy
            .send_event(AppEvent::RequestCommonResource)
            .unwrap_or_else(|_| warn!("Failed to send RequestCommonResource"));

        // also requesting map list
        if self.options.fetch_map_list {
            self.event_loop_proxy
                .send_event(AppEvent::RequestMapList)
                .unwrap_or_else(|_| warn!("Failed to send RequestMapList"));
        }

        // also replay list
        if self.options.fetch_replay_list {
            self.event_loop_proxy
                .send_event(AppEvent::RequestReplayList)
                .unwrap_or_else(|_| warn!("Failed to send RequestReplayList"));
        }
    }

    pub(super) fn create_egui(&mut self) {
        info!("Creating egui renderer");

        let Some(window_state) = self.state.window_state.clone() else {
            warn!("Window is not initialized. Cannot create egui renderer");
            return;
        };

        let Some(render_context) = &self.render_context else {
            warn!("Render context is not initialized. Cannot create egui renderer");
            return;
        };

        let egui_renderer = EguiRenderer::new(
            render_context.device(),
            render_context.swapchain_format().clone(),
            None,
            1,
            &window_state.window(),
        );

        self.egui_renderer = egui_renderer.into();

        info!("Finished creating egui renderer");
    }

    pub(super) fn finish_create_world(
        &mut self,
        bsp_resource: BspResource,
        world_buffer: WorldStaticBuffer,
        skybox_buffer: Option<SkyboxBuffer>,
    ) {
        self.state.render_state.world_buffer = world_buffer.into();

        self.state.render_state.skybox = skybox_buffer;

        // inserting audio from bsp resourec
        // but first, need to clear audio that are not part of the common resource
        self.state
            .audio_resource
            .retain(|k, _| self.state.other_resources.common_resource.contains_key(k));

        bsp_resource.sound_lookup.iter().for_each(|(k, v)| {
            self.state.audio_resource.insert(k.to_string(), v.clone());
        });

        // restart the camera
        self.state.render_state.camera = Camera::default();

        // but then set our camera to be in one of the spawn location
        bsp_resource
            .bsp
            .entities
            .iter()
            .find(|entity| {
                entity
                    .get("classname")
                    .is_some_and(|classname| classname == "info_player_start")
            })
            .map(|entity| {
                entity
                    .get("origin")
                    .and_then(|origin_text| vec3(&origin_text))
                    .map(|origin| {
                        self.state.render_state.camera.set_position(origin);
                        self.state.render_state.camera.rebuild_orientation();
                    });
            });

        // restart render options
        self.state.render_state.render_options = RenderOptions::default();

        // if loading bsp, just force free cam every time
        match self.state.file_state.selected_file_type {
            SelectedFileType::Bsp => {
                self.state.input_state.free_cam = true;
            }
            _ => (),
        }
        // reset file input tpye
        self.state.file_state.selected_file_type = SelectedFileType::None;

        // reset texts
        self.state.text_state.clear_text();

        // resetting time when we are ready
        self.state.time = 0.;

        // stop spinner
        self.state.file_state.stop_spinner();

        let Some(render_context) = self.render_context.as_ref() else {
            return;
        };

        // entity dictionary
        self.state.entity_state = Some(EntityState {
            entity_dictionary: bsp_resource.entity_dictionary,
            viewmodel_state: ViewModelState::default(),
            playermodel_state: PlayerModelState::new(
                render_context.device(),
                render_context.queue(),
            ),
        });
    }
}
