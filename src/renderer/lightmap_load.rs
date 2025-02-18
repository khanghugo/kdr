use std::collections::HashMap;

use image::{GenericImage, RgbaImage};

use crate::renderer::utils::{face_vertices, get_lightmap_dimensions, vertex_uv};

use super::RenderContext;

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

impl RenderContext {
    pub fn load_lightmap(&self, bsp: &bsp::Bsp) -> LightMapAtlasBuffer {
        // let's do 4K
        // todo: multiple atlases
        const DIMENSION: u32 = 4096;
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
                .unwrap();

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

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
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

        self.queue.write_texture(
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

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture same name sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &self.texture_bind_group_layout,
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