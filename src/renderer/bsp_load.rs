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

impl Drop for BspTextureBatchBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
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
    pub entities: Option<BspEntitiesBuffer>,
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

    fn load_entities(&self, bsp: &bsp::Bsp, lightmap: &LightMapAtlasBuffer) -> Option<BspEntitiesBuffer> {
        // TODO sort all of the vertices later
        let rest = &bsp.models[1..];

        if rest.is_empty() {
            return None;
        }

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

        Some(BspEntitiesBuffer(batches))
    }

    pub fn load_bsp(&self, bsp: &bsp::Bsp) -> BspBuffer {
        let lightmap = self.load_lightmap(bsp);
        lightmap.debug_visualization();

        BspBuffer {
            worldspawn: self.load_worldspawn(bsp, &lightmap),
            entities: self.load_entities(bsp, &lightmap),
            lightmap: lightmap.into(),
        }
    }
}

struct LightmapDimensions {
    width: i32,
    height: i32,
    min_u: i32,
    min_v: i32,
}