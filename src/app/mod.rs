use std::sync::Arc;

// can use this for both native and web
use web_time::{Duration, Instant};

use interaction::Key;
use pollster::FutureExt;
use replay::Replay;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowAttributesExtWebSys;

mod interaction;
mod replay;

use crate::{
    ghost::get_ghost,
    loader::{ResourceIdentifier, ResourceProvider, native::NativeResourceProvider},
    renderer::{RenderContext, RenderState, camera::Camera, world_buffer::WorldLoader},
};

const WINDOW_WIDTH: i32 = 1280;
const WINDOW_HEIGHT: i32 = 960;

// TODO restructure this
// app might still be a general "app" that both native and web points to
// the difference might be the "window" aka where the canvas is
// though not sure how to handle loop, that is for my future self
struct App {
    graphic_context: Option<RenderContext>,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            graphic_context: Default::default(),
            window: Default::default(),
            time: Duration::ZERO,
            last_time: Instant::now(),
            frame_time: 1.,
            render_state: Default::default(),
            keys: Key::empty(),
            mouse_right_hold: false,
            ghost: None,
        }
    }
}

impl App {
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

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let mut window_attributes = Window::default_attributes().with_inner_size(LogicalSize {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        });

        // need to pass window into web <canvas>
        #[cfg(target_arch = "wasm32")]
        {
            // Get the canvas from the DOM
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let canvas_element = document.get_element_by_id("canvas").unwrap();

            // Append canvas to body if it's not already there
            let body = document.body().unwrap();
            if canvas_element.parent_node().is_none() {
                body.append_child(&canvas_element).unwrap();
            }

            window_attributes =
                window_attributes.with_canvas(Some(canvas_element.dyn_into().unwrap()));
        }

        let window = event_loop.create_window(window_attributes).unwrap();
        let window = Arc::new(window);

        let render_context = pollster::block_on(RenderContext::new(window.clone()));

        // load ghost and then bsp?
        {
            let resource_loader = NativeResourceProvider::new("/home/khang/bxt/game_isolated/");

            let demo_path =
                "/home/khang/bxt/game_isolated/cstrike/cc1036/c21_malle_enjoy_Mrjuice_0052.85.dem";
            let demo_bytes = std::fs::read(demo_path).unwrap();
            let ghost = get_ghost(demo_path, &demo_bytes).unwrap();
            let resource_identifier = ResourceIdentifier {
                map_name: ghost.map_name.to_owned(),
                game_mod: ghost.game_mod.to_owned(),
            };

            let resource = resource_loader
                .get_resource(&resource_identifier)
                .block_on()
                .unwrap()
                .to_bsp_resource();

            let world_buffer = WorldLoader::load_world(
                &render_context.device(),
                &render_context.queue(),
                &resource,
            );

            self.render_state.world_buffer = vec![world_buffer];

            self.render_state.skybox = render_context.skybox_loader.load_skybox(
                &render_context.device(),
                &render_context.queue(),
                &resource.skybox,
            );

            self.ghost = Some(Replay {
                ghost,
                playback_mode: replay::ReplayPlaybackMode::RealTime,
            });
        }

        self.render_state.camera = Camera::default();
        // now do stuffs

        self.window = Some(window);
        self.graphic_context = render_context.into();
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
                drop(self.graphic_context.as_mut());

                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.tick();

                self.graphic_context.as_mut().map(|res| {
                    // rendering world
                    res.render(&mut self.render_state);

                    // TODO rendering GUI with egui or somethin??
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
}

// "rustc: cannot declare a non-inline module inside a block unless it has a path attribute"
#[cfg(not(target_arch = "wasm32"))]
mod tracing;

pub fn run_kdr() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        tracing::ensure_logging_hooks();
    }

    let event_loop = EventLoop::new().unwrap();
    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut a = App::default();
    event_loop.run_app(&mut a).unwrap();
}
