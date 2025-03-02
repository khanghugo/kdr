use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use wgpu::util::DeviceExt;

use crate::renderer::texture_buffer::texture::TextureBuffer;

use super::{
    camera::Camera,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
    utils::{eightbpp_to_rgba8, face_vertices, vertex_uv},
};

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
            ],
        }
    }
}

pub struct BspBuffer {
    pub worldspawn: Vec<BspVertexBuffer>,
    pub entities: Vec<Vec<BspVertexBuffer>>,
    pub textures: Vec<TextureArrayBuffer>,
    pub lightmap: LightMapAtlasBuffer,
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct BspVertex {
    pos: [f32; 3],
    tex_coord: [f32; 2],
    lightmap_coord: [f32; 2],
    layer: u32, // texture idx from texture array
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

#[derive(Debug)]
pub struct LightMapAtlasAllocation {
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_width: f32,
    pub atlas_height: f32,
    pub min_x: f32,
    pub min_y: f32,
    pub lightmap_width: f32,
    pub lightmap_height: f32,
}

pub struct LightMapAtlasBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub allocations: HashMap<usize, LightMapAtlasAllocation>,
}

impl Drop for LightMapAtlasBuffer {
    fn drop(&mut self) {
        self.texture.destroy();
    }
}

impl LightMapAtlasBuffer {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("light map bind group layout descriptor"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }
    }

    pub fn debug_visualization(&self) {
        let size = self.texture.size();
        let mut img = RgbaImage::new(size.width, size.height);

        // Draw allocation borders
        for allocation in self.allocations.values() {
            let x_start = (allocation.atlas_x * size.width as f32) as u32;
            let y_start = (allocation.atlas_y * size.height as f32) as u32;
            let width = (allocation.atlas_width * size.width as f32) as u32;
            let height = (allocation.atlas_height * size.height as f32) as u32;

            // Draw red border
            for x in x_start..x_start + width {
                img.put_pixel(x, y_start, image::Rgba([255, 0, 0, 255]));
                img.put_pixel(x, y_start + height - 1, image::Rgba([255, 0, 0, 255]));
            }
            for y in y_start..y_start + height {
                img.put_pixel(x_start, y, image::Rgba([255, 0, 0, 255]));
                img.put_pixel(x_start + width - 1, y, image::Rgba([255, 0, 0, 255]));
            }
        }

        img.save("examples/lightmap_debug.png").unwrap();
    }
}

pub struct BspLoader;

