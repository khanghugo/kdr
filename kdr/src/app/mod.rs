use std::{path::Path, sync::Arc};

use ::tracing::{info, warn};
use constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
use ghost::GhostInfo;
use state::{
    AppState,
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
use crate::utils::browser_console_log;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowAttributesExtWebSys};
pub mod constants;
mod state;

use crate::renderer::{
    RenderContext, camera::Camera, egui_renderer::EguiRenderer, world_buffer::WorldLoader,
};
use loader::{Resource, ResourceIdentifier, ResourceProvider, error::ResourceProviderError};

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
}

pub enum CustomEvent {
    CreateRenderContext(Arc<Window>),
    FinishCreateRenderContext(RenderContext),
    CreateEgui,
    RequestResource(ResourceIdentifier),
    ReceiveResource(Resource),
    NewFileSelected,
    ReceivedGhostRequest(ResourceIdentifier, GhostInfo),
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
                    .unwrap_or_else(|_| warn!("cannot send creating egui renderer request"));

                // self.event_loop_proxy
                //     .send_event(CustomEvent::RequestResource(ResourceIdentifier {
                //         map_name: "chk_section.bsp".to_string(),
                //         game_mod: "cstrike_downloads".to_string(),
                //     }))
                //     .unwrap_or_else(|_| warn!("cannot send debug request"));
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

                self.state.render_state.camera = Camera::default();

                // ghost is loaded first so dont do this
                // self.state.ghost = None;
            }
            CustomEvent::NewFileSelected => {
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
                        .unwrap_or("unknown");

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
                } else if file_extension == "dem" {
                    info!("Received .dem from file dialogue. Processing .dem");

                    let event_loop_proxy = self.event_loop_proxy.clone();
                    let send_message = move |identifier, ghost| {
                        event_loop_proxy
                            .send_event(CustomEvent::ReceivedGhostRequest(identifier, ghost))
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
                } else {
                    warn!("Bad resource: {}", file_path.display());
                }
            }
            CustomEvent::ReceivedGhostRequest(identifier, ghost) => {
                info!("Finished processing .dem. Loading replay");

                self.state.replay = Some(Replay {
                    ghost,
                    playback_mode: ReplayPlaybackMode::RealTime,
                });

                self.state.time = 0.;

                self.event_loop_proxy
                    .send_event(CustomEvent::RequestResource(identifier))
                    .unwrap_or_else(|_| warn!("Failed to send RequestResource"));
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

    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

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
