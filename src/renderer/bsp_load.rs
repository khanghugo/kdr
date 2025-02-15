use std::collections::HashMap;

use wgpu::util::DeviceExt;

use super::{
    BspFaceBuffer, RenderContext, TextureBuffer,
    types::BspTextureBatchBuffer,
    utils::{eightbpp_to_rgba8, triangulate_convex_polygon},
};

impl RenderContext {
    pub fn load_bsp_based_on_texture_batch(&self, bsp: &bsp::Bsp) -> Vec<BspTextureBatchBuffer> {
        let mut batches = HashMap::<usize, (Vec<f32>, Vec<u32>)>::new();

        for face in &bsp.faces {
            let (vertices, indices) = process_face(face, bsp);

            let texinfo = &bsp.texinfo[face.texinfo as usize];

            let batch = batches
                .entry(texinfo.texture_index as usize)
                .or_insert((Vec::new(), Vec::new()));

            // newer vertices will have their index start at 0 but we don't want that
            // need to divide by 8 because each "vertices" has 8 floats
            let new_vertices_offset = batch.0.len() / 8;

            batch.0.extend(vertices);
            batch
                .1
                .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
        }

        batches
            .into_iter()
            .map(|(texture_index, (vertices, indices))| {
                let vertex_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("loading a bsp vertex"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });

                let index_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("loading a bsp vertex index"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        });

                BspTextureBatchBuffer {
                    vertex_buffer,
                    index_buffer,
                    index_count: indices.len(),
                    texture_index,
                }
            })
            .collect()
    }

    // batch loading based on polygon
    pub fn load_bsp_based_on_face(&self, bsp: &bsp::Bsp) -> Vec<BspFaceBuffer> {
        let res = bsp
            .faces
            .iter()
            .map(|face| {
                let (vertices, indices) = process_face(face, bsp);
                let texinfo = &bsp.texinfo[face.texinfo as usize];

                let vertex_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("loading a bsp vertex"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        });

                let vertex_index_buffer =
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("loading a bsp vertex index"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                        });

                BspFaceBuffer {
                    vertex_buffer,
                    texture_index: texinfo.texture_index as usize,
                    index_count: indices.len(),
                    index_buffer: vertex_index_buffer,
                }
            })
            .collect();

        res
    }

    pub fn load_miptex(&self, miptex: &bsp::Texture) -> TextureBuffer {
        // TODO: maybe this needs checking??
        let mip_image = &miptex.mip_images[0];
        let rgba8 = eightbpp_to_rgba8(
            mip_image.data.get_bytes(),
            miptex.palette.get_bytes(),
            miptex.width,
            miptex.height,
        );

        self.load_texture(&rgba8)
    }
}

fn process_face(face: &bsp::Face, bsp: &bsp::Bsp) -> (Vec<f32>, Vec<u32>) {
    let mut face_vertices = vec![];

    for edge_idx in (face.first_edge as u32)..(face.first_edge as u32 + face.edge_count as u32) {
        let surf_edge = bsp.surf_edges[edge_idx as usize];

        let [v1_idx, v2_idx] = bsp.edges[surf_edge.abs() as usize];

        if surf_edge.is_positive() {
            face_vertices.push(bsp.vertices[v1_idx as usize]);
        } else {
            face_vertices.push(bsp.vertices[v2_idx as usize]);
        }
    }

    let indices = triangulate_convex_polygon(&face_vertices);

    // very inefficient right now
    // becuase all vertices here have the same normal
    let normal = bsp.planes[face.plane as usize].normal;
    let texinfo = &bsp.texinfo[face.texinfo as usize];

    // uv
    let miptex = &bsp.textures[texinfo.texture_index as usize];
    let inv_width = 1.0 / miptex.width as f32;
    let inv_height = 1.0 / miptex.height as f32;

    let vertices_texcoord: Vec<[f32; 2]> = face_vertices
        .iter()
        .map(|pos| {
            [
                (pos.dot(texinfo.u) + texinfo.u_offset) * inv_width,
                (pos.dot(texinfo.v) + texinfo.v_offset) * inv_height,
            ]
        })
        .collect();

    // collect to buffer
    let interleaved: Vec<f32> = face_vertices
        .into_iter()
        .zip(vertices_texcoord.into_iter())
        .flat_map(|(pos, texcoord)| {
            [
                // no need to flip any of the geometry
                // we will do that to the camera
                // -pos.x,
                // pos.z, // flip y and z because the game is z up
                // pos.y,
                pos.x,
                pos.y,
                pos.z,
                normal.x,
                normal.y,
                normal.z,
                texcoord[0],
                texcoord[1],
            ]
        })
        .collect();

    (interleaved, indices)
}
