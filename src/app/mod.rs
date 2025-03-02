use std::{sync::Arc, time::Instant};

use cgmath::Deg;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::Window,
};

use crate::renderer::{
    RenderContext, RenderState, bsp_buffer::BspLoader, camera::Camera, mdl_buffer::MdlLoader,
};

pub const CAM_SPEED: f32 = 1000.;
pub const CAM_TURN: f32 = 150.; // degrees

const FILE: &str = "./examples/textures.obj";
const BSP_FILE: &str = "./examples/hb_MART.bsp";
const MDL_FILE: &str = "/home/khang/kdr/examples/ambeech1.mdl";
// const MDL_FILE: &str = "/home/khang/kdr/examples/sekai3.mdl";
// const BSP_FILE: &str = "/home/khang/map/arte_aerorun/slide_surfer.bsp";

#[derive(Debug, Clone, Copy)]
struct Key(u32);

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

use bitflags::bitflags;

bitflags! {
    impl Key: u32 {
        const Forward   = (1 << 0);
        const Back      = (1 << 1);
        const MoveLeft  = (1 << 2);
        const MoveRight = (1 << 3);
        const Left      = (1 << 4);
        const Right     = (1 << 5);
        const Up        = (1 << 6);
        const Down      = (1 << 7);
        const Shift     = (1 << 8);
        const Control   = (1 << 9);
    }
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
            let bsp_buffer =
                BspLoader::load_bsp(&render_context.device, &render_context.queue, &bsp);
            self.render_state.bsp_buffers = vec![bsp_buffer];

            let mdl = mdl::Mdl::open_from_file(MDL_FILE).unwrap();
            let mdl_buffer =
                MdlLoader::load_mdls(&render_context.device, &render_context.queue, &[mdl]);

            self.render_state.mdl_buffers = vec![mdl_buffer];
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
            } => match event.physical_key {
                winit::keyboard::PhysicalKey::Code(key_code) => match key_code {
                    KeyCode::KeyW => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Forward);
                        } else {
                            self.keys = self.keys.intersection(Key::Forward.complement());
                        }
                    }
                    KeyCode::KeyS => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Back);
                        } else {
                            self.keys = self.keys.intersection(Key::Back.complement());
                        }
                    }
                    KeyCode::KeyA => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::MoveLeft);
                        } else {
                            self.keys = self.keys.intersection(Key::MoveLeft.complement());
                        }
                    }
                    KeyCode::KeyD => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::MoveRight);
                        } else {
                            self.keys = self.keys.intersection(Key::MoveRight.complement());
                        }
                    }
                    KeyCode::ArrowLeft => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Left);
                        } else {
                            self.keys = self.keys.intersection(Key::Left.complement());
                        }
                    }
                    KeyCode::ArrowRight => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Right);
                        } else {
                            self.keys = self.keys.intersection(Key::Right.complement());
                        }
                    }
                    KeyCode::ArrowUp => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Up);
                        } else {
                            self.keys = self.keys.intersection(Key::Up.complement());
                        }
                    }
                    KeyCode::ArrowDown => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Down);
                        } else {
                            self.keys = self.keys.intersection(Key::Down.complement());
                        }
                    }
                    KeyCode::ShiftLeft => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Shift);
                        } else {
                            self.keys = self.keys.intersection(Key::Shift.complement());
                        }
                    }
                    KeyCode::ControlLeft => {
                        if event.state.is_pressed() {
                            self.keys = self.keys.union(Key::Control);
                        } else {
                            self.keys = self.keys.intersection(Key::Control.complement());
                        }
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }
    }
}

impl App {
    fn forward(&mut self) {
        self.render_state
            .camera
            .move_along_view(self.get_move_displacement());
    }

    fn back(&mut self) {
        self.render_state
            .camera
            .move_along_view(-self.get_move_displacement());
    }

    fn moveleft(&mut self) {
        self.render_state
            .camera
            .move_along_view_orthogonal(-self.get_move_displacement());
    }

    fn moveright(&mut self) {
        self.render_state
            .camera
            .move_along_view_orthogonal(self.get_move_displacement());
    }

    fn up(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_pitch(self.get_camera_displacement());
    }

    fn down(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_pitch(-self.get_camera_displacement());
    }

    fn left(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_yaw(self.get_camera_displacement());
    }

    fn right(&mut self) {
        self.render_state
            .camera
            .rotate_in_place_yaw(-self.get_camera_displacement());
    }

    fn get_move_displacement(&self) -> f32 {
        CAM_SPEED * self.frame_time * self.get_multiplier()
    }

    fn get_camera_displacement(&self) -> Deg<f32> {
        Deg(CAM_TURN * self.frame_time) * self.get_multiplier()
    }

    fn get_multiplier(&self) -> f32 {
        if self.keys.contains(Key::Shift) {
            2.0
        } else if self.keys.contains(Key::Control) {
            0.5
        } else {
            1.0
        }
    }

    /// Only ticks on redraw
    fn tick(&mut self) {
        let now = Instant::now();
        self.frame_time = now.duration_since(self.last_time).as_secs_f32();
        self.last_time = now;

        if self.keys.contains(Key::Forward) {
            self.forward();
        }
        if self.keys.contains(Key::Back) {
            self.back();
        }
        if self.keys.contains(Key::MoveLeft) {
            self.moveleft();
        }
        if self.keys.contains(Key::MoveRight) {
            self.moveright();
        }
        if self.keys.contains(Key::Left) {
            self.left();
        }
        if self.keys.contains(Key::Right) {
            self.right();
        }
        if self.keys.contains(Key::Up) {
            self.up();
        }
        if self.keys.contains(Key::Down) {
            self.down();
        }
    }
}

pub fn bsp() {
    // let vertices = models.iter().map(|model| model.mesh)
    // let reader = BufReader::new(&obj_bytes[..]);
    // let (models, materials) = tobj::load_obj_buf(&mut reader, &tobj::LoadOptions::default(), |p| {

    // });
    // wgpu uses `log` for all of our logging, so we initialize a logger with the `env_logger` crate.
    //
    // To change the log level, set the `RUST_LOG` environment variable. See the `env_logger`
    // documentation for more information.
    env_logger::init();

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
