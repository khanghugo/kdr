use std::{io::BufReader, num::NonZeroU32, sync::Arc, time::Instant};

use bsp_load::{BspBuffer, BspTextureBatchBuffer, BspVertex, BspWorldSpawnBuffer};
use camera::{CAM_SPEED, CAM_TURN, Camera};
use lightmap_load::LightMapAtlasBuffer;
use miptex_load::BspMipTex;
use types::{BspFaceBuffer, MeshBuffer};
use wgpu::{Extent3d, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::Window,
};

use cgmath::Deg;

use bitflags::bitflags;

mod bsp_load;
mod camera;
mod lightmap_load;
mod miptex_load;
mod types;
mod utils;

const FILE: &str = "./examples/textures.obj";
const BSP_FILE: &str = "./examples/chk_section.bsp";

const MAX_TEXTURES: u32 = 128;

struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    bsp_render_pipeline: wgpu::RenderPipeline,
    swapchain_format: wgpu::TextureFormat,
    surface: wgpu::Surface<'static>,
    cam_buffer: wgpu::Buffer,
    cam_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        self.device.destroy();
        self.cam_buffer.destroy();
        self.depth_texture.destroy();
    }
}

struct RenderState {
    bsp_buffer: BspBuffer,
    bsp_miptexes: Vec<BspMipTex>,

    camera: Camera,

    // debug
    draw_call: usize,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            camera: Default::default(),
            bsp_buffer: Default::default(),
            bsp_miptexes: Default::default(),
            draw_call: 0,
        }
    }
}

impl RenderContext {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();

        // edit limits
        let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        // limits.max_sampled_textures_per_shader_stage = MAX_TEXTURES;
        // end limits

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::TEXTURE_BINDING_ARRAY,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .unwrap();

        let shader = device.create_shader_module(wgpu::include_wgsl!("./shader/main.wgsl"));

        // camera stuffs
        let cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera buffer"),
            size: 4 * 4 * 4, // 4x4 matrix
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false, // we will update it
        });

        let cam_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // should go into the camera function
        let cam_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &cam_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: cam_buffer.as_entire_binding(),
            }],
        });

        // texture stuffs
        let texture_bind_group_layout =
            device.create_bind_group_layout(&BspMipTex::bind_group_layout_descriptor());

        // lightmap stuffs
        let lightmap_bind_group_layout =
            device.create_bind_group_layout(&LightMapAtlasBuffer::bind_group_layout_descriptor());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &cam_bind_group_layout,
                &texture_bind_group_layout,
                &lightmap_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // depth stuffs
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // rendering stuffs
        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        // enable alpha blending
        let alpha_blending = wgpu::ColorTargetState {
            format: swapchain_format,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
            write_mask: wgpu::ColorWrites::ALL,
        };

        let bsp_vertex_buffer_layout = BspVertex::buffer_layout();

        let bsp_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[bsp_vertex_buffer_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(alpha_blending)],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            present_mode: wgpu::PresentMode::Immediate, // to mailbox later
            ..config
        };

        surface.configure(&device, &config);

        Self {
            device,
            queue,
            bsp_render_pipeline,
            swapchain_format,
            surface,
            cam_bind_group,
            cam_buffer,
            depth_texture,
            depth_view,
            texture_bind_group_layout,
        }
    }

    pub fn render(&self, state: &mut RenderState) {
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        // camera projection
        {
            let view_proj = state.camera.build_view_projection_matrix();
            let view_proj_cast: &[f32; 16] = view_proj.as_ref();
            let view_proj_bytes: &[u8] = bytemuck::cast_slice(view_proj_cast);
            self.queue
                .write_buffer(&self.cam_buffer, 0, view_proj_bytes);
        }

        // render bsp
        {
            let pass_descriptor = wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            };

            let mut rpass = encoder.begin_render_pass(&pass_descriptor);

            rpass.set_pipeline(&self.bsp_render_pipeline);
            rpass.set_bind_group(0, &self.cam_bind_group, &[]);
            rpass.set_bind_group(
                2,
                &state.bsp_buffer.lightmap.as_ref().unwrap().bind_group,
                &[],
            );

            // TODO: room for improvement

            // drawing worldspawn
            let world_spawn = &state.bsp_buffer.worldspawn;

            world_spawn.0.iter().for_each(|batch| {
                // rpass.set_bind_group(1, &state.bsp_textures[batch.texture_index].bind_group, &[]);
                rpass.set_bind_group(1, &state.bsp_miptexes[batch.texture_index].bind_group, &[]);
                rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
            });

            // drawing entities
            state.bsp_buffer.entities.as_ref().map(|entities| {
                entities.0.iter().for_each(|batch| {
                    rpass.set_bind_group(
                        1,
                        &state.bsp_miptexes[batch.texture_index].bind_group,
                        &[],
                    );
                    rpass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    rpass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..batch.index_count as u32, 0, 0..1);
                });
            });

            state.draw_call = 0;
            state.draw_call += world_spawn.0.len();
            state
                .bsp_buffer
                .entities
                .as_ref()
                .map(|entities| state.draw_call += entities.0.len());
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn load_obj(&self, model: tobj::Model) -> MeshBuffer {
        let vertex_array = mesh_to_interleaved_data(&model.mesh);

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("loading .obj"),
                contents: bytemuck::cast_slice(&vertex_array),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

        let vertex_index_array: Vec<u32> = (0..model.mesh.indices.len() as u32).collect();

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("loading .obj indices"),
                contents: bytemuck::cast_slice(&vertex_index_array),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

        MeshBuffer {
            vertex_buffer,
            index_buffer,
            index_length: vertex_index_array.len(),
            material: model.mesh.material_id,
        }
    }
}

