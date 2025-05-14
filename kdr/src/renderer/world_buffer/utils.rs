use std::collections::HashMap;

use image::RgbaImage;
use mdl::Trivert;
use wgpu::util::DeviceExt;

use crate::renderer::utils::eightbpp_to_rgba8;

use super::{WorldVertex, WorldVertexBuffer};

pub fn triangulate_mdl_triverts(
    index_buffer: &mut Vec<u32>,
    triverts: &Vec<Trivert>,
    is_strip: bool,
    offset: usize,
) {
    if is_strip {
        for i in 0..triverts.len().saturating_sub(2) {
            let v1 = offset + i;
            let v2 = offset + i + 1;
            let v3 = offset + i + 2;

            if i % 2 == 0 {
                // Even-indexed triangles
                index_buffer.push(v1 as u32);
                index_buffer.push(v2 as u32);
                index_buffer.push(v3 as u32);
            } else {
                // Odd-indexed triangles (flip winding order)
                index_buffer.push(v2 as u32);
                index_buffer.push(v1 as u32);
                index_buffer.push(v3 as u32);
            }
        }
    } else {
        let first_index = offset as u32;
        for i in 1..triverts.len().saturating_sub(1) {
            index_buffer.push(first_index);
            index_buffer.push((offset + i) as u32);
            index_buffer.push((offset + i + 1) as u32);
        }
    }
}

pub fn get_mdl_textures(mdl: &mdl::Mdl) -> Vec<RgbaImage> {
    mdl.textures
        .iter()
        .map(|texture| {
            eightbpp_to_rgba8(
                &texture.image,
                &texture.palette,
                texture.dimensions().0,
                texture.dimensions().1,
                None,
            )
        })
        .collect()
}

/// Key: Batch Index aka Texture Array Index
///
/// Value: (World Vertex Array, Index Array)
pub type BatchLookup = HashMap<usize, (Vec<WorldVertex>, Vec<u32>)>;

pub fn create_world_vertex_buffer(
    device: &wgpu::Device,
    batch_lookup: BatchLookup,
) -> Vec<WorldVertexBuffer> {
    batch_lookup
        .into_iter()
        .map(|(texture_array_index, (vertices, indices))| {
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("world index buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

            WorldVertexBuffer {
                vertex_buffer,
                index_buffer,
                index_count: indices.len(),
                texture_array_index,
            }
        })
        .collect()
}
