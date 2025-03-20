use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use wgpu::util::DeviceExt;

use crate::renderer::{
    bsp_lightmap::LightMapAtlasAllocation, texture_buffer::texture::TextureBuffer,
};

use super::{
    bsp_lightmap::LightMapAtlasBuffer,
    camera::Camera,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
    utils::{eightbpp_to_rgba8, face_vertices, vertex_uv},
};

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct BspVertex {
    pos: [f32; 3],
    tex_coord: [f32; 2],
    lightmap_coord: [f32; 2],
    layer: u32, // texture idx from texture array
    renderamt: f32,
}

impl BspVertex {
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
                // layer
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 28,
                    shader_location: 3,
                },
                // renderamt
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 32,
                    shader_location: 4,
                },
            ],
        }
    }
}

pub struct BspBuffer {
    pub opaque: Vec<BspVertexBuffer>,
    // only 1 buffer because we don't need to sort for transparency
    pub transparent: Vec<BspVertexBuffer>,
    pub textures: Vec<TextureArrayBuffer>,
    pub lightmap: LightMapAtlasBuffer,
}

pub struct BspVertexBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: usize,
    pub texture_array_index: usize,
}

impl Drop for BspVertexBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

pub struct BspLoader;

impl BspLoader {
    fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
        opaque: bool,
    ) -> wgpu::RenderPipeline {
        let bsp_shader = device.create_shader_module(wgpu::include_wgsl!("./shader/bsp.wgsl"));

        let texture_array_bind_group_layout =
            device.create_bind_group_layout(&TextureArrayBuffer::bind_group_layout_descriptor());

        let camera_bind_group_layout =
            device.create_bind_group_layout(&Camera::bind_group_layout_descriptor());

        let lightmap_bind_group_layout =
            device.create_bind_group_layout(&LightMapAtlasBuffer::bind_group_layout_descriptor());

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &texture_array_bind_group_layout,
                &lightmap_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        let bsp_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bsp render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bsp_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[BspVertex::buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bsp_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &fragment_targets
                    .into_iter()
                    .map(|v| Some(v))
                    .collect::<Vec<Option<wgpu::ColorTargetState>>>(),
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Cw,
                cull_mode: Some(wgpu::Face::Back),
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: if opaque { true } else { false },
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        bsp_render_pipeline
    }

    pub fn create_render_pipeline_opaque(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
    ) -> wgpu::RenderPipeline {
        Self::create_render_pipeline(device, fragment_targets, true)
    }

    pub fn load_bsp(device: &wgpu::Device, queue: &wgpu::Queue, bsp: &bsp::Bsp) -> BspBuffer {
        let lightmap = LightMapAtlasBuffer::load_lightmap(device, queue, bsp);
        let (texture_lookup_table, texture_arrays) = Self::load_textures(device, queue, &bsp);

        // get opaque and transparent entities
        let (opaque_entities, transparent_entities): (Vec<_>, _) = bsp
            .entities
            .iter()
            // first need to filter out non-brush entities
            .filter(|entity| {
                // manually add worldspawn because it doesnt have "model" key
                if let Some(classname) = entity.get("classname") {
                    if classname == "worldspawn" {
                        return true;
                    }
                }

                if let Some(model) = entity.get("model") {
                    // only brush models have "*" prefix
                    if model.starts_with("*") {
                        return true;
                    }
                }

                // if an entity doesnt have a model, it is probably something not important
                false
            })
            // now partition the entities
            .partition(|entity| {
                if let Some(rendermode) = entity
                    .get("rendermode")
                    .and_then(|rendermode| rendermode.parse::<i32>().ok())
                {
                    // rendermode 0 = normal
                    // rendermode 4 = solid aka alpha test
                    let is_solid = [0, 4].contains(&rendermode);

                    if is_solid {
                        return true;
                    }

                    // check for renderamt
                    if let Some(renderamt) = entity
                        .get("renderamt")
                        .and_then(|renderamt| renderamt.parse::<f32>().ok())
                    {
                        let is_255 = renderamt >= 255.;

                        // 255 means no need to bother blending
                        if is_255 {
                            return true;
                        }
                    }

                    // renderamt is defaulted to be 0 when entity is spawned
                    return false;
                }

                // no rendermode, could be world brush
                true
            });

        // now load the faces from the models
        // need to add worldspawn manually into list of opaque entities
        let opaque_entities_faces = opaque_entities.iter().flat_map(|entity| {
            let is_worldspawn = entity
                .get("classname")
                .is_some_and(|classname| classname == "worldspawn");

            let model_idx = if is_worldspawn {
                0
            } else {
                entity.get("model")
            // we know for sure that this is a brush
            .unwrap()
            // remove the asterisk
            [1..]
                    .parse::<u32>()
                    .expect("cannot find entity model")
            };

            let model = &bsp.models[model_idx as usize];

            let first_face = model.first_face as usize;
            let face_count = model.face_count as usize;

            let faces = &bsp.faces[first_face..(first_face + face_count)];

            faces
                .into_iter()
                .enumerate()
                .map(|(offset, face)| ProcessFaceData {
                    index: first_face + offset,
                    face,
                    custom_render: None,
                })
                // constraints? what gives
                .collect::<Vec<ProcessFaceData>>()
        });

        let opaque_entities_buffer = Self::load_polygons(
            device,
            bsp,
            opaque_entities_faces,
            &lightmap,
            &texture_lookup_table,
        );

        let transparent_entities_faces = transparent_entities.iter().flat_map(|entity| {
            let model_idx = entity.get("model")
            // we know for sure that this is a brush
            .unwrap()
            // remove the asterisk
            [1..]
                .parse::<u32>()
                .expect("cannot find entity model");

            let model = &bsp.models[model_idx as usize];

            let first_face = model.first_face as usize;
            let face_count = model.face_count as usize;

            let faces = &bsp.faces[first_face..(first_face + face_count)];

            // the value is guaranteed to be there
            let rendermode = entity
                .get("rendermode")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap();
            let renderamt = entity
                .get("renderamt")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0);
            let renderfx = entity
                .get("renderfx")
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);

            faces
                .into_iter()
                .enumerate()
                .map(|(offset, face)| ProcessFaceData {
                    index: first_face + offset,
                    face,
                    custom_render: Some(CustomRender {
                        rendermode,
                        renderamt,
                        renderfx,
                    }),
                })
                // constraints? what gives
                .collect::<Vec<ProcessFaceData>>()
        });

        let transparent_entities_buffer = Self::load_polygons(
            device,
            bsp,
            transparent_entities_faces,
            &lightmap,
            &texture_lookup_table,
        );

        BspBuffer {
            opaque: opaque_entities_buffer,
            transparent: transparent_entities_buffer,
            textures: texture_arrays,
            lightmap,
        }
    }

    fn load_polygons<'a, T>(
        device: &wgpu::Device,
        bsp: &'a bsp::Bsp,
        faces_data: T,
        lightmap: &LightMapAtlasBuffer,
        texture_lookup_table: &[(usize, usize)],
    ) -> Vec<BspVertexBuffer>
    where
        T: Iterator<Item = ProcessFaceData<'a>>,
    {
        // (array index, (interleaved vertex data, vertex indices))
        // layer index is inside interleaved data
        let mut batches = HashMap::<usize, (Vec<BspVertex>, Vec<u32>)>::new();

        for face_data in faces_data {
            let face = face_data.face;

            let texinfo = &bsp.texinfo[face.texinfo as usize];
            let (array_idx, layer_idx) = texture_lookup_table[texinfo.texture_index as usize];

            let (vertices, indices) = process_bsp_face(&face_data, bsp, lightmap, layer_idx);

            let batch = batches.entry(array_idx).or_insert((Vec::new(), Vec::new()));

            // newer vertices will have their index start at 0 but we don't want that
            // need to divide by <x> because each "vertices" has <x> floats
            let new_vertices_offset = batch.0.len();

            batch.0.extend(vertices);
            batch
                .1
                .extend(indices.into_iter().map(|i| i + new_vertices_offset as u32));
        }

        let batches = batches
            .into_iter()
            .map(|(texture_array_index, (vertices, indices))| {
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bsp vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("bsp index buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                BspVertexBuffer {
                    vertex_buffer,
                    index_buffer,
                    index_count: indices.len(),
                    texture_array_index,
                }
            })
            .collect();

        batches
    }

    fn load_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bsp: &bsp::Bsp,
    ) -> (Vec<(usize, usize)>, Vec<TextureArrayBuffer>) {
        let bsp_textures: Vec<RgbaImage> = bsp
            .textures
            .iter()
            .map(|texture| {
                let texture_name = texture.texture_name.get_string_standard();
                let override_alpha = if texture_name == "SKY" {
                    16.into()
                } else {
                    None
                };

                eightbpp_to_rgba8(
                    texture.mip_images[0].data.get_bytes(),
                    texture.palette.get_bytes(),
                    texture.width,
                    texture.height,
                    override_alpha,
                )
            })
            .collect();

        // this bucket groups all textures with the same dimension together
        // key is dimension
        // value is Vec<texture index>
        let mut buckets: HashMap<(u32, u32), Vec<usize>> = HashMap::new();

        // no more comments here
        // go look at [`MdlLoader::load_textures`]
        bsp_textures
            .iter()
            .enumerate()
            .for_each(|(texture_idx, texture)| {
                buckets
                    .entry(texture.dimensions())
                    .or_insert(vec![])
                    .push(texture_idx);
            });

        // index is texture index
        // element is the bucket index and layer index
        let mut lookup_table: Vec<(usize, usize)> = vec![(0, 0); bsp_textures.len()];

        let texture_arrays: Vec<TextureArrayBuffer> = buckets
            .iter()
            .enumerate()
            .map(|(bucket_idx, (_, texture_indices))| {
                texture_indices
                    .iter()
                    .enumerate()
                    .for_each(|(layer_idx, &texture_index)| {
                        lookup_table[texture_index] = (bucket_idx, layer_idx);
                    });

                let ref_vec = texture_indices
                    .iter()
                    .map(|&texture_index| &bsp_textures[texture_index])
                    .collect::<Vec<_>>();

                create_texture_array(device, queue, &ref_vec).unwrap()
            })
            .collect();

        (lookup_table, texture_arrays)
    }
}

