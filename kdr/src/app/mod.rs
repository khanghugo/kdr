use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

use ::tracing::{info, warn};
use common::{KDR_CANVAS_ID, UNKNOWN_GAME_MOD, vec3};

#[cfg(target_arch = "wasm32")]
use common::{REQUEST_MAP_ENDPOINT, REQUEST_MAP_GAME_MOD_QUERY, REQUEST_REPLAY_ENDPOINT};

use constants::{DEFAULT_HEIGHT, DEFAULT_WIDTH};
use ghost::{GhostBlob, GhostInfo, get_ghost_blob_from_bytes};
use kira::sound::static_sound::StaticSoundData;
use state::{
    AppState,
    audio::{AudioBackend, AudioStateError},
    file::{LoadingState, SelectedFileType},
    overlay::control_panel::PostProcessingControlState,
    render::RenderOptions,
    replay::{Replay, ReplayPlaybackMode},
    window::WindowState,
};

// can use this for both native and web
use web_time::{Duration, Instant};

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop, EventLoopProxy},
    window::Window,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};

pub mod constants;
mod state;

use crate::{
    renderer::{
        RenderContext,
        camera::Camera,
        egui_renderer::EguiRenderer,
        skybox::{SkyboxBuffer, SkyboxLoader},
        world_buffer::{WorldBuffer, WorldLoader},
    },
    utils::spawn_async,
};
use loader::{
    MapIdentifier, MapList, ProgressResourceProvider, ReplayList, Resource, ResourceMap,
    ResourceProvider, bsp_resource::BspResource, error::ResourceProviderError,
};

#[cfg(not(target_arch = "wasm32"))]
use loader::native::NativeResourceProvider;

#[cfg(target_arch = "wasm32")]
use loader::web::{WebResourceProvider, parse_location_search};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Problems with resource provider: {source}")]
    ProviderError { source: ResourceProviderError },

    #[error("No resource provider found")]
    NoProvider,

    #[error("Audio error: {source}")]
    AudioError {
        #[source]
        source: AudioStateError,
    },

    #[error("Unknown file format: {file_name}")]
    UnknownFile { file_name: String },
}

pub enum AppEvent {
    CreateRenderContext(Arc<Window>),
    FinishCreateRenderContext(RenderContext),
    CreateEgui,
    RequestMap(MapIdentifier),
    ReceiveResource(Resource),
    NewFileSelected,
    RequestReplay(String),
    ReceiveReplayBlob {
        replay_name: PathBuf,
        replay_blob: GhostBlob,
    },
    ReceiveReplay(MapIdentifier, GhostInfo),
    ReceivePostProcessingUpdate(PostProcessingControlState),
    MaybeStartAudioBackEnd,
    RequestCommonResource,
    #[allow(dead_code)]
    ReceiveCommonResource(ResourceMap),
    RequestMapList,
    ReceivedMapList(MapList),
    RequestReplayList,
    ReceiveReplayList(ReplayList),
    FinishCreateWorld(BspResource, WorldBuffer, Option<SkyboxBuffer>),
    UpdateFetchProgress(f32),
    #[cfg(target_arch = "wasm32")]
    ParseLocationSearch,
    UnknownFormatModal,
    RequestResize,
    RequestEnterFullScreen,
    RequestExitFullScreen,
    RequestToggleFullScreen,
    ErrorEvent(AppError),
}

// TODO restructure this
// app might still be a general "app" that both native and web points to
// the difference might be the "window" aka where the canvas is
// though not sure how to handle loop, that is for my future self
struct App {
    render_context: Option<RenderContext>,
    egui_renderer: Option<EguiRenderer>,

    state: AppState,

    // resource provider
    #[cfg(not(target_arch = "wasm32"))]
    native_resource_provider: Option<NativeResourceProvider>,
    #[cfg(target_arch = "wasm32")]
    web_resource_provider: Option<WebResourceProvider>,

    // https://github.com/Jelmerta/Kloenk/blob/main/src/application.rs
    event_loop_proxy: EventLoopProxy<AppEvent>,
}

