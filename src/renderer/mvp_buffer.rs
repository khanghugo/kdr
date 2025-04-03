use wgpu::util::DeviceExt;

use crate::{app::constants::MAX_MVP, loader::bsp_resource::WorldEntity};

// this should work for bsp as well because we will have func_rotating_door and whatever
pub struct MvpBuffer {
    pub bind_group: wgpu::BindGroup,
    pub buffer: wgpu::Buffer,
}

impl Drop for MvpBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

const EMPTY_MATRIX: [[f32; 4]; 4] = [[0f32; 4]; 4];

impl MvpBuffer {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("model view projection bind group layout"),
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
        }
    }

    pub fn create_mvp(device: &wgpu::Device, entity_infos: &[&WorldEntity]) -> Self {
        let mut matrices: Vec<[[f32; 4]; 4]> = entity_infos
            .iter()
            .map(|info| info.build_mvp().into())
            .collect();

        // uniform buffer has fixed and defined size
        matrices.resize(MAX_MVP as usize, EMPTY_MATRIX);

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection array buffer"),
            contents: bytemuck::cast_slice(&matrices),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let mvp_bind_group_layout =
            device.create_bind_group_layout(&MvpBuffer::bind_group_layout_descriptor());

        let mvp_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view projection array bind group"),
            layout: &mvp_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mvp_buffer.as_entire_binding(),
            }],
        });

        MvpBuffer {
            bind_group: mvp_bind_group,
            buffer: mvp_buffer,
        }
    }

    pub fn update_entity_mvp_buffer(
        &self,
        queue: &wgpu::Queue,
        world_entity_index: usize,
        entity_info: &WorldEntity,
    ) {
        let offset = world_entity_index * 4 * 4 * 4;

        let mvp = entity_info.build_mvp();
        let mvp_cast: &[f32; 16] = mvp.as_ref();
        let mvp_bytes: &[u8] = bytemuck::cast_slice(mvp_cast);

        queue.write_buffer(&self.buffer, offset as u64, mvp_bytes);
    }
}
