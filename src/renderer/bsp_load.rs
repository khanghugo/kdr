use std::collections::HashMap;

use cgmath::{Point2, Point3};
use wgpu::util::DeviceExt;

use super::{
    RenderContext,
    lightmap_load::LightMapAtlasBuffer,
    utils::{face_vertices, vertex_uv},
};

#[repr(C)]
pub struct BspVertex {
    pos: Point3<f32>,
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
                // tex
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                // lightmap
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 20,
                    shader_location: 2,
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
        // (texture index, (interleaved vertex data, vertex indices))
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

    fn load_entities(
        &self,
        bsp: &bsp::Bsp,
        lightmap: &LightMapAtlasBuffer,
    ) -> Option<BspEntitiesBuffer> {
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
        // TODO: use texture array
        // bsp or static entities in general benefit more from texture array
        // because they dont need animating
        let lightmap = self.load_lightmap(bsp);
        lightmap.debug_visualization();

        BspBuffer {
            worldspawn: self.load_worldspawn(bsp, &lightmap),
            entities: self.load_entities(bsp, &lightmap),
            lightmap: lightmap.into(),
        }
    }
}

/// Returns (interleaved vertex data, vertex indices)
fn process_face(
    face: &bsp::Face,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
    face_idx: usize,
) -> (Vec<f32>, Vec<u32>) {
    let face_vertices = face_vertices(face, bsp);

    let indices = triangulate_convex_polygon(&face_vertices);

    // very inefficient right now
    // becuase all vertices here have the same normal
    let normal = bsp.planes[face.plane as usize].normal;
    let texinfo = &bsp.texinfo[face.texinfo as usize];

    // uv
    let miptex = &bsp.textures[texinfo.texture_index as usize];

    let vertices_texcoords: Vec<[f32; 2]> = face_vertices
        .iter()
        .map(|pos| vertex_uv(pos, &texinfo))
        .collect();

    let vertices_normalized_texcoords: Vec<[f32; 2]> = vertices_texcoords
        .iter()
        .map(|uv| [uv[0] / miptex.width as f32, uv[1] / miptex.height as f32])
        .collect();

    // lightmap
    let what = lightmap.allocations.get(&face_idx);

    // https://github.com/magcius/noclip.website/blob/66595465295720f8078a53d700988241b0adc2b0/src/GoldSrc/BSPFile.ts#L285
    let (face_min_u, _face_max_u, face_min_v, _face_max_v) = get_face_uv_box(&vertices_texcoords);

    let lightmap_texcoords: Vec<[f32; 2]> = if let Some(allocation) = what {
        let lightmap_texcoords = vertices_texcoords.iter().map(|&[u, v, ..]| {
            let lightmap_u =
                ((u / 16.0) - (face_min_u / 16.0).floor() + 0.5) / allocation.lightmap_width;
            let lightmap_v =
                ((v / 16.0) - (face_min_v / 16.0).floor() + 0.5) / allocation.lightmap_height;

            [
                allocation.atlas_x + lightmap_u * allocation.atlas_width,
                allocation.atlas_y + lightmap_v * allocation.atlas_height,
            ]
        });

        lightmap_texcoords.collect()
    } else {
        vertices_normalized_texcoords
            .iter()
            .map(|_| [0., 0.])
            .collect()
    };

    // collect to buffer
    let interleaved: Vec<f32> = face_vertices
        .into_iter()
        .zip(vertices_normalized_texcoords.into_iter())
        .zip(lightmap_texcoords.into_iter())
        .flat_map(|((pos, texcoord), lightmap_coord)| {
            [
                // no need to flip any of the geometry
                // we will do that to the camera
                // -pos.x,
                // pos.z, // flip y and z because the game is z up
                // pos.y,
                pos.x,
                pos.y,
                pos.z,
                // normal.x,
                // normal.y,
                // normal.z,
                texcoord[0],
                texcoord[1],
                lightmap_coord[0],
                lightmap_coord[1],
            ]
        })
        .collect();

    (interleaved, indices)
}

// the dimension of the face on texture coordinate
fn get_face_uv_box(uvs: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    let mut min_u = uvs[0][0];
    let mut min_v = uvs[0][1];
    let mut max_u = uvs[0][0];
    let mut max_v = uvs[0][1];

    for i in 1..uvs.len() {
        let u = uvs[i][0];
        let v = uvs[i][1];

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

    return (min_u, max_u, min_v, max_v);
}

// deepseek wrote this
// input is a winding order polygon
// the output is the vertex index
// so there is no need to do anythign to the vertex buffer, only play around with the index buffer
fn triangulate_convex_polygon(vertices: &[bsp::Vec3]) -> Vec<u32> {
    // For convex polygons, we can simply fan-triangulate from the first vertex
    // This creates triangle indices: 0-1-2, 0-2-3, 0-3-4, etc.
    assert!(vertices.len() >= 3, "Polygon needs at least 3 vertices");

    let mut indices = Vec::with_capacity((vertices.len() - 2) * 3);
    for i in 1..vertices.len() - 1 {
        indices.push(0);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    indices
}