impl BspLoader {
    pub fn create_render_pipeline(
        device: &wgpu::Device,
        fragment_targets: Vec<wgpu::ColorTargetState>,
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
                depth_write_enabled: true,
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

    pub fn load_bsp(device: &wgpu::Device, queue: &wgpu::Queue, bsp: &bsp::Bsp) -> BspBuffer {
        let lightmap = Self::load_lightmap(device, queue, bsp);
        let (texture_lookup_table, texture_arrays) = Self::load_textures(device, queue, &bsp);
        let worldspawn = Self::load_worldspawn(device, bsp, &lightmap, &texture_lookup_table);

        // TODO make it array of entities as it should be
        let entities = Self::load_entities(device, bsp, &lightmap, &texture_lookup_table);

        BspBuffer {
            worldspawn,
            entities: vec![entities],
            textures: texture_arrays,
            lightmap,
        }
    }

    fn load_worldspawn(
        device: &wgpu::Device,
        bsp: &bsp::Bsp,
        lightmap: &LightMapAtlasBuffer,
        texture_lookup_table: &[(usize, usize)],
    ) -> Vec<BspVertexBuffer> {
        let worldspawn = &bsp.models[0];
        let faces = &bsp.faces[worldspawn.first_face as usize
            ..(worldspawn.first_face as usize + worldspawn.face_count as usize)];

        Self::load_polygons(
            device,
            bsp,
            faces.iter().enumerate(),
            lightmap,
            texture_lookup_table,
        )
    }

    fn load_entities(
        device: &wgpu::Device,
        bsp: &bsp::Bsp,
        lightmap: &LightMapAtlasBuffer,
        texture_lookup_table: &[(usize, usize)],
    ) -> Vec<BspVertexBuffer> {
        // TODO sort all of the vertices later
        let rest = &bsp.models[1..];

        if rest.is_empty() {
            return vec![];
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

        Self::load_polygons(
            device,
            bsp,
            entity_faces
                .iter()
                .enumerate()
                .map(|(idx, e)| (idx + first_entity_face as usize, e)),
            lightmap,
            texture_lookup_table,
        )
    }

    fn load_polygons<'a, T>(
        device: &wgpu::Device,
        bsp: &bsp::Bsp,
        faces: T,
        lightmap: &LightMapAtlasBuffer,
        texture_lookup_table: &[(usize, usize)],
    ) -> Vec<BspVertexBuffer>
    where
        T: Iterator<Item = (usize, &'a bsp::Face)>,
    {
        // (array index, (interleaved vertex data, vertex indices))
        // layer index is inside interleaved data
        let mut batches = HashMap::<usize, (Vec<BspVertex>, Vec<u32>)>::new();

        for (face_idx, face) in faces {
            let texinfo = &bsp.texinfo[face.texinfo as usize];
            let (array_idx, layer_idx) = texture_lookup_table[texinfo.texture_index as usize];

            let (vertices, indices) = process_face(face, bsp, lightmap, face_idx, layer_idx);

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
                eightbpp_to_rgba8(
                    texture.mip_images[0].data.get_bytes(),
                    texture.palette.get_bytes(),
                    texture.width,
                    texture.height,
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

    fn load_lightmap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bsp: &bsp::Bsp,
    ) -> LightMapAtlasBuffer {
        // let's do 4K
        // ~~todo: multiple atlases~~
        // no need, this dimension 1024 is enough to fit 64 allocblock
        const DIMENSION: u32 = 1024;
        const PADDING: i32 = 0;

        let mut atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            DIMENSION as i32,
            DIMENSION as i32,
        ));
        let mut allocations = HashMap::new();

        let mut atlas_image = RgbaImage::new(DIMENSION, DIMENSION);

        bsp.faces.iter().enumerate().for_each(|(idx, face)| {
            // let tex_info = &bsp.texinfo[face.texinfo as usize];
            // let texture = &bsp.textures[tex_info.texture_index as usize];
            if face.lightmap_offset == -1 {
                return;
            }

            let face_vertices = face_vertices(face, bsp);
            let texinfo = &bsp.texinfo[face.texinfo as usize];

            let vertices_texcoords: Vec<[f32; 2]> = face_vertices
                .iter()
                .map(|pos| vertex_uv(pos, &texinfo))
                .collect();

            let lightmap_dimensions = get_lightmap_dimensions(&vertices_texcoords);

            let alloc_width = lightmap_dimensions.width + 2 * PADDING;
            let alloc_height = lightmap_dimensions.height + 2 * PADDING;

            let allocation = atlas
                .allocate(guillotiere::size2(alloc_width, alloc_height))
                .expect("cannot allocate lightmap atlas");

            // very easy to get things wrong, dont touch too much
            let atlas_allocation = LightMapAtlasAllocation {
                atlas_x: (allocation.rectangle.min.x + PADDING) as f32 / DIMENSION as f32,
                atlas_y: (allocation.rectangle.min.y + PADDING) as f32 / DIMENSION as f32,
                atlas_width: (lightmap_dimensions.width) as f32 / DIMENSION as f32,
                atlas_height: (lightmap_dimensions.height) as f32 / DIMENSION as f32,
                // min_uv belongs to texture coordinate of the current face, not the lightmap
                min_x: lightmap_dimensions.min_u as f32,
                min_y: lightmap_dimensions.min_v as f32,
                lightmap_width: lightmap_dimensions.width as f32,
                lightmap_height: lightmap_dimensions.height as f32,
            };

            allocations.insert(idx, atlas_allocation);

            let lightmap_run_end = lightmap_dimensions.height * lightmap_dimensions.width;

            assert_eq!(face.lightmap_offset % 3, 0);
            let tupled_offset = face.lightmap_offset as usize / 3; // the the original offset is on byte but we have rgb

            let lightmap_run =
                &bsp.lightmap[tupled_offset..(tupled_offset + lightmap_run_end as usize)];

            // main texture
            for y in 0..(lightmap_dimensions.height) {
                for x in 0..(lightmap_dimensions.width) {
                    let curr_element = x + y * (lightmap_dimensions.width);
                    let curr_pixel = lightmap_run[curr_element as usize];
                    let curr_rgba = [curr_pixel[0], curr_pixel[1], curr_pixel[2], 255];

                    atlas_image.put_pixel(
                        (x + allocation.rectangle.min.x + PADDING) as u32,
                        (y + allocation.rectangle.min.y + PADDING) as u32,
                        image::Rgba(curr_rgba),
                    );
                }
            }

            {
                let original_width = lightmap_dimensions.width;
                let original_height = lightmap_dimensions.height;

                for y in 0..alloc_height {
                    for x in 0..alloc_width {
                        // Only process padding areas
                        if x >= PADDING
                            && x < alloc_width - PADDING
                            && y >= PADDING
                            && y < alloc_height - PADDING
                        {
                            continue;
                        }

                        // Calculate source coordinates with mirroring
                        let src_x = (x - PADDING).clamp(0, original_width - 1).max(0);
                        let src_y = (y - PADDING).clamp(0, original_height - 1).max(0);

                        if let Some(pixel) = lightmap_run
                            .get(src_x as usize + src_y as usize * original_width as usize)
                        {
                            let dest_x = allocation.rectangle.min.x + x;
                            let dest_y = allocation.rectangle.min.y + y;
                            atlas_image.put_pixel(
                                dest_x as u32,
                                dest_y as u32,
                                image::Rgba([pixel[0], pixel[1], pixel[2], 255]),
                            );
                        }
                    }
                }
            }

            // border
            // {
            //     let border_color = [255, 0, 255, 255]; // Purple for visibility
            //     for x in allocation.rectangle.min.x..allocation.rectangle.max.x {
            //         for y in allocation.rectangle.min.y..allocation.rectangle.max.y {
            //             if x == allocation.rectangle.min.x
            //                 || x == allocation.rectangle.max.x - 1
            //                 || y == allocation.rectangle.min.y
            //                 || y == allocation.rectangle.max.y - 1
            //             {
            //                 atlas_image.put_pixel(x as u32, y as u32, image::Rgba(border_color));
            //             }
            //         }
            //     }
            // }
        });

        let (width, height) = atlas_image.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lightmap atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_image,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width), // rgba
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group_layout =
            device.create_bind_group_layout(&TextureBuffer::bind_group_layout_descriptor());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("light map sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("light map bind group"),
            layout: &bind_group_layout,
            entries: &[
                // texture
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                // sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        atlas_image.save("./examples/out.png");

        LightMapAtlasBuffer {
            texture,
            view,
            allocations,
            bind_group,
        }
    }
}

/// Returns (interleaved vertex data, vertex indices)
fn process_face(
    face: &bsp::Face,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
    face_idx: usize,
    texture_layer_idx: usize,
) -> (Vec<BspVertex>, Vec<u32>) {
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

#[derive(Debug)]
struct LightmapDimension {
    pub width: i32,
    pub height: i32,
    pub min_u: i32,
    pub min_v: i32,
}

// minimum light map size is always 2x2
// https://github.com/rein4ce/hlbsp/blob/1546eaff4e350a2329bc2b67378f042b09f0a0b7/js/hlbsp.js#L499
fn get_lightmap_dimensions(uvs: &[[f32; 2]]) -> LightmapDimension {
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

    // light map dimension is basically the face dimensions divided by 16
    // because luxel is 1 per 16 texel
    return LightmapDimension {
        width: ((max_u as f32 / 16.0).ceil() as i32) - ((min_u as f32 / 16.0).floor() as i32) + 1,
        height: ((max_v as f32 / 16.0).ceil() as i32) - ((min_v as f32 / 16.0).floor() as i32) + 1,
        min_u,
        min_v,
    };
}
