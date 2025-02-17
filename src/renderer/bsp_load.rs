use std::collections::HashMap;

use cgmath::{Point2, Point3};
use wgpu::{naga::back, util::DeviceExt};

use super::{
    BspFaceBuffer, RenderContext,
    lightmap_load::LightMapAtlasBuffer,
    utils::{process_face, triangulate_convex_polygon},
};

#[repr(C)]
pub struct BspVertex {
    pos: Point3<f32>,
    norm: Point3<f32>,
    tex_coord: Point2<f32>,
    lightmap_coord: Point2<f32>,
}

// one buffer contains all vertices with the same texture
pub struct BspTextureBatchBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: usize,
    pub texture_index: usize,
}

// world spawn can just render
#[derive(Default)]
pub struct BspWorldSpawnBuffer(pub Vec<BspTextureBatchBuffer>);

// entities needs to sort
#[derive(Default)]
pub struct BspEntitiesBuffer(pub Vec<BspTextureBatchBuffer>);

#[derive(Default)]
pub struct BspBuffer {
    pub worldspawn: BspWorldSpawnBuffer,
    pub entities: BspEntitiesBuffer,
    pub lightmap: Option<LightMapAtlasBuffer>,
}

impl BspVertex {
    fn f32_count() -> usize {
        std::mem::size_of::<Self>() / 4
    }

    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
                // tex
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 2,
                },
                // lightmap
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 32,
                    shader_location: 3,
                },
            ],
        }
    }
}

