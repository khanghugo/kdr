use std::{path::Path, sync::Arc, time::Instant};

use movement::Key;
use tracing::ensure_logging_hooks;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::Window,
};

mod movement;
mod tracing;

use crate::{
    bsp_loader::get_bsp_resources,
    renderer::{RenderContext, RenderState, camera::Camera, world_buffer::WorldLoader},
};

const FILE: &str = "./examples/textures.obj";
// const BSP_FILE: &str = "/home/khang/bxt/game_isolated/cstrike_downloads/maps/trans_compile.bsp";
// const BSP_FILE: &str = "./examples/chk_section.bsp";
// const BSP_FILE: &str = "/home/khang/bxt/game_isolated/cstrike_downloads/maps/arte_drift.bsp";
// const BSP_FILE: &str = "/home/khang/bxt/_game_native/cstrike_downloads/maps/hb_MART.bsp";
const BSP_FILE: &str = "/home/khang/bxt/game_isolated/cstrike_downloads/maps/chk_section.bsp";
// const BSP_FILE: &str = "/home/khang/bxt/game_isolated/cstrike_downloads/maps/surf_cyberwave.bsp";

struct App {
    graphic_context: Option<RenderContext>,
    window: Option<Arc<Window>>,

    // time
    last_time: Instant,
    frame_time: f32,

    // stuffs
    render_state: RenderState,

    // input
    keys: Key,
}

impl Default for App {
    fn default() -> Self {
        Self {
            graphic_context: Default::default(),
            window: Default::default(),
            last_time: Instant::now(),
            frame_time: 1.,
            render_state: Default::default(),
            keys: Key::empty(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes().with_inner_size(LogicalSize {
                width: 1440,
                height: 900,
            }))
            .unwrap();
        let window = Arc::new(window);

        let render_context = pollster::block_on(RenderContext::new(window.clone()));

        // load bsp
        {
            let bsp = bsp::Bsp::from_file(BSP_FILE).unwrap();
            let resource = get_bsp_resources(bsp, Path::new(BSP_FILE));

            let world_buffer = WorldLoader::load_world(
                &render_context.device(),
                &render_context.queue(),
                &resource,
            );

            self.render_state.world_buffer = vec![world_buffer];
        }

        self.render_state.camera = Camera::default();

        // now do stuffs

        self.window = Some(window);
        self.graphic_context = render_context.into();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
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
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => {
                self.handle_keyboard_input(event);
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