impl App {
    pub fn new(
        provider_uri: Option<String>,
        event_loop: &winit::event_loop::EventLoop<AppEvent>,
    ) -> Self {
        let provider = provider_uri.and_then(|provider_uri| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let path = Path::new(provider_uri.as_str());
                let native_provider = NativeResourceProvider::new(path);
                return Some(native_provider);
            }

            #[cfg(target_arch = "wasm32")]
            {
                let web_provider = WebResourceProvider::new(provider_uri);
                return Some(web_provider);
            }
        });

        let event_loop_proxy = event_loop.create_proxy();

        Self {
            render_context: None,
            egui_renderer: None,
            state: AppState::new(event_loop_proxy.clone()),
            #[cfg(not(target_arch = "wasm32"))]
            native_resource_provider: provider,
            #[cfg(target_arch = "wasm32")]
            web_resource_provider: provider,
            event_loop_proxy,
        }
    }

    pub fn resize(&mut self, physical_size: winit::dpi::PhysicalSize<u32>) {
        let Some(render_context) = self.render_context.as_mut() else {
            warn!("Reszing without render context");
            return;
        };

        let Some(window_state) = self.state.window_state.as_mut() else {
            warn!("Resizing without window");
            return;
        };

        // for final size, match the canvas physical size
        // I tried doing maxing here but it doesn't work as well.
        let width = physical_size.width
        // .max(WINDOW_MINIMUM_WIDTH)
        ;
        let height = physical_size.height
        // .max(WINDOW_MINIMUM_HEIGHT)
        ;

        // resizing natively
        render_context.resize(width, height);

        // resize webly
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let Some(canvas_element) = document.get_element_by_id(KDR_CANVAS_ID) else {
                warn!("No <canvas> block found");
                return;
            };

            let canvas: web_sys::HtmlCanvasElement = canvas_element.dyn_into().unwrap();

            canvas.set_width(width);
            canvas.set_height(height);
        }

        // updating the ui numbers
        window_state.width = width;
        window_state.height = height;

        // updating aspect here
        self.state.update_fov();
    }
}

impl ApplicationHandler<AppEvent> for App {
    // this is better suited for native run, not for web
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes().with_inner_size(LogicalSize {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
        });

        // need to pass window into web <canvas>
        #[cfg(target_arch = "wasm32")]
        {
            info!("Attaching <canvas> to winit Window");

            // Get the canvas from the DOM
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let canvas_element = document.get_element_by_id(KDR_CANVAS_ID).unwrap();

            // Append canvas to body if it's not already there
            let body = document.body().unwrap();
            if canvas_element.parent_node().is_none() {
                warn!("cannot find <canvas id=\"{}\">", KDR_CANVAS_ID);

                body.append_child(&canvas_element).unwrap();
            }

            let canvas: web_sys::HtmlCanvasElement = canvas_element.dyn_into().unwrap();

            canvas.set_width(DEFAULT_WIDTH);
            canvas.set_height(DEFAULT_HEIGHT);

            if canvas.get_context("webgl2").is_err() {
                warn!("<canvas> does not have webgl2 context");
            }

            // TODO, make sure we have the element first before doing this?
            window_attributes = window_attributes.with_canvas(Some(canvas));
        }

        let window = event_loop.create_window(window_attributes).unwrap();
        let window = Arc::new(window);
        let window_state = WindowState {
            window: window.clone(),
            is_fullscreen: false,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
        };

        // needed
        // Not needed. The goal is that there is a minimum size of winow that user cannot go below
        // On native, this works fine. But on the web, everything is broken after scaling it smaller.
        // window.set_min_inner_size(
        //     winit::dpi::PhysicalSize::new(WINDOW_MINIMUM_WIDTH, WINDOW_MINIMUM_HEIGHT).into(),
        // );

        self.state.window_state = Some(window_state);