impl RenderContext {
    fn load_faces<'a, T>(
        &self,
        bsp: &bsp::Bsp,
        faces: T,
        lightmap: &LightMapAtlasBuffer,
    ) -> Vec<BspTextureBatchBuffer>
    where
        T: Iterator<Item = (usize, &'a bsp::Face)>,
    {
        let mut batches = HashMap::<usize, (Vec<f32>, Vec<u32>)>::new();

        for (face_idx, face) in faces {
            let (vertices, indices) = process_face(face, bsp, lightmap, face_idx);
            let texinfo = &bsp.texinfo[face.texinfo as usize];

            let batch = batches
                .entry(texinfo.texture_index as usize)
                .or_insert((Vec::new(), Vec::new()));

            // newer vertices will have their index start at 0 but we don't want that
            // need to divide by <x> because each "vertices" has <x> floats
            let new_vertices_offset = batch.0.len() / BspVertex::f32_count();

            batch.0.extend(vertices);
            batch
                .1
                .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
        }

        let batches = batches
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
            .collect();

        batches
    }

    fn load_worldspawn(
        &self,
        bsp: &bsp::Bsp,
        lightmap: &LightMapAtlasBuffer,
    ) -> BspWorldSpawnBuffer {
        let worldspawn = &bsp.models[0];
        let faces = &bsp.faces[worldspawn.first_face as usize
            ..(worldspawn.first_face as usize + worldspawn.face_count as usize)];

        let batches = self.load_faces(bsp, faces.iter().enumerate(), lightmap);

        BspWorldSpawnBuffer(batches)
    }

    fn load_entities(&self, bsp: &bsp::Bsp, lightmap: &LightMapAtlasBuffer) -> BspEntitiesBuffer {
        // TODO sort all of the vertices later
        let rest = &bsp.models[1..];

        let entity_faces: Vec<bsp::Face> = rest
            .iter()
            .flat_map(|model| {
                let current_faces = &bsp.faces[model.first_face as usize
                    ..(model.first_face as usize + model.face_count as usize)];

                current_faces
            })
            .cloned() // i cri everytime
            .collect();

        let first_entity_face = rest[0].first_face;

        let batches = self.load_faces(
            bsp,
            entity_faces
                .iter()
                .enumerate()
                .map(|(idx, e)| (idx + first_entity_face as usize, e)),
            &lightmap,
        );

        BspEntitiesBuffer(batches)
    }

    pub fn load_bsp(&self, bsp: &bsp::Bsp) -> BspBuffer {
        let lightmap = self.load_lightmap(bsp);

        BspBuffer {
            worldspawn: self.load_worldspawn(bsp, &lightmap),
            entities: self.load_entities(bsp, &lightmap),
            lightmap: lightmap.into(),
        }
    }

    // fn load_bsp_based_on_texture_batch(&self, bsp: &bsp::Bsp) -> Vec<BspTextureBatchBuffer> {
    //     let mut batches = HashMap::<usize, (Vec<f32>, Vec<u32>)>::new();

    //     for face in &bsp.faces {
    //         let (vertices, indices) = process_face(face, bsp);

    //         let texinfo = &bsp.texinfo[face.texinfo as usize];

    //         let batch = batches
    //             .entry(texinfo.texture_index as usize)
    //             .or_insert((Vec::new(), Vec::new()));

    //         // newer vertices will have their index start at 0 but we don't want that
    //         // need to divide by 8 because each "vertices" has 8 floats
    //         let new_vertices_offset = batch.0.len() / 8;

    //         batch.0.extend(vertices);
    //         batch
    //             .1
    //             .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
    //     }

    //     batches
    //         .into_iter()
    //         .map(|(texture_index, (vertices, indices))| {
    //             let vertex_buffer =
    //                 self.device
    //                     .create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //                         label: Some("loading a bsp vertex"),
    //                         contents: bytemuck::cast_slice(&vertices),
    //                         usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    //                     });

    //             let index_buffer =
    //                 self.device
    //                     .create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //                         label: Some("loading a bsp vertex index"),
    //                         contents: bytemuck::cast_slice(&indices),
    //                         usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    //                     });

    //             BspTextureBatchBuffer {
    //                 vertex_buffer,
    //                 index_buffer,
    //                 index_count: indices.len(),
    //                 texture_index,
    //             }
    //         })
    //         .collect()
    // }

    // batch loading based on polygon
    // fn load_bsp_based_on_face(&self, bsp: &bsp::Bsp) -> Vec<BspFaceBuffer> {
    //     let res = bsp
    //         .faces
    //         .iter()
    //         .map(|face| {
    //             let (vertices, indices) = process_face(face, bsp);
    //             let texinfo = &bsp.texinfo[face.texinfo as usize];

    //             let vertex_buffer =
    //                 self.device
    //                     .create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //                         label: Some("loading a bsp vertex"),
    //                         contents: bytemuck::cast_slice(&vertices),
    //                         usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    //                     });

    //             let vertex_index_buffer =
    //                 self.device
    //                     .create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //                         label: Some("loading a bsp vertex index"),
    //                         contents: bytemuck::cast_slice(&indices),
    //                         usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
    //                     });

    //             BspFaceBuffer {
    //                 vertex_buffer,
    //                 texture_index: texinfo.texture_index as usize,
    //                 index_count: indices.len(),
    //                 index_buffer: vertex_index_buffer,
    //             }
    //         })
    //         .collect();

    //     res
    // }
}

struct LightmapDimensions {
    width: i32,
    height: i32,
    min_u: i32,
    min_v: i32,
}

// https://github.com/rein4ce/hlbsp/blob/1546eaff4e350a2329bc2b67378f042b09f0a0b7/js/hlbsp.js#L499
fn get_lightmap_dimensions(uvs: &[[f32; 2]]) -> LightmapDimensions {
    let mut min_u = uvs[0][0].floor() as i32;
    let mut min_v = uvs[0][1].floor() as i32;
    let mut max_u = uvs[0][0].floor() as i32;
    let mut max_v = uvs[0][1].floor() as i32;

    for i in 1..uvs.len() {
        let u = uvs[i][0].floor() as i32;
        let v = uvs[i][1].floor() as i32;

        if u < min_u {
            min_u = u;
        }
        if v < min_v {
            min_v = v;
        }
        if u > max_u {
            max_u = u;
        }
        if v > max_v {
            max_v = v;
        }
    }

    return LightmapDimensions {
        width: ((max_u as f32 / 16.0).ceil() as i32) - ((min_u as f32 / 16.0).floor() as i32) + 1,
        height: ((max_v as f32 / 16.0).ceil() as i32) - ((min_v as f32 / 16.0).floor() as i32) + 1,
        min_u,
        min_v,
    };
}
