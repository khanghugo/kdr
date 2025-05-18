use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use ::tracing::{info, warn};
#[cfg(target_arch = "wasm32")]
use common::KDR_CANVAS_ID;

use constants::{DEFAULT_HEIGHT, DEFAULT_WIDTH};
use ghost::{GhostBlob, GhostInfo};
use state::{
    AppState, audio::AudioStateError, overlay::control_panel::PostProcessingControlState,
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
mod user_event;

use crate::renderer::{
    EguiRenderer, RenderContext, skybox::SkyboxBuffer, world_buffer::WorldStaticBuffer,
};
use loader::{
    MapIdentifier, MapList, ReplayList, Resource, ResourceMap, bsp_resource::BspResource,
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

    #[error("Unknown file format: {file_name}")]
    UnknownFile { file_name: String },

    #[error("Cannot connect to WebSocket server")]
    WebSocketConnection,
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
    FinishCreateWorld(BspResource, WorldStaticBuffer, Option<SkyboxBuffer>),
    UpdateFetchProgress(f32),
    #[cfg(target_arch = "wasm32")]
    ParseLocationSearch,
    UnknownFormatModal,
    RequestResize,
    RequestEnterFullScreen,
    RequestExitFullScreen,
    RequestToggleFullScreen,
    #[allow(unused)]
    CreatePuppeteerConnection,
    ErrorEvent(AppError),
}

// TODO restructure this
// app might still be a general "app" that both native and web points to
// the difference might be the "window" aka where the canvas is
// though not sure how to handle loop, that is for my future self
struct App {
    options: RunKDROptions,

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
        options: RunKDROptions,
        event_loop: &winit::event_loop::EventLoop<AppEvent>,
    ) -> Self {
        let provider = options
            .resource_provider_base
            .as_ref()
            .and_then(|provider_uri| {
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
        let state = AppState::new(event_loop_proxy.clone());

        Self {
            options: options.clone(),
            render_context: None,
            egui_renderer: None,
            state,
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
                        "FPS: {}. Draw calls: {}. Time: {:.2}",
                        fps, self.state.render_state.draw_call, self.state.time
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

    fn user_event(&mut self, event_loop: &winit::event_loop::ActiveEventLoop, event: AppEvent) {
        self._user_event(event_loop, event);
    }
}

mod tracing;

#[derive(Debug, Clone)]
pub struct RunKDROptions {
    // the reason why this is optional is because of native setup
    // where people don't have folder selected
    pub resource_provider_base: Option<String>,
    pub websocket_url: Option<String>,
    pub fetch_map_list: bool,
    pub fetch_replay_list: bool,
}

/// When the app is initialized, we must already know the resource provider.
///
/// In case of native application, we can feed it in later. That is why the argument is optional.
#[allow(unused)]
pub fn run_kdr(options: RunKDROptions) {
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
        let app = App::new(options, &event_loop);
        event_loop.spawn_app(app);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new(options, &event_loop);
        event_loop.run_app(&mut app).unwrap();
    }
}