        self.event_loop_proxy
            .send_event(AppEvent::CreateRenderContext(window))
            .unwrap_or_else(|_| warn!("Failed to send CreateRenderContext message"));
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        // Mainly for mouse movement
        match event {
            winit::event::DeviceEvent::MouseMotion { delta } => {
                self.state.handle_mouse_movement(delta);
            }
            _ => (),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        // pass event into egui after this match
        //
        match &event {
            WindowEvent::CloseRequested => {
                drop(self.render_context.as_mut());

                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.state.tick();

                let Some(window_state) = self.state.window_state.clone() else {
                    warn!("Redraw requested without window");
                    return;
                };

                let Some(render_context) = &self.render_context else {
                    // warn!("Redraw requested without render context");

                    // need to request redraw even if there's nothing to draw until there's something to draw
                    // definitely not an insanity strat
                    window_state.window().request_redraw();

                    return;
                };

                let mut encoder = render_context
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                let swapchain_texture = render_context.surface_texture();
                let swapchain_view = swapchain_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                {
                    self.state
                        .render_state
                        .render(&render_context, &mut encoder, &swapchain_view);

                    if let Some(ref mut egui_renderer) = self.egui_renderer {
                        // dont even touch these things
                        // serious
                        // let scale_factor = window.scale_factor();
                        // have to use egui pixel per point
                        // the reason is that its "native_pixels_per_point" is already our "window.scale_factor"
                        // egui keeps track of its "zoom_factor", so we don't have to do any extra math
                        // with this, the UI is scaled correctly
                        // however, there is a slight problem with the UIs elements
                        // for example, the crosshair, we have to get the correct dimensions after scaling
                        // basically, anything related to UI must use "context.pixels_per_point"
                        let scale_factor = egui_renderer.context().pixels_per_point();

                        let screen_descriptor = egui_wgpu::ScreenDescriptor {
                            size_in_pixels: [
                                // the screen must match the render target
                                render_context.surface_config().width,
                                render_context.surface_config().height,
                            ],
                            pixels_per_point: scale_factor as f32,
                        };

                        let draw_function = self.state.draw_egui();

                        egui_renderer.render(
                            render_context.device(),
                            render_context.queue(),
                            &mut encoder,
                            &window_state.window(),
                            &swapchain_view,
                            screen_descriptor,
                            draw_function,
                        );
                    } else {
                        warn!("Redraw requested without egui renderer. Skipped rendering egui");
                    };
                }

                let fps = (1.0 / self.state.frame_time).round();

                // rename window based on fps
                window_state.window().set_title(
                    format!(
                        "FPS: {}. Draw calls: {}",
                        fps, self.state.render_state.draw_call
                    )
                    .as_str(),
                );

                // update
                render_context.queue().submit(Some(encoder.finish()));
                swapchain_texture.present();
                window_state.window().request_redraw();

                // polling the states every redraw request
                self.state.file_state_poll();
            }
            // window event inputs are focused so we need to be here
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                self.state
                    .handle_keyboard_input(event.physical_key, event.state);

                self.state.maybe_start_audio_based_on_user_interaction();
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                self.state.handle_mouse_input(button, state);

                self.state.maybe_start_audio_based_on_user_interaction();
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position: _,
            } => {
                // Do not use this event to handle mouse movement.
                // It is confined but it can hit the border.
                // Thus, the position is clamped.
                // Use raw input instead

                // cannot start audio with just cursor move
                // it doesnt qualify as "a user gesture on the page"
                // self.state.maybe_start_audio_based_on_user_interaction();
            }
            WindowEvent::Resized(physical_size) => {
                self.resize(*physical_size);
            }
            _ => (),
        }

        let Some(window_state) = self.state.window_state.clone() else {
            warn!("Passing window events to egui without window");
            return;
        };

