use cgmath::Zero;
use tracing::warn;
use wgpu::util::DeviceExt;

use crate::app::constants::MAX_MVP;

// this should work for bsp as well because we will have func_rotating_door and whatever
pub struct MvpBuffer {
    pub bind_group: wgpu::BindGroup,
    // mvp buffer for basically everything in the map
    pub buffer: wgpu::Buffer,
    queue: wgpu::Queue,
}

impl Drop for MvpBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
    }
}

impl MvpBuffer {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("model view projection bind group layout"),
            entries: &[
                // mvp buffer
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
            ],
        }
    }

    pub fn create_mvp(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut transformations: Vec<cgmath::Matrix4<f32>>,
    ) -> Self {
        // uniform buffer has fixed and defined size
        if transformations.len() > MAX_MVP {
            warn!("There are more transformations than MAX_MVP");
        }

        transformations.resize(MAX_MVP, cgmath::Matrix4::zero());

        let transformations_casted: Vec<[[f32; 4]; 4]> =
            transformations.into_iter().map(|x| x.into()).collect();

        let mvp_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("model view projection buffer"),
            contents: bytemuck::cast_slice(&transformations_casted),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            device.create_bind_group_layout(&MvpBuffer::bind_group_layout_descriptor());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model view projection array bind group"),
            layout: &bind_group_layout,
            entries: &[
                // mvp buffer
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: mvp_buffer.as_entire_binding(),
                },
            ],
        });

        MvpBuffer {
            bind_group,
            buffer: mvp_buffer,
            queue: queue.clone(),
        }
    }

    /// mvp_index is the index of the mvp, no need to calculate offset from the caller
    pub fn update_mvp_buffer(&self, mvp: cgmath::Matrix4<f32>, mvp_index: usize) {
        let mvp_cast: [[f32; 4]; 4] = mvp.into();
        let offset = mvp_index as u64 * 64;

        self.queue
            .write_buffer(&self.buffer, offset, bytemuck::cast_slice(&mvp_cast));
    }

    pub fn update_mvp_buffer_many(&self, mvps: Vec<cgmath::Matrix4<f32>>, mvp_index_start: usize) {
        let mvps_cast: Vec<[[f32; 4]; 4]> = mvps.into_iter().map(|x| x.into()).collect();
        let offset = mvp_index_start as u64 * 64;

        self.queue
            .write_buffer(&self.buffer, offset, bytemuck::cast_slice(&mvps_cast));
    }

    // pub fn update_entity_mvp_buffer(&self, entity_info: &WorldEntity, time: f32) {
    //     match entity_info.build_mvp(time) {
    //         BuildMvpResult::Entity(matrix4) => {
    //             let offset = entity_info.world_index * 4 * 4 * 4;

    //             let mvp_cast: &[f32; 16] = matrix4.as_ref();
    //             let mvp_bytes: &[u8] = bytemuck::cast_slice(mvp_cast);
    //             self.queue
    //                 .write_buffer(&self.buffer, offset as u64, mvp_bytes);
    //         }
    //         BuildMvpResult::Skeletal(matrix4s) => {
    //             let entity_skeletal_start = self.skeletal_lookup.get(&entity_info.world_index);

    //             matrix4s.iter().enumerate().for_each(|(idx, mat)| {
    //                 let mvp_cast: &[f32; 16] = mat.as_ref();
    //                 let mvp_bytes: &[u8] = bytemuck::cast_slice(mvp_cast);

    //                 if idx == 0 {
    //                     let offset = entity_info.world_index * 4 * 4 * 4;
    //                     self.queue
    //                         .write_buffer(&self.buffer, offset as u64, mvp_bytes);
    //                 } else {
    //                     let Some(entity_skeletal_start) = entity_skeletal_start else {
    //                         return;
    //                     };

    //                     let mvp_idx = entity_skeletal_start + idx - 1;
    //                     let offset = mvp_idx * 4 * 4 * 4;

    //                     self.queue
    //                         .write_buffer(&self.skeletal_buffer, offset as u64, mvp_bytes);
    //                 }
    //             });
    //         }
    //     };
    // }
}
