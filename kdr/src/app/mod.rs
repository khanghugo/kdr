use std::{path::Path, sync::Arc};

use ::tracing::{info, warn};
use common::{UNKNOWN_GAME_MOD, vec3};
use constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
use ghost::GhostInfo;
use state::{
    AppState, InputFileType,
    audio::{AudioBackend, AudioStateError},
    overlay::control_panel::PostProcessingControlState,
    replay::{Replay, ReplayPlaybackMode},
};
// pollster for native use only
#[cfg(not(target_arch = "wasm32"))]
use pollster::FutureExt;

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

use crate::renderer::{
    RenderContext, RenderOptions, camera::Camera, egui_renderer::EguiRenderer,
    world_buffer::WorldLoader,
};
use loader::{
    MapList, Resource, ResourceIdentifier, ResourceMap, ResourceProvider,
    error::ResourceProviderError,
};

#[cfg(not(target_arch = "wasm32"))]
use loader::native::NativeResourceProvider;

#[cfg(target_arch = "wasm32")]
use loader::web::WebResourceProvider;

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
}

pub enum CustomEvent {
    CreateRenderContext(Arc<Window>),
    FinishCreateRenderContext(RenderContext),
    CreateEgui,
    RequestResource(ResourceIdentifier),
    ReceiveResource(Resource),
    NewFileSelected,
    ReceiveGhostRequest(ResourceIdentifier, GhostInfo),
    ReceivePostProcessingUpdate(PostProcessingControlState),
    MaybeStartAudioBackEnd,
    RequestCommonResource,
    #[allow(dead_code)]
    ReceivedCommonResource(ResourceMap),
    RequestMapList,
    ReceivedMapList(MapList),
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
    event_loop_proxy: EventLoopProxy<CustomEvent>,
}

impl App {
    pub fn new(
        provider_uri: Option<String>,
        event_loop: &winit::event_loop::EventLoop<CustomEvent>,
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
}

impl ApplicationHandler<CustomEvent> for App {
    // this is better suited for native run, not for web
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes().with_inner_size(LogicalSize {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        });

        // need to pass window into web <canvas>
        #[cfg(target_arch = "wasm32")]
        {
            info!("Attaching <canvas> to winit Window");
            // Get the canvas from the DOM
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let canvas_element = document.get_element_by_id("canvas").unwrap();

            // Append canvas to body if it's not already there
            let body = document.body().unwrap();
            if canvas_element.parent_node().is_none() {
                warn!("cannot find <canvas id=\"canvas\">");

                body.append_child(&canvas_element).unwrap();
            }

            let canvas: web_sys::HtmlCanvasElement = canvas_element.dyn_into().unwrap();

            if canvas.get_context("webgl2").is_err() {
                warn!("<canvas> does not have webgl2 context");
            }

            // TODO, make sure we have the element first before doing this?
            window_attributes = window_attributes.with_canvas(Some(canvas));
        }

        let window = event_loop.create_window(window_attributes).unwrap();
        let window = Arc::new(window);

        self.state.window = Some(window.clone());
        self.event_loop_proxy
            .send_event(CustomEvent::CreateRenderContext(window))
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

                let Some(window) = self.state.window.clone() else {
                    warn!("Redraw requested without window");
                    return;
                };

                let Some(ref mut render_context) = self.render_context else {
                    warn!("Redraw requested without render context");

                    // need to request redraw even if there's nothing to draw until there's something to draw
                    // definitely not an insanity strat
                    window.request_redraw();

                    return;
                };

                let mut encoder = render_context
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                let swapchain_texture = render_context.surface_texture();
                let swapchain_view = swapchain_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [
                        render_context.surface_config().width,
                        render_context.surface_config().height,
                    ],
                    pixels_per_point: window.scale_factor() as f32,
                };

                {
                    render_context.render(
                        &mut self.state.render_state,
                        &mut encoder,
                        &swapchain_view,
                    );

                    if let Some(ref mut egui_renderer) = self.egui_renderer {
                        let draw_function = self.state.draw_egui();

                        egui_renderer.render(
                            render_context.device(),
                            render_context.queue(),
                            &mut encoder,
                            &window,
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
                window.set_title(
                    format!(
                        "FPS: {}. Draw calls: {}",
                        fps, self.state.render_state.draw_call
                    )
                    .as_str(),
                );

                // update
                render_context.queue().submit(Some(encoder.finish()));
                swapchain_texture.present();
                window.request_redraw();

                // polling the states every redraw request
                self.state.state_poll();
            }
            // window event inputs are focused so we need to be here
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                self.state
                    .handle_keyboard_input(event.physical_key, event.state);
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                self.state.handle_mouse_input(button, state);
            }
            WindowEvent::CursorMoved {
                device_id: _,
                position: _,
            } => {
                // Do not use this event to handle mouse movement.
                // It is confined but it can hit the border.
                // Thus, the position is clamped.
                // Use raw input instead
            }
            _ => (),
        }