fn mesh_to_interleaved_data(mesh: &tobj::Mesh) -> Vec<f32> {
    assert!(!mesh.positions.is_empty(), "Missing position data");
    assert!(mesh.positions.len() % 3 == 0, "Invalid position data");
    assert!(!mesh.normals.is_empty(), "Missing normals");
    assert!(mesh.normals.len() % 3 == 0, "Invalid normal data");
    assert!(!mesh.texcoords.is_empty(), "Missing texture coordinates");
    assert!(mesh.texcoords.len() % 2 == 0, "Invalid texcoord data");

    mesh.indices
        .iter()
        .flat_map(|&idx| {
            let pos = &mesh.positions[(3 * idx as usize)..(3 * idx as usize + 3)];
            // let pos = [mesh.positions[3 * idx as usize], mesh.positions[3 * idx as usize + 1], mesh.positions[3 * idx as usize + 2]];
            // let pos = pos.as_slice();
            let normal = &mesh.normals[(3 * idx as usize)..(3 * idx as usize + 3)];
            let texcoord = &mesh.texcoords[(2 * idx as usize)..(2 * idx as usize + 2)];

            [pos, normal, texcoord].into_iter().flatten()
        })
        .cloned()
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct Key(u32);

struct App {
    graphic_context: Option<RenderContext>,
    window: Option<Arc<Window>>,
    objs: Vec<MeshBuffer>,

    // time
    last_time: Instant,
    frame_time: f32,

    // stuffs
    render_state: RenderState,

    // input
    keys: Key,
}

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
            objs: Default::default(),
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

        // loading obj
        // {
        //     let (models, materials) = tobj::load_obj(FILE, &tobj::LoadOptions {
        //         triangulate: true,
        //         single_index: true,
        //         ..Default::default()
        //     })
        //     .unwrap();

        //     self.render_state.models = models
        //         .into_iter()
        //         .map(|model| render_context.load_obj(model))
        //         .collect();
        //     let materials = materials.unwrap_or(vec![]);

        //     let textures = materials
        //         .into_iter()
        //         .filter_map(|material| material.diffuse_texture)
        //         .map(|path| {
        //             image::open(path)
        //                 .unwrap()
        //                 .flipv() // flip vertically
        //                 .to_rgba8()
        //         })
        //         .map(|img| render_context.load_texture(&img))
        //         .collect::<Vec<TextureBuffer>>();
        //     self.render_state.textures = textures;
        // }

        // load bsp
        {
            let bsp = bsp::Bsp::from_file(BSP_FILE).unwrap();
            self.render_state.bsp_buffer = render_context.load_bsp(&bsp);
            // self.render_state.bsp_textures = bsp
            //     .textures
            //     .iter()
            //     .map(|miptex| render_context.load_miptex_to_rgba8(miptex))
            //     .collect();
            self.render_state.bsp_miptexes = bsp
                .textures
                .iter()
                .map(|miptex| render_context.load_miptex(miptex))
                .collect();
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