        if let Some(egui_renderer) = self.egui_renderer.as_mut() {
            egui_renderer.handle_input(&window_state.window(), &event);
            egui_renderer.context();
        }
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::CreateRenderContext(window) => {
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
            AppEvent::FinishCreateRenderContext(render_context) => {
                info!("Finished creating a render context");

                self.render_context = render_context.into();

                // parsing query (first?) if possible
                #[cfg(target_arch = "wasm32")]
                {
                    self.event_loop_proxy
                        .send_event(AppEvent::ParseLocationSearch)
                        .unwrap_or_else(|_| warn!("Failed to send ParseLocationSearch"));
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
                self.event_loop_proxy
                    .send_event(AppEvent::RequestMapList)
                    .unwrap_or_else(|_| warn!("Failed to send RequestMapList"));

                // also replay list
                self.event_loop_proxy
                    .send_event(AppEvent::RequestReplayList)
                    .unwrap_or_else(|_| warn!("Failed to send RequestReplayList"));
            }
            AppEvent::CreateEgui => {
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
            AppEvent::RequestMap(resource_identifier) => {
                info!("Requesting resources: {:?}", resource_identifier);

                // Attempting to start audio whenever we request to load a map.
                // This is to guaranteed that there are some user actions taken
                // and the browser will kindly let us start audio stream.
                self.event_loop_proxy
                    .send_event(AppEvent::MaybeStartAudioBackEnd)
                    .unwrap_or_else(|_| warn!("Failed to send StartAudio"));

                // when we have a ghost and we wnat to load a map instead, we need to know what is being loaded
                // resource loading goes: ghost -> map
                // so, if we want to load map, that means we have to restart ghost if we play ghost previously
                // however, due to the resource loading order, we cannot just do that
                // so here, we need to know what kind of resource is being loaded to reset data correctly
                if matches!(
                    self.state.file_state.selected_file_type,
                    SelectedFileType::Bsp
                ) {
                    self.state.replay = None;
                }

                #[cfg(target_arch = "wasm32")]
                let Some(resource_provider) = &self.web_resource_provider else {
                    return;
                };

                #[cfg(not(target_arch = "wasm32"))]
                let Some(resource_provider) = &self.native_resource_provider else {
                    return;
                };

                // need to clone resource_provider because it is borrowed from self with &'1 lifetime
                // meanwhile, the spawn_local has 'static lifetime
                // resource_provider is just a url/path, so we are all good in cloning
                let resource_provider = resource_provider.to_owned();

                let event_loop_proxy = self.event_loop_proxy.clone();
                let event_loop_proxy2 = self.event_loop_proxy.clone();

                let send_receive_message =
                    move |res: Result<Resource, ResourceProviderError>| match res {
                        Ok(resource) => {
                            event_loop_proxy
                                .send_event(AppEvent::ReceiveResource(resource))
                                .unwrap_or_else(|_| warn!("cannot send ReceiveResource"));
                        }
                        Err(err) => event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
                    };
                let send_update_fetch_progress = move |v: f32| {
                    event_loop_proxy2
                        .send_event(AppEvent::UpdateFetchProgress(v))
                        .unwrap_or_else(|_| warn!("Cannot send UpdateFetchProgress"));
                };

                self.state
                    .file_state
                    .start_spinner(&resource_identifier.map_name);

                spawn_async(async move {
                    let resource_res = resource_provider
                        .request_map_with_progress(&resource_identifier, move |progress| {
                            send_update_fetch_progress(progress);
                        })
                        .await;

                    send_receive_message(resource_res);
                });
            }
            AppEvent::UpdateFetchProgress(progress_x) => {
                match &mut self.state.file_state.loading_state {
                    LoadingState::Fetching { progress, .. } => {
                        *progress = progress_x;
                    }
                    _ => (),
                }
            }
            AppEvent::ReceiveResource(resource) => {
                info!("Received resources");

                // from fetching to loading
                self.state.file_state.advance_spinner_state();

                let Some(render_context) = &self.render_context else {
                    warn!("Received resources but no render context to render");
                    return;
                };

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_message = move |bsp_resource, world_buffer, skybox_buffer| {
                    event_loop_proxy
                        .send_event(AppEvent::FinishCreateWorld(
                            bsp_resource,
                            world_buffer,
                            skybox_buffer,
                        ))
                        .unwrap_or_else(|_| warn!("Cannot send FinishCreateWorld"));
                };

                let device = render_context.device().clone();
                let queue = render_context.queue().clone();

                // TODO: load resources correctly
                // map assets can be loaded in another thread then we can send an event
                // however, gpu resources need to be on one thread
                // again, this spawn_async needs to lock device and queue
                // which also are needed in the render loop
                // so, the optimization is inside those functions
                // spawn_async(async move {
                let bsp_resource = resource.to_bsp_resource();

                let world_buffer = WorldLoader::load_world(&device, &queue, &bsp_resource);

                let skybox_buffer =
                    SkyboxLoader::load_skybox(&device, &queue, &bsp_resource.skybox);

                send_message(bsp_resource, world_buffer, skybox_buffer);
                // });
            }
            AppEvent::FinishCreateWorld(bsp_resource, world_buffer, skybox_buffer) => {
                self.state.render_state.world_buffer = vec![world_buffer];

                self.state.render_state.skybox = skybox_buffer;

                // inserting audio from bsp resourec
                // but first, need to clear audio that are not part of the common resource
                self.state
                    .audio_resource
                    .retain(|k, _| self.state.other_resources.common_resource.contains_key(k));

                bsp_resource.sound_lookup.into_iter().for_each(|(k, v)| {
                    self.state.audio_resource.insert(k, v);
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

                self.state.file_state.stop_spinner();
            }
            AppEvent::NewFileSelected => {
                self.state.file_state.selected_file_type = SelectedFileType::None;

                let Some(file_path) = &self.state.file_state.selected_file else {
                    warn!("New file is said to be selected but no new file found");
                    return;
                };

                let Some(file_bytes) = &self.state.file_state.selected_file_bytes else {
                    warn!("New file bytes are not loaded");
                    return;
                };

                let file_path = Path::new(file_path);

                if file_path.extension().is_some_and(|ext| ext == "bsp") {
                    info!("Received .bsp from file dialogue");

                    let possible_game_mod = file_path
                        .parent() // maps folder
                        .and_then(|path| path.parent()) // game mod
                        .and_then(|path| path.file_name())
                        .and_then(|osstr| osstr.to_str())
                        // the server needs to understand how to intepret the unknown map
                        .unwrap_or(UNKNOWN_GAME_MOD);

                    let bsp_name = file_path.file_name().unwrap().to_str().unwrap();

                    let resource_identifier = MapIdentifier {
                        map_name: bsp_name.to_string(),
                        game_mod: possible_game_mod.to_string(),
                    };

                    self.event_loop_proxy
                        .send_event(AppEvent::RequestMap(resource_identifier))
                        .unwrap_or_else(|_| {
                            warn!("Cannot send resource request message after file dialogue")
                        });

                    self.state.file_state.selected_file_type = SelectedFileType::Bsp;
                } else {
                    let Ok(ghost_blob) = get_ghost_blob_from_bytes(
                        file_path.display().to_string().as_str(),
                        file_bytes.to_vec(),
                        None,
                    ) else {
                        // if format is not known, take us to the modal to select the format
                        self.event_loop_proxy
                            .send_event(AppEvent::UnknownFormatModal)
                            .unwrap_or_else(|_| warn!("Failed to send UnknownFormatModal"));

                        return;
                    };

                    info!("Received a replay from file dialogue");

                    self.event_loop_proxy
                        .send_event(AppEvent::ReceiveReplayBlob {
                            replay_name: file_path.to_path_buf(),
                            replay_blob: ghost_blob,
                        })
                        .unwrap_or_else(|_| warn!("Cannot send ReceiveReplayBlob"));
                }
            }
            AppEvent::RequestReplay(replay_name) => {
                info!("Requesting replay `{}`", replay_name);

                #[cfg(not(target_arch = "wasm32"))]
                let Some(provider) = &self.native_resource_provider else {
                    warn!("Cannot find native resource provider");

                    self.event_loop_proxy
                        .send_event(AppEvent::ErrorEvent(AppError::NoProvider))
                        .unwrap_or_else(|_| warn!("Failed to send NoProvider"));

                    return;
                };

                #[cfg(target_arch = "wasm32")]
                let Some(provider) = &self.web_resource_provider else {
                    warn!("Cannot find web resource provider");
                    // TODO send to error
                    return;
                };

                let provider = provider.clone();
                let replay_name2 = replay_name.clone();

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_message = move |replay_request_result: Result<
                    GhostBlob,
                    ResourceProviderError,
                >| {
                    match replay_request_result {
                        Ok(ghost_blob) => {
                            event_loop_proxy
                                .send_event(AppEvent::ReceiveReplayBlob {
                                    replay_name: replay_name.clone().into(),
                                    replay_blob: ghost_blob,
                                })
                                .unwrap_or_else(|_| warn!("Failed to send ReceivedGhostRequest"));
                        }
                        Err(op) => event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                                source: op,
                            }))
                            .unwrap_or_else(|_| warn!("Failed to send ErrorEvent")),
                    }
                };

                spawn_async(async move {
                    let what = provider.request_replay(&replay_name2).await;

                    send_message(what);
                });
            }
            AppEvent::ReceiveReplayBlob {
                replay_name,
                replay_blob,
            } => {
                info!("Received replay blob");

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_message = move |identifier, ghost| {
                    event_loop_proxy
                        .send_event(AppEvent::ReceiveReplay(identifier, ghost))
                        .unwrap_or_else(|_| warn!("Failed to send ReceivedGhostRequest"));
                };

                #[cfg(not(target_arch = "wasm32"))]
                let Some(provider) = &self.native_resource_provider else {
                    warn!("Cannot find native resource provider");

                    self.event_loop_proxy
                        .send_event(AppEvent::ErrorEvent(AppError::NoProvider))
                        .unwrap_or_else(|_| warn!("Failed to send NoProvider"));

                    return;
                };

                #[cfg(target_arch = "wasm32")]
                let Some(provider) = &self.web_resource_provider else {
                    warn!("Cannot find web resource provider");
                    // TODO send to error
                    return;
                };

                let provider = provider.clone();

                spawn_async(async move {
                    let Ok((identifier, ghost)) =
                        provider.get_ghost_data(replay_name, replay_blob).await
                    else {
                        warn!("Cannot load ghost data");
                        // TODO send error here
                        return;
                    };

                    send_message(identifier, ghost);
                });

                self.state.file_state.selected_file_type = SelectedFileType::Replay;
            }
            AppEvent::ReceiveReplay(identifier, ghost) => {
                info!("Finished processing .dem. Loading replay");

                self.state.replay = Some(Replay {
                    ghost,
                    playback_mode: ReplayPlaybackMode::Interpolated,
                    last_frame: 0,
                });

                // make sure the user cannot move camera because it is true by default
                self.state.input_state.free_cam = false;

                self.event_loop_proxy
                    .send_event(AppEvent::RequestMap(identifier))
                    .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
            }
            AppEvent::ReceivePostProcessingUpdate(state) => {
                let Some(render_context) = &self.render_context else {
                    warn!("Received ReceivePostProcessingUpdate but no render context available");
                    return;
                };

                let mut pp = render_context.post_processing.write().unwrap();

                pp.get_kuwahara_toggle().map(|res| *res = state.kuwahara);

                pp.get_bloom_toggle().map(|res| *res = state.bloom);

                pp.get_chromatic_aberration_toggle()
                    .map(|res| *res = state.chromatic_aberration);

                pp.get_gray_scale_toggle()
                    .map(|res| *res = state.gray_scale);
            }
            AppEvent::MaybeStartAudioBackEnd => {
                // We can only start audio manager after the user interacts with the site.
                if self.state.audio_state.backend.is_some() {
                    return;
                }

                // FIXME: for some reasons, on native, audio backend automagically starts even though i explicitly
                // program that it can only start with user interaction
                // it is not a bad thing, but this seems inconsistent
                if !self.state.audio_state.able_to_start_backend {
                    return;
                }

                info!("Starting audio manager");

                match AudioBackend::start() {
                    Ok(backend) => {
                        self.state.audio_state.backend = backend.into();
                    }
                    Err(err) => {
                        self.event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::AudioError { source: err }))
                            .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"));
                    }
                }
            }
            AppEvent::RequestCommonResource => {
                info!("Requesting common resource");

                #[cfg(target_arch = "wasm32")]
                {
                    let Some(resource_provider) = &self.web_resource_provider else {
                        warn!("Attempting to request common resource without provider");
                        self.event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::NoProvider))
                            .unwrap_or_else(|_| warn!("Cannot send ErrorEvent"));

                        return;
                    };

