use wgpu::util::DeviceExt;

use crate::bsp_loader::MdlEntityInfo;

// this should work for bsp as well because we will have func_rotating_door and whatever
pub struct MdlMvp {
    pub bind_group: wgpu::BindGroup,
    pub buffer: wgpu::Buffer,
    pub entity_infos: Vec<MdlEntityInfo>,
}

impl MdlMvp {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("model view projection bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        }
    }

    pub fn create_mvp(device: &wgpu::Device, entity_infos: &[&MdlEntityInfo]) -> Self {
        let matrices: Vec<[[f32; 4]; 4]> = entity_infos
            .iter()
            .map(|info| info.model_view_projection.into())
            .collect();

        // fix empty matrix in case there are zero models
        let matrices = if matrices.is_empty() {
            vec![[[0f32; 4]; 4]]
        } else {
            matrices
        };

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection array buffer"),
            contents: bytemuck::cast_slice(&matrices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MdlMvp::bind_group_layout_descriptor());

        let mvp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view projection array bind group"),
            layout: &mvp_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mvp_buffer.as_entire_binding(),
            }],
        });

        MdlMvp {
            bind_group: mvp_bind_group,
            buffer: mvp_buffer,
            entity_infos: entity_infos
                .iter()
                .map(|s| {
                    // the fuck?
                    let what = s.to_owned().to_owned();
                    what
                })
                .collect(),
        }
    }
}
