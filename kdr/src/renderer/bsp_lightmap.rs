use std::{collections::HashMap, io::Write};

use image::RgbaImage;

use crate::renderer::{
    texture_buffer::texture::TextureBuffer,
    utils::{face_vertices, vertex_uv},
};

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

    pub fn load_lightmap(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bsp: &bsp::Bsp,
    ) -> LightMapAtlasBuffer {
        // let's do 4K
        // ~~todo: multiple atlases~~
        // no need, this dimension 1024 is enough to fit 64 allocblock
        // FIXME: 1096 here just to make sure that all allocations work, not sure why
        const DIMENSION: u32 = 1096;
        const PADDING: i32 = 0;

        let atlas_options = guillotiere::AllocatorOptions {
            small_size_threshold: 4,
            large_size_threshold: 16,
            ..Default::default()
        };

        let mut atlas = guillotiere::AtlasAllocator::with_options(
            guillotiere::size2(DIMENSION as i32, DIMENSION as i32),
            &atlas_options,
        );

        let mut allocations = HashMap::new();

        let mut atlas_image = RgbaImage::new(DIMENSION, DIMENSION);

        let has_light_map = bsp.lightmap.len() > 1;

        if has_light_map {
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

                // TODO: fix kzkl_soraia420 some how
                // if the atlas is slightly bigger, like +32 in dimensions, we have 75% atlas used
                // but if it is 1024, we have 100+% used
                // maybe check with face with "-1" texture
                let allocation = atlas
                    .allocate(guillotiere::size2(alloc_width, alloc_height))
                    .unwrap_or_else(|| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let bytes: Vec<u8> = vec![];
                            let mut cursor = std::io::Cursor::new(bytes);
                            guillotiere::dump_svg(&atlas, &mut cursor).unwrap();

                            let out = "/home/khang/kdr/examples/out.svg";

                            let mut file = std::fs::OpenOptions::new()
                                .create(true)
                                .write(true)
                                .truncate(true)
                                .open(out)
                                .unwrap();

                            file.write_all(&cursor.into_inner()).unwrap();
                            file.flush().unwrap();
                        }

                        panic!(
                            "cannot allocate lightmap atlas. Happens to maps like kzkl_soraia420"
                        )
                    });

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

                // fixing edge cases for map like de_airstrip
                let lightmap_len = bsp.lightmap.len();

                let lightmap_run = &bsp.lightmap
                    [tupled_offset..(tupled_offset + lightmap_run_end as usize).min(lightmap_len)];

                // main texture
                for y in 0..(lightmap_dimensions.height) {
                    for x in 0..(lightmap_dimensions.width) {
                        let curr_element = x + y * (lightmap_dimensions.width);

                        // fixing edge cases for map like de_airstrip
                        if curr_element >= lightmap_run.len() as i32 {
                            continue;
                        }

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
            });
        } else {
            // just make it all white, otherwise, it will look bad
            atlas_image.fill(255u8);
        }

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

        // atlas_image.save("./examples/out.png");

        LightMapAtlasBuffer {
            texture,
            view,
            allocations,
            bind_group,
        }
    }
}

#[derive(Debug)]
struct LightmapDimension {
    pub width: i32,
    pub height: i32,
    pub min_u: i32,
    pub min_v: i32,
}

// https://github.com/magcius/noclip.website/blob/e748c03dbf626da5ae5f04868be410c3723724e2/src/GoldSrc/BSPFile.ts#L259
fn get_lightmap_dimensions(uvs: &[[f32; 2]]) -> LightmapDimension {
    let min_u = uvs.iter().min_by(|a, b| a[0].total_cmp(&b[0])).unwrap()[0];
    let min_v = uvs.iter().min_by(|a, b| a[1].total_cmp(&b[1])).unwrap()[1];
    let max_u = uvs.iter().max_by(|a, b| a[0].total_cmp(&b[0])).unwrap()[0];
    let max_v = uvs.iter().max_by(|a, b| a[1].total_cmp(&b[1])).unwrap()[1];

    const LIGHTMAP_SCALE: f32 = 1. / 16.;

    let lightmap_width =
        (max_u * LIGHTMAP_SCALE).ceil() as i32 - (min_u * LIGHTMAP_SCALE).floor() as i32 + 1;
    let lightmap_height =
        (max_v * LIGHTMAP_SCALE).ceil() as i32 - (min_v * LIGHTMAP_SCALE).floor() as i32 + 1;

    return LightmapDimension {
        width: lightmap_width,
        height: lightmap_height,
        // it has to be floor
        min_u: min_u.floor() as i32,
        min_v: min_v.floor() as i32,
    };
}
