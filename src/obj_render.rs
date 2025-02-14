use std::{io::BufReader, num::NonZeroU32, sync::Arc, time::Instant};

use glam::Vec3;
use image::{RgbaImage, imageops::grayscale};
use wgpu::{Extent3d, util::DeviceExt};
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ControlFlow, EventLoop},
    keyboard::KeyCode,
    window::Window,
};

use cgmath::{Deg, EuclideanSpace, Matrix4, Point3, Vector3, perspective};

const FILE: &str = "./examples/textures.obj";

struct Camera {
    pos: Point3<f32>,
    target: Point3<f32>,
    up: Vector3<f32>,
    aspect: f32,
    fovy: Deg<f32>,
    znear: f32,
    zfar: f32,

    // rotation
    radius: f32,
    speed: f32,
    angle: Deg<f32>,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        self.proj() * self.view()
    }

    fn view(&self) -> Matrix4<f32> {
        Matrix4::look_at_rh(self.pos, self.target, self.up)
    }

    fn proj(&self) -> Matrix4<f32> {
        perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }

    fn move_cam(&mut self, new: Point3<f32>) {
        self.pos = new;
    }

    fn update(&mut self, dt: f32) {
        self.angle += Deg(self.speed * dt);

        let new_pos = Point3::new(
            self.radius * self.angle.0.to_radians().cos(), // remember to convert to radian
            0.0,
            self.radius * self.angle.0.to_radians().sin(), // remember to convert to radian
        );

        self.move_cam(new_pos);
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: Point3::<f32>::new(2.0, 2.0, 2.0),
            target: Point3::<f32>::origin(),
            up: Vector3::unit_y(),
            aspect: 640 as f32 / 480 as f32,
            fovy: Deg(90.0),
            znear: 1.0,
            zfar: 1000.0,
            radius: 20.,
            speed: 30.,
            angle: Deg(0.0),
        }
    }
}

struct ObjRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    render_pipeline: wgpu::RenderPipeline,
    swapchain_format: wgpu::TextureFormat,
    surface: wgpu::Surface<'static>,
    cam_buffer: wgpu::Buffer,
    cam_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

struct RenderState {
    textures: Vec<TextureBuffer>,
    camera: Camera,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            textures: Default::default(),
            camera: Default::default(),
        }
    }
}

struct ObjBuffer {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_length: usize,

    // vector of indices, pointing to render object textures
    // for some convenient reasons, .obj will have 1 texture per mesh!!!
    material: Option<usize>,
}

impl Drop for ObjBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

struct TextureBuffer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl Drop for TextureBuffer {
    fn drop(&mut self) {
        self.texture.destroy();
    }
}

const MAX_TEXTURES: u32 = 128; // complying to max_sampled_textures_per_shader_stage

impl ObjRenderer {
    async fn new(window: Arc<Window>) -> Self {
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
        let mut limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        limits.max_sampled_textures_per_shader_stage = MAX_TEXTURES;
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("./obj.wgsl"));

        // camera stuffs
        let cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera buffer"),
            size: 64 * 2, // 2x 4x4 matrix
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
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("texture bind group layout"),
                entries: &[
                    // sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // textures
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&cam_bind_group_layout, &texture_bind_group_layout],
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

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 4 * 8,                     // 3 pos + 3 normal + 2 tex = 8
            step_mode: wgpu::VertexStepMode::Vertex, // huh
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 3 * 4,
                    shader_location: 1,
                },
                // // tex
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 3 * 4 * 2,
                    shader_location: 2,
                },
            ],
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[vertex_buffer_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
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
            render_pipeline,
            swapchain_format,
            surface,
            cam_bind_group,
            cam_buffer,
            depth_texture,
            depth_view,
            texture_bind_group_layout,
        }
    }

    fn render(&self, objs: &[ObjBuffer], state: &RenderState) {
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

            // let mut rpass =
            //     encoder.scoped_render_pass("in render pass", &self.device, pass_descriptor);
            let mut rpass = encoder.begin_render_pass(&pass_descriptor);

            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.cam_bind_group, &[]);

            // TODO: room for improvement
            objs.iter().for_each(|obj| {
                rpass.set_bind_group(1, &state.textures[obj.material.unwrap()].bind_group, &[]);

                rpass.set_vertex_buffer(0, obj.vertex_buffer.slice(..));
                rpass.set_index_buffer(obj.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..obj.index_length as u32, 0, 0..1);
            });
        }

        // state.profiler.resolve_queries(&mut encoder);

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    fn load_obj(&self, model: tobj::Model) -> ObjBuffer {
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

        ObjBuffer {
            vertex_buffer,
            index_buffer,
            index_length: vertex_index_array.len(),
            material: model.mesh.material_id,
        }
    }

    fn load_texture(&self, img: &RgbaImage) -> TextureBuffer {
        let (width, height) = img.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture same name"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width), // rgba
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture same name sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                // sampler
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                // textures
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
            ],
        });

        TextureBuffer {
            texture,
            view: texture_view,
            bind_group,
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

struct App {
    graphic_context: Option<ObjRenderer>,
    window: Option<Arc<Window>>,
    objs: Vec<ObjBuffer>,
    last_time: Instant,

    render_state: RenderState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            graphic_context: Default::default(),
            window: Default::default(),
            objs: Default::default(),
            last_time: Instant::now(),
            render_state: Default::default(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        let window = Arc::new(window);

        let render_context = pollster::block_on(ObjRenderer::new(window.clone()));

        // loading obj
        let (models, materials) = tobj::load_obj(FILE, &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        })
        .unwrap();

        println!("{:#?}", models);

        let materials = materials.unwrap_or(vec![]);

        self.render_state.camera = Camera::default();

        // now do stuffs
        self.objs = models
            .into_iter()
            .map(|model| render_context.load_obj(model))
            .collect();

        println!("objs count is {}", self.objs.len());

        let textures = materials
            .into_iter()
            .filter_map(|material| material.diffuse_texture)
            .map(|path| {
                image::open(path)
                    .unwrap()
                    .flipv() // flip vertically
                    .to_rgba8()
            })
            .map(|img| render_context.load_texture(&img))
            .collect::<Vec<TextureBuffer>>();

        self.render_state.textures = textures;
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
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let diff = now.duration_since(self.last_time);
                self.last_time = now;

                self.render_state.camera.update(diff.as_secs_f32());

                self.graphic_context
                    .as_mut()
                    .map(|res| res.render(&self.objs, &self.render_state));

                self.window.as_mut().map(|window| {
                    // rename window based on fps
                    window
                        .set_title(format!("FPS: {}", (1.0 / diff.as_secs_f32()).round()).as_str());
                    // update
                    window.request_redraw();
                });
            }
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => match event.physical_key {
                winit::keyboard::PhysicalKey::Code(key_code) => {
                    if matches!(key_code, KeyCode::KeyW) {
                        self.render_state.camera.radius -= 1.;
                        self.render_state.camera.radius = self.render_state.camera.radius.max(0.);
                    }
                    if matches!(key_code, KeyCode::KeyS) {
                        self.render_state.camera.radius += 1.;
                    }

                    if matches!(key_code, KeyCode::KeyA) {
                        self.render_state.camera.target += [0., 0., 1.].into();
                    }

                    if matches!(key_code, KeyCode::KeyD) {
                        self.render_state.camera.target -= [0., 0., 1.].into();
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }
}

struct Vertex {
    pos: Vec3,
    norm: Vec3,
}

pub fn obj() {
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
