use loader::bsp_resource::WorldEntity;
use wgpu::util::DeviceExt;

use crate::app::constants::MAX_MVP;

// this should work for bsp as well because we will have func_rotating_door and whatever
pub struct MvpBuffer {
    pub bind_group: wgpu::BindGroup,
    // mvp buffer for basically everything in the map
    pub entity_buffer: wgpu::Buffer,
    // if a studio model has more than 1 bone, bone #1 is inside entity buffer,
    // but starting from bone #2, they all will be in this buffer
    pub skeletal_buffer: wgpu::Buffer,
}

impl Drop for MvpBuffer {
    fn drop(&mut self) {
        self.entity_buffer.destroy();
        self.skeletal_buffer.destroy();
    }
}

const EMPTY_MATRIX: [[f32; 4]; 4] = [[0f32; 4]; 4];

impl MvpBuffer {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("model view projection bind group layout"),
            entries: &[
                // entity buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // skeletal buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        }
    }

    pub fn create_mvp(device: &wgpu::Device, entity_infos: &[&WorldEntity]) -> Self {
        let mut entity_mvps: Vec<[[f32; 4]; 4]> = vec![];

        // it has emtpy matrix as index 0 because index 0 is unambiguous for our data structure
        let mut skeletal_mvps: Vec<[[f32; 4]; 4]> = vec![EMPTY_MATRIX];

        // for entity, it is pretty straightforward
        // for skeletal, build mvp(s). the first mvp will be inside entity mvps
        // subsequent mvps are inside skeletal mvps
        // make sure that the order of world buffer to match this order as well
        entity_infos
            .iter()
            .for_each(|entity| match entity.build_mvp() {
                loader::bsp_resource::BuildMvpResult::Entity(matrix4) => {
                    entity_mvps.push(matrix4.into());
                }
                loader::bsp_resource::BuildMvpResult::Skeletal(matrix4s) => {
                    entity_mvps.push(matrix4s[0].into());

                    matrix4s[1..].into_iter().for_each(|&x| {
                        skeletal_mvps.push(x.into());
                    });
                }
            });

        // uniform buffer has fixed and defined size
        entity_mvps.resize(MAX_MVP as usize, EMPTY_MATRIX);
        skeletal_mvps.resize(MAX_MVP as usize, EMPTY_MATRIX);

        let entity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection entity buffer"),
            contents: bytemuck::cast_slice(&entity_mvps),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let skeletal_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection skeletal buffer"),
            contents: bytemuck::cast_slice(&skeletal_mvps),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&MvpBuffer::bind_group_layout_descriptor());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view projection array bind group"),
            layout: &bind_group_layout,
            entries: &[
                // entity buffer
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: entity_buffer.as_entire_binding(),
                },
                // skeletal buffer
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skeletal_buffer.as_entire_binding(),
                },
            ],
        });

        MvpBuffer {
            bind_group,
            entity_buffer,
            skeletal_buffer,
        }
    }

    pub fn update_entity_mvp_buffer(
        &self,
        queue: &wgpu::Queue,
        world_entity_index: usize,
        entity_info: &WorldEntity,
    ) {
        match entity_info.build_mvp() {
            loader::bsp_resource::BuildMvpResult::Entity(matrix4) => {
                let offset = world_entity_index * 4 * 4 * 4;

                let mvp_cast: &[f32; 16] = matrix4.as_ref();
                let mvp_bytes: &[u8] = bytemuck::cast_slice(mvp_cast);
                queue.write_buffer(&self.entity_buffer, offset as u64, mvp_bytes);
            }
            loader::bsp_resource::BuildMvpResult::Skeletal(matrix4s) => {
                matrix4s.iter().enumerate().for_each(|(idx, mat)| {
                    let mvp_cast: &[f32; 16] = mat.as_ref();
                    let mvp_bytes: &[u8] = bytemuck::cast_slice(mvp_cast);

                    if idx == 0 {
                        let offset = world_entity_index * 4 * 4 * 4;
                        queue.write_buffer(&self.entity_buffer, offset as u64, mvp_bytes);
                    } else {
                        // need to know the entire entity list to correctly find the offset
                        todo!("doesnt know how to update mvp buffer for skeletal models yet")
                    }
                });
            }
        };
    }
}
