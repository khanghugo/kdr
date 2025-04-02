use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use movement::Key;
use pollster::FutureExt;
use tracing::ensure_logging_hooks;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

mod movement;
mod tracing;

use crate::{
    loader::{ResourceProvider, native::NativeResourceProvider},
    renderer::{RenderContext, RenderState, camera::Camera, world_buffer::WorldLoader},
};

const WINDOW_WIDTH: i32 = 1440;
const WINDOW_HEIGHT: i32 = 900;

struct App {
    graphic_context: Option<RenderContext>,
    window: Option<Arc<Window>>,

    // time
    time: Duration,
    last_time: Instant,
    frame_time: f32,

    // stuffs
    render_state: RenderState,

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
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_inner_size(LogicalSize {
                width: WINDOW_WIDTH,
                height: WINDOW_HEIGHT,
            }))
            .unwrap();
        let window = Arc::new(window);

        let render_context = pollster::block_on(RenderContext::new(window.clone()));

        // load bsp
        {
            let resource_loader = NativeResourceProvider::new("/home/khang/bxt/game_isolated/");
            let resource = resource_loader
                .get_resource(&crate::loader::ResourceIdentifier {
                    map_name: "c1a0.bsp".to_string(),
                    game_mod: "cstrike".to_string(),
                })
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

                self.graphic_context
                    .as_mut()
                    .map(|res| res.render(&mut self.render_state));

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

pub fn bsp() {
    ensure_logging_hooks();

    let event_loop = EventLoop::new().unwrap();
    // When the current loop iteration finishes, immediately begin a new
    // iteration regardless of whether or not new events are available to
    // process. Preferred for applications that want to render as fast as
    // possible, like games.
    event_loop.set_control_flow(ControlFlow::Poll);

    // When the current loop iteration finishes, suspend the thread until
    // another event arrives. Helps keeping CPU utilization low if nothing
    // is happening, which is preferred if the application might be idling in
    // the background.
    // event_loop.set_control_flow(ControlFlow::Wait);

    // let mut app = HelloTriangle::new(event_loop);

    let mut a = App::default();
    event_loop.run_app(&mut a).unwrap();
}
