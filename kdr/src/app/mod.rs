use std::{path::Path, sync::Arc};

use ::tracing::{info, warn};
use constants::{WINDOW_HEIGHT, WINDOW_WIDTH};
// pollster for native use only
#[cfg(not(target_arch = "wasm32"))]
use pollster::FutureExt;

// can use this for both native and web
use web_time::{Duration, Instant};

use interaction::Key;
use replay::Replay;
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
mod interaction;
mod replay;

use crate::renderer::{RenderContext, RenderState, camera::Camera, world_buffer::WorldLoader};
use loader::{Resource, ResourceIdentifier, ResourceProvider, error::ResourceProviderError};

#[cfg(not(target_arch = "wasm32"))]
use loader::native::NativeResourceProvider;

#[cfg(target_arch = "wasm32")]
use loader::web::WebResourceProvider;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Problems with resource provider: {source}")]
    ProviderError { source: ResourceProviderError },
}

pub enum CustomEvent {
    CreateRenderContext(Arc<Window>),
    FinishCreateRenderContext(RenderContext),
    RequestResource(ResourceIdentifier),
    ReceiveResource(Resource),
    ErrorEvent(AppError),
}

// TODO restructure this
// app might still be a general "app" that both native and web points to
// the difference might be the "window" aka where the canvas is
// though not sure how to handle loop, that is for my future self
struct App {
    render_context: Option<RenderContext>,
    window: Option<Arc<Window>>,

    // time
    time: Duration,
    last_time: Instant,
    frame_time: f32,

    // stuffs
    // TODO future render state might need to be optional so that we can reload map or something?? not sure
    // like we can start the app with nothing going on and hten drag and rdop the map 🤤
    render_state: RenderState,
    // optional ghost because we might just want to render bsp
    ghost: Option<Replay>,

    // input
    keys: Key,
    mouse_right_hold: bool,

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

        Self {
            render_context: Default::default(),
            window: Default::default(),
            time: Duration::ZERO,
            last_time: Instant::now(),
            frame_time: 1.,
            render_state: Default::default(),
            keys: Key::empty(),
            mouse_right_hold: false,
            ghost: None,
            #[cfg(not(target_arch = "wasm32"))]
            native_resource_provider: provider,
            #[cfg(target_arch = "wasm32")]
            web_resource_provider: provider,
            event_loop_proxy: event_loop.create_proxy(),
        }
    }

    /// Tick function modifies everything in the app including the rendering state.
    ///
    /// If there is any event going on every frame, it should be contained in this function.
    pub fn tick(&mut self) {
        self.delta_update();

        self.interaction_tick();
        self.replay_tick();
    }

    fn delta_update(&mut self) {
        let now = Instant::now();
        let diff = now.duration_since(self.last_time);
        self.frame_time = diff.as_secs_f32();
        self.last_time = now;
        self.time += diff;
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

        self.window = Some(window.clone());
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
                self.handle_mouse_movement(delta);
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
        match event {
            WindowEvent::CloseRequested => {
                drop(self.render_context.as_mut());

                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.tick();

                self.render_context.as_mut().map(|res| {
                    // rendering world
                    res.render(&mut self.render_state);
                });

                self.window.as_mut().map(|window| {
                    let fps = (1.0 / self.frame_time).round();

                    // rename window based on fps
                    window.set_title(
                        format!("FPS: {}. Draw calls: {}", fps, self.render_state.draw_call)
                            .as_str(),
                    );
                    // update
                    window.request_redraw();
                });
            }
            // window event inputs are focused so we need to be here
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } => {
                self.handle_keyboard_input(event.physical_key, event.state);
            }
            WindowEvent::MouseInput {
                device_id: _,
                state,
                button,
            } => {
                self.handle_mouse_input(button, state);
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
                info!("Finish creating a render context");

                self.render_context = render_context.into();

                self.event_loop_proxy
                    .send_event(CustomEvent::RequestResource(ResourceIdentifier {
                        map_name: "boot_camp.bsp".to_string(),
                        game_mod: "valve".to_string(),
                    }))
                    .unwrap_or_else(|_| warn!("cannot send debug request"));
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

                // let resource_identifier = ResourceIdentifier {
                //     map_name: "c1a0.bsp".to_owned(),
                //     game_mod: "valve".to_owned(),
                // };

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

                self.render_state.world_buffer = vec![world_buffer];

                self.render_state.skybox = render_context.skybox_loader.load_skybox(
                    &render_context.device(),
                    &render_context.queue(),
                    &bsp_resource.skybox,
                );

                // self.ghost = Some(Replay {
                //     ghost,
                //     playback_mode: replay::ReplayPlaybackMode::RealTime,
                // });

                self.render_state.camera = Camera::default();

                self.ghost = None;
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
        #[cfg(target_arch = "wasm32")]
        {
            browser_console_log("cannto start evetn loop");
        }
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