                    let resource_provider = resource_provider.clone();
                    let event_loop_proxy = self.event_loop_proxy.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        // can clone this however much we want

                        let common_resource_future = resource_provider.request_common_resource();
                        match common_resource_future.await {
                            Ok(common_res) => event_loop_proxy
                                .send_event(AppEvent::ReceiveCommonResource(common_res))
                                .unwrap_or_else(|_| warn!("Cannot send ReceivedCommonResource")),
                            Err(err) => {
                                warn!("Failed to get common resource");

                                event_loop_proxy
                                    .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                                        source: err,
                                    }))
                                    .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"))
                            }
                        }
                    });
                }
            }
            // this event isnt very necessary but whatever
            AppEvent::ReceiveCommonResource(common_resource) => {
                info!("Received common resource");

                if common_resource.is_empty() {
                    info!("Common resource data is empty");
                }

                // inserting audio from common resource
                common_resource.iter().for_each(|(k, v)| {
                    if !k.ends_with(".wav") {
                        return;
                    }

                    let cursor = Cursor::new(
                        // HOLY
                        v.to_owned(),
                    );

                    let Ok(sound_data) = StaticSoundData::from_cursor(cursor) else {
                        warn!("Failed to parse audio file: `{}`", k);
                        return;
                    };

                    self.state.audio_resource.insert(k.to_string(), sound_data);
                });

                self.state.other_resources.common_resource = common_resource;
            }
            AppEvent::RequestMapList => {
                info!("Requesting map list");

                #[cfg(target_arch = "wasm32")]
                let Some(resource_provider) = &self.web_resource_provider else {
                    warn!("Requesting map list without resource provider");
                    return;
                };

                #[cfg(not(target_arch = "wasm32"))]
                let Some(resource_provider) = &self.native_resource_provider else {
                    warn!("Requesting map list without resource provider");
                    return;
                };

                // need to clone resource_provider because it is borrowed from self with &'1 lifetime
                // meanwhile, the spawn_local has 'static lifetime
                // resource_provider is just a url/path, so we are all good in cloning
                let resource_provider = resource_provider.to_owned();

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_receive_message =
                    move |res: Result<MapList, ResourceProviderError>| match res {
                        Ok(map_list) => {
                            event_loop_proxy
                                .send_event(AppEvent::ReceivedMapList(map_list))
                                .unwrap_or_else(|_| warn!("cannot send ReceivedMapList"));
                        }
                        Err(err) => event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
                    };

                spawn_async(async move {
                    let map_list = resource_provider.request_map_list().await;
                    send_receive_message(map_list);
                });
            }
            AppEvent::ReceivedMapList(map_list) => {
                let mod_count = map_list.len();
                let map_count: usize = map_list.values().map(|k| k.len()).sum();

                info!(
                    "Received a map list of {} maps over {} game mods",
                    map_count, mod_count
                );

                // sorting stuffs so it appears prettier
                // sorting keys
                let mut sorted_game_mod: Vec<_> = map_list.into_iter().collect();

                sorted_game_mod.sort_by(|a, b| a.0.cmp(&b.0));

                // sorting values
                let sorted_map_list: Vec<_> = sorted_game_mod
                    .into_iter()
                    .map(|(game_mod, maps)| {
                        let mut sorted_maps: Vec<_> = maps.into_iter().collect();
                        sorted_maps.sort();

                        (game_mod, sorted_maps)
                    })
                    .collect();

                self.state.other_resources.map_list = sorted_map_list;
            }
            AppEvent::RequestReplayList => {
                info!("Requesting replay list");

                #[cfg(target_arch = "wasm32")]
                let Some(resource_provider) = &self.web_resource_provider else {
                    warn!("Requesting replay list without resource provider");
                    return;
                };

                #[cfg(not(target_arch = "wasm32"))]
                let Some(resource_provider) = &self.native_resource_provider else {
                    warn!("Requesting replay list without resource provider");
                    return;
                };

                let resource_provider = resource_provider.to_owned();

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_receive_message =
                    move |res: Result<ReplayList, ResourceProviderError>| match res {
                        Ok(replay_list) => {
                            event_loop_proxy
                                .send_event(AppEvent::ReceiveReplayList(replay_list))
                                .unwrap_or_else(|_| warn!("cannot send ReceiveReplayList"));
                        }
                        Err(err) => event_loop_proxy
                            .send_event(AppEvent::ErrorEvent(AppError::ProviderError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
                    };

                spawn_async(async move {
                    let replay_list = resource_provider.request_replay_list().await;
                    send_receive_message(replay_list);
                });
            }
            AppEvent::ReceiveReplayList(replay_list) => {
                let replay_count = replay_list.len();

                info!("Received a replay list of {} replays", replay_count);

                self.state.other_resources.replay_list = replay_list;
            }
            #[cfg(target_arch = "wasm32")]
            AppEvent::ParseLocationSearch => {
                let Some(window) = web_sys::window() else {
                    warn!("Parsing location search without window");
                    return;
                };

                let Ok(search) = window.location().search() else {
                    warn!("Parsing loation search without search");
                    return;
                };

                let queries = parse_location_search(&search);

                // prioritize replay before map
                if let Some(replay) = queries.get(REQUEST_REPLAY_ENDPOINT) {
                    info!("Received replay request in query: {}", replay);

                    // audio will only start when user interaction is recorded
                    self.state.audio_state.able_to_start_backend = false;
                    self.state.paused = true;

                    self.event_loop_proxy
                        .send_event(AppEvent::RequestReplay(replay.to_string()))
                        .unwrap_or_else(|_| warn!("Failed to send RequestReplay"));

                    return;
                }

                if let Some(map_name) = queries.get(REQUEST_MAP_ENDPOINT) {
                    if let Some(game_mod) = queries.get(REQUEST_MAP_GAME_MOD_QUERY) {
                        let identifier = MapIdentifier {
                            map_name: map_name.to_string(),
                            game_mod: game_mod.to_string(),
                        };

                        info!("Received map request in query: {:?}", identifier);

                        self.event_loop_proxy
                            .send_event(AppEvent::RequestMap(identifier))
                            .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
                    } else {
                        warn!(
                            "Request map query without game mod query `{}`",
                            REQUEST_MAP_GAME_MOD_QUERY
                        );
                    }
                }
            }
            AppEvent::UnknownFormatModal => {
                self.state.ui_state.unknown_format_modal.enabled = true;
            }
            AppEvent::RequestResize => {
                let Some(window_state) = self.state.window_state.as_ref() else {
                    return;
                };

                let width = window_state.width;
                let height = window_state.height;

                let size = winit::dpi::PhysicalSize { width, height };

                // do not use this
                // this will lock the window min size
                // window.set_min_inner_size(size.into());

                if window_state.window().request_inner_size(size).is_none() {
                    warn!("Request resize failed");
                }

                self.resize(size.clone());
            }
            AppEvent::RequestEnterFullScreen => {
                let Some(window_state) = self.state.window_state.as_mut() else {
                    return;
                };

                let window = window_state.window();

                // for some magical reasons, i don't even need to set the width and height???
                // and when exiting fullscreen, the old resolution is restored
                // thank you winit
                if let Some(monitor) = window.current_monitor() {
                    window.set_fullscreen(
                        winit::window::Fullscreen::Borderless(monitor.into()).into(),
                    );
                }

                // on top of monitor fullscreen, also need canvas fullscreen for the web
                #[cfg(target_arch = "wasm32")]
                {
                    let window = web_sys::window().unwrap();
                    let document = window.document().unwrap();
                    let canvas = document.get_element_by_id(KDR_CANVAS_ID).unwrap();

                    if canvas.request_fullscreen().is_err() {
                        warn!("Failed to request fullscreen");
                    }
                }
            }
            AppEvent::RequestExitFullScreen => {
                let Some(window_state) = self.state.window_state.as_mut() else {
                    return;
                };

                window_state.window().set_fullscreen(None);

                // doesnt need web specific fullscreen exit
                #[cfg(target_arch = "wasm32")]
                {}
            }
            AppEvent::RequestToggleFullScreen => {
                self.state.window_state.as_ref().map(|window_state| {
                    if window_state.is_fullscreen {
                        let _ = self
                            .event_loop_proxy
                            .send_event(AppEvent::RequestEnterFullScreen);
                    } else {
                        let _ = self
                            .event_loop_proxy
                            .send_event(AppEvent::RequestExitFullScreen);
                    }
                });
            }
            AppEvent::ErrorEvent(app_error) => {
                warn!("Error: {}", app_error.to_string());

                // if there is error, just stop with things
                // stop the loading
                self.state.file_state.loading_state = LoadingState::Idle;

                // stop with ghost
                self.state.replay = None;

                // drop all the files
                self.state.file_state.selected_file = None;
                self.state.file_state.selected_file_bytes = None;

                // toast the error
                self.state.ui_state.toaster.warning(app_error.to_string());
            }
        }
    }
}

mod tracing;

/// When the app is initialized, we must already know the resource provider.
///
/// In case of native application, we can feed it in later. That is why the argument is optional.
#[allow(unused)]
pub fn run_kdr(resource_provider_base: Option<String>) {
    tracing::ensure_logging_hooks();

    let Ok(event_loop) = EventLoop::<AppEvent>::with_user_event().build() else {
        warn!("Cannot start event loop");
        warn!("Must restart the app");
        return;
    };

    // Must be Wait if we don't want CPU bottleneck on the web.
    // On native, we draw fast enough that it is basically a synchronous task.
    // However, on the web, we might request redraw even though the other frame isn't done drawing.
    // This leads to abysmal performance.
    event_loop.set_control_flow(ControlFlow::Wait);

    // https://github.com/Jelmerta/Kloenk/blob/main/src/main.rs
    #[cfg(target_arch = "wasm32")]
    {
        let app = App::new(resource_provider_base, &event_loop);
        event_loop.spawn_app(app);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new(resource_provider_base, &event_loop);
        event_loop.run_app(&mut app).unwrap();
    }
}