struct ProcessFaceData<'a> {
    index: usize,
    face: &'a bsp::Face,
    custom_render: Option<CustomRender>,
}

struct CustomRender {
    rendermode: i32,
    renderamt: f32,
    renderfx: i32,
}

/// Returns (interleaved vertex data, vertex indices)
fn process_bsp_face(
    face_data: &ProcessFaceData,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
    texture_layer_idx: usize,
) -> (Vec<BspVertex>, Vec<u32>) {
    let ProcessFaceData {
        index: face_idx,
        face,
        custom_render,
    } = face_data;

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
    let vertices: Vec<BspVertex> = face_vertices
        .into_iter()
        .zip(vertices_normalized_texcoords.into_iter())
        .zip(lightmap_texcoords.into_iter())
        .map(|((pos, texcoord), lightmap_coord)| BspVertex {
            pos: pos.to_array().into(),
            tex_coord: texcoord.into(),
            lightmap_coord: lightmap_coord.into(),
            layer: texture_layer_idx as u32,
            renderamt: custom_render
                .as_ref()
                .map(|v| v.renderamt / 255.0)
                // amount 1 = 255 = full aka opaque
                // transparent objects have their own default so this won't be a problem
                .unwrap_or(1.0),
        })
        .collect();

    (vertices, indices)
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