        let Some(window) = self.state.window.clone() else {
            warn!("Passing window events to egui without window");
            return;
        };

        if let Some(egui_renderer) = self.egui_renderer.as_mut() {
            // let a = egui_renderer.;
            egui_renderer.handle_input(&window, &event);
            egui_renderer.context();
        }
    }

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: CustomEvent) {
        match event {
            CustomEvent::CreateRenderContext(window) => {
                info!("Creating a render context");

                let render_context_future = RenderContext::new(window.clone());

                let event_loop_proxy = self.event_loop_proxy.clone();
                let send_message = move |render_context: RenderContext| {
                    event_loop_proxy
                        .send_event(CustomEvent::FinishCreateRenderContext(render_context))
                        .unwrap_or_else(|_| warn!("Failed to send FinishCreateRenderContext"));
                };

                #[cfg(target_arch = "wasm32")]
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        let render_context = render_context_future.await;
                        send_message(render_context);
                    });
                }

                // we can do it like wasm where we send message and what not?
                // TOOD maybe follow the same thing in wasm so things look samey everywhere
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let render_context = render_context_future.block_on();
                    send_message(render_context)
                }
            }
            CustomEvent::FinishCreateRenderContext(render_context) => {
                info!("Finished creating a render context");

                self.render_context = render_context.into();

                // create egui after render context is done initializing
                self.event_loop_proxy
                    .send_event(CustomEvent::CreateEgui)
                    .unwrap_or_else(|_| warn!("Failed to send CreateEgui"));

                // request common resource at the same time as well because why not
                self.event_loop_proxy
                    .send_event(CustomEvent::RequestCommonResource)
                    .unwrap_or_else(|_| warn!("Failed to send RequestCommonResource"));

                // also requesting map list
                self.event_loop_proxy
                    .send_event(CustomEvent::RequestMapList)
                    .unwrap_or_else(|_| warn!("Failed to send RequestMapList"));
            }
            CustomEvent::CreateEgui => {
                info!("Creating egui renderer");

                let Some(window) = self.state.window.clone() else {
                    warn!("Window is not initialized. Cannot create egui renderer");
                    return;
                };

                let Some(ref render_context) = self.render_context else {
                    warn!("Render context is not initialized. Cannot create egui renderer");
                    return;
                };

                let egui_renderer = EguiRenderer::new(
                    render_context.device(),
                    render_context.swapchain_format().clone(),
                    None,
                    1,
                    &window,
                );

                self.egui_renderer = egui_renderer.into();

                info!("Finished creating egui renderer");
            }
            CustomEvent::RequestResource(resource_identifier) => {
                info!("Requesting resources: {:?}", resource_identifier);

                // Attempting to start audio whenever we request to load a map.
                // This is to guaranteed that there are some user actions taken
                // and the browser will kindly let us start audio stream.
                self.event_loop_proxy
                    .send_event(CustomEvent::MaybeStartAudioBackEnd)
                    .unwrap_or_else(|_| warn!("Failed to send StartAudio"));

                // when we have a ghost and we wnat to load a map instead, we need to know what is being loaded
                // resource loading goes: ghost -> map
                // so, if we want to load map, that means we have to restart ghost if we play ghost previously
                // however, due to the resource loading order, we cannot just do that
                // so here, we need to know what kind of resource is being loaded to reset data correctly
                if matches!(self.state.input_file_type, InputFileType::Bsp) {
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
                let send_receive_message =
                    move |res: Result<Resource, ResourceProviderError>| match res {
                        Ok(resource) => {
                            event_loop_proxy
                                .send_event(CustomEvent::ReceiveResource(resource))
                                .unwrap_or_else(|_| warn!("cannot send ReceiveResource"));
                        }
                        Err(err) => event_loop_proxy
                            .send_event(CustomEvent::ErrorEvent(AppError::ProviderError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
                    };

                #[cfg(target_arch = "wasm32")]
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        // resource identifier stays in here as well so no lifetime shenanigans can happen
                        let resource_res =
                            resource_provider.get_resource(&resource_identifier).await;
                        send_receive_message(resource_res);
                    });
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let resource = resource_provider
                        .get_resource(&resource_identifier)
                        .block_on();

                    send_receive_message(resource);
                }
            }
            CustomEvent::ReceiveResource(resource) => {
                info!("Received resources");

                let Some(render_context) = &mut self.render_context else {
                    warn!("Received resources but no render context to render");
                    return;
                };

                let bsp_resource = resource.to_bsp_resource();

                let world_buffer = WorldLoader::load_world(
                    &render_context.device(),
                    &render_context.queue(),
                    &bsp_resource,
                );

                self.state.render_state.world_buffer = vec![world_buffer];

                self.state.render_state.skybox = render_context.skybox_loader.load_skybox(
                    &render_context.device(),
                    &render_context.queue(),
                    &bsp_resource.skybox,
                );

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

                self.state.render_state.render_options = RenderOptions::default();

                // reset file input tpye
                self.state.input_file_type = InputFileType::None;
            }
            CustomEvent::NewFileSelected => {
                self.state.input_file_type = InputFileType::None;

                let Some(file_path) = &self.state.selected_file else {
                    warn!("New file is said to be selected but no new file found");
                    return;
                };

                let Some(file_bytes) = &self.state.selected_file_bytes else {
                    warn!("New file bytes are not loaded");
                    return;
                };

                let file_path = Path::new(file_path);

                let Some(file_extension) = file_path.extension() else {
                    warn!("New file does not contain an extension");
                    return;
                };

                if file_extension == "bsp" {
                    info!("Received .bsp from file dialogue");

                    let possible_game_mod = file_path
                        .parent() // maps folder
                        .and_then(|path| path.parent()) // game mod
                        .and_then(|path| path.file_name())
                        .and_then(|osstr| osstr.to_str())
                        // the server needs to understand how to intepret the unknown map
                        .unwrap_or(UNKNOWN_GAME_MOD);

                    let bsp_name = file_path.file_name().unwrap().to_str().unwrap();

                    let resource_identifier = ResourceIdentifier {
                        map_name: bsp_name.to_string(),
                        game_mod: possible_game_mod.to_string(),
                    };

                    self.event_loop_proxy
                        .send_event(CustomEvent::RequestResource(resource_identifier))
                        .unwrap_or_else(|_| {
                            warn!("Cannot send resource request message after file dialogue")
                        });

                    self.state.input_file_type = InputFileType::Bsp;
                } else if file_extension == "dem" {
                    info!("Received .dem from file dialogue. Processing .dem");

                    let event_loop_proxy = self.event_loop_proxy.clone();
                    let send_message = move |identifier, ghost| {
                        event_loop_proxy
                            .send_event(CustomEvent::ReceiveGhostRequest(identifier, ghost))
                            .unwrap_or_else(|_| warn!("Failed to send ReceivedGhostRequest"));
                    };

                    #[cfg(not(target_arch = "wasm32"))]
                    let Some(provider) = &self.native_resource_provider else {
                        warn!("Cannot find native resource provider");

                        self.event_loop_proxy
                            .send_event(CustomEvent::ErrorEvent(AppError::NoProvider))
                            .unwrap_or_else(|_| warn!("Failed to send NoProvider"));

                        return;
                    };

                    #[cfg(target_arch = "wasm32")]
                    let Some(provider) = &self.web_resource_provider else {
                        warn!("Cannot find web resource provider");
                        // TODO send to error
                        return;
                    };

                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let Ok((identifier, ghost)) =
                            provider.get_ghost_data(file_path, file_bytes).block_on()
                        else {
                            warn!("Cannot load ghost data");
                            // TODO send error here
                            return;
                        };

                        send_message(identifier, ghost);
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        let provider = provider.clone();
                        let file_path = file_path.to_owned();
                        let file_bytes = file_bytes.to_owned();

                        wasm_bindgen_futures::spawn_local(async move {
                            let Ok((identifier, ghost)) =
                                provider.get_ghost_data(file_path, &file_bytes).await
                            else {
                                warn!("Cannot load ghost data");
                                // TODO send error here
                                return;
                            };

                            send_message(identifier, ghost);
                        });
                    }

                    self.state.input_file_type = InputFileType::Replay;
                } else {
                    warn!("Bad resource: {}", file_path.display());
                }
            }
            CustomEvent::ReceiveGhostRequest(identifier, ghost) => {
                info!("Finished processing .dem. Loading replay");

                ghost
                    .frames
                    .iter()
                    .filter_map(|frame| frame.extras.as_ref())
                    .filter(|extra| !extra.sound.is_empty())
                    .for_each(|extra| println!("{:?}", extra.sound));

                self.state.replay = Some(Replay {
                    ghost,
                    playback_mode: ReplayPlaybackMode::Interpolated,
                });

                // resetting the time, obviously
                self.state.time = 0.;

                // make sure the user cannot move camera because it is true by default
                self.state.input_state.free_cam = false;

                self.event_loop_proxy
                    .send_event(CustomEvent::RequestResource(identifier))
                    .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
            }
            CustomEvent::ReceivePostProcessingUpdate(state) => {
                let Some(renderer) = &mut self.render_context else {
                    warn!("Received ReceivePostProcessingUpdate but no render context available");
                    return;
                };

                renderer
                    .post_processing
                    .get_kuwahara_toggle()
                    .map(|res| *res = state.kuwahara);

                renderer
                    .post_processing
                    .get_bloom_toggle()
                    .map(|res| *res = state.bloom);

                renderer
                    .post_processing
                    .get_chromatic_aberration_toggle()
                    .map(|res| *res = state.chromatic_aberration);

                renderer
                    .post_processing
                    .get_gray_scale_toggle()
                    .map(|res| *res = state.gray_scale);
            }
            CustomEvent::MaybeStartAudioBackEnd => {
                // We can only start audio manager after the user interacts with the site.
                if self.state.audio_state.backend.is_some() {
                    return;
                }

                info!("Starting audio manager");

                match AudioBackend::start() {
                    Ok(backend) => {
                        self.state.audio_state.backend = backend.into();
                    }
                    Err(err) => {
                        self.event_loop_proxy
                            .send_event(CustomEvent::ErrorEvent(AppError::AudioError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"));
                    }
                }
            }
            CustomEvent::RequestCommonResource => {
                info!("Requesting common resource");

                #[cfg(target_arch = "wasm32")]
                {
                    let Some(resource_provider) = &self.web_resource_provider else {
                        warn!("Attempting to request common resource without provider");
                        self.event_loop_proxy
                            .send_event(CustomEvent::ErrorEvent(AppError::NoProvider))
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
                                .send_event(CustomEvent::ReceivedCommonResource(common_res))
                                .unwrap_or_else(|_| warn!("Cannot send ReceivedCommonResource")),
                            Err(err) => {
                                warn!("Failed to get common resource");

                                event_loop_proxy
                                    .send_event(CustomEvent::ErrorEvent(AppError::ProviderError {
                                        source: err,
                                    }))
                                    .unwrap_or_else(|_| warn!("Failed to send ErrorEvent"))
                            }
                        }
                    });
                }
            }
            // this event isnt very necessary but whatever
            CustomEvent::ReceivedCommonResource(common_resource) => {
                info!("Received common resource");

                if common_resource.is_empty() {
                    info!("Common resource data is empty");
                }

                self.state.other_resources.common_resource = common_resource;
            }
            CustomEvent::RequestMapList => {
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
                                .send_event(CustomEvent::ReceivedMapList(map_list))
                                .unwrap_or_else(|_| warn!("cannot send ReceivedMapList"));
                        }
                        Err(err) => event_loop_proxy
                            .send_event(CustomEvent::ErrorEvent(AppError::ProviderError {
                                source: err,
                            }))
                            .unwrap_or_else(|_| warn!("cannot send AppError::ProviderError")),
                    };

                #[cfg(target_arch = "wasm32")]
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        let map_list = resource_provider.get_map_list().await;
                        send_receive_message(map_list);
                    });
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    let map_list = resource_provider.get_map_list().block_on();
                    send_receive_message(map_list);
                }
            }
            CustomEvent::ReceivedMapList(map_list) => {
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
            CustomEvent::ErrorEvent(app_error) => {
                warn!("Error: {}", app_error.to_string());
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

    let Ok(event_loop) = EventLoop::<CustomEvent>::with_user_event().build() else {
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
