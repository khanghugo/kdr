mod dynamic_buffer;
mod static_buffer;
mod types;
pub mod utils;

pub use static_buffer::WorldStaticBuffer;
pub use types::*;

use crate::renderer::texture_buffer::texture_array::TextureArrayBuffer;

use super::{bsp_lightmap::LightMapAtlasBuffer, camera::CameraBuffer, mvp_buffer::MvpBuffer};

pub struct WorldLoader;

impl WorldLoader {
    pub fn create_opaque_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::Opaque,
        )
    }

    pub fn create_transparent_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::Transparent,
        )
    }

    pub fn create_z_prepass_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::ZPrepass,
        )
    }

    pub fn create_skybox_mask_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(
            device,
            fragment_targets,
            depth_format,
            WorldPipelineType::SkyboxMask,
        )
    }

    fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        depth_format: wgpu::TextureFormat,
        pipeline_type: WorldPipelineType,
    ) -> wgpu::RenderPipeline {
        let world_shader = device.create_shader_module(wgpu::include_wgsl!("../shader/world.wgsl"));

        // common data
        let texture_array_bind_group_layout =
            device.create_bind_group_layout(&TextureArrayBuffer::bind_group_layout_descriptor());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&CameraBuffer::bind_group_layout_descriptor());

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MvpBuffer::bind_group_layout_descriptor());

        // bsp specific
        let lightmap_bind_group_layout =
            device.create_bind_group_layout(&LightMapAtlasBuffer::bind_group_layout_descriptor());

        let push_constant_ranges = match pipeline_type {
            WorldPipelineType::ZPrepass | WorldPipelineType::SkyboxMask => vec![],
            WorldPipelineType::Opaque | WorldPipelineType::Transparent => {
                vec![wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::FRAGMENT,
                    range: 0..std::mem::size_of::<WorldPushConstants>() as u32,
                }]
            }
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &camera_bind_group_layout,        // 0
                &mvp_bind_group_layout,           // 1
                &texture_array_bind_group_layout, // 2
                &lightmap_bind_group_layout,      // 3
            ],
            push_constant_ranges: &push_constant_ranges,
        });

        let fragment_targets = fragment_targets
            .into_iter()
            .map(|v| Some(v))
            .collect::<Vec<Option<wgpu::ColorTargetState>>>();

        // dont write any more depth after z prepass
        let depth_write_enabled = match pipeline_type {
            WorldPipelineType::ZPrepass => true,
            // if i somehow start doign z prepass again, opaque should not write to depth
            WorldPipelineType::Opaque => true,
            WorldPipelineType::Transparent | WorldPipelineType::SkyboxMask => false,
        };

        let pipeline_label = match pipeline_type {
            WorldPipelineType::ZPrepass => "world z prepass render pipeline",
            WorldPipelineType::Opaque => "world opaque render pipeline",
            WorldPipelineType::Transparent => "world transparent render pipeline",
            WorldPipelineType::SkyboxMask => "world skybox mask render pipeline",
        };

        let depth_compare = match pipeline_type {
            WorldPipelineType::ZPrepass => wgpu::CompareFunction::Less,
            WorldPipelineType::Opaque => wgpu::CompareFunction::LessEqual,
            WorldPipelineType::Transparent => wgpu::CompareFunction::Less,
            // need to write stencil in a way that the skybrushes are behind some objects
            WorldPipelineType::SkyboxMask => wgpu::CompareFunction::LessEqual,
        };

        let stencil_state: wgpu::StencilState = match pipeline_type {
            WorldPipelineType::SkyboxMask => wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    pass_op: wgpu::StencilOperation::Replace,
                    fail_op: wgpu::StencilOperation::Keep,
                    ..Default::default()
                },
                read_mask: 0xFF,
                write_mask: 0xFF,
                ..Default::default()
            },
            WorldPipelineType::ZPrepass
            | WorldPipelineType::Opaque
            | WorldPipelineType::Transparent => Default::default(),
        };

        let vertex_shader_entry_point = match pipeline_type {
            WorldPipelineType::SkyboxMask => "skybox_mask_vs",
            // does nothing
            WorldPipelineType::ZPrepass => "skybox_mask_vs",
            WorldPipelineType::Opaque | WorldPipelineType::Transparent => "vs_main",
        };

        let world_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(pipeline_label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &world_shader,
                    entry_point: Some(vertex_shader_entry_point),
                    compilation_options: Default::default(),
                    buffers: &[WorldVertex::buffer_layout()],
                },
                fragment: match pipeline_type {
                    WorldPipelineType::ZPrepass | WorldPipelineType::SkyboxMask => None,
                    WorldPipelineType::Opaque | WorldPipelineType::Transparent => {
                        Some(wgpu::FragmentState {
                            module: &world_shader,
                            entry_point: Some(
                                if matches!(pipeline_type, WorldPipelineType::Opaque) {
                                    "fs_opaque"
                                } else {
                                    "fs_transparent"
                                },
                            ),
                            compilation_options: Default::default(),
                            targets: &fragment_targets,
                        })
                    }
                },
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Cw,
                    cull_mode: Some(wgpu::Face::Back),
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled,
                    depth_compare,
                    stencil: stencil_state,
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        world_render_pipeline
    }
}
