use std::collections::HashMap;

use image::{GenericImage, RgbaImage};

use crate::renderer::utils::{face_vertices, get_lightmap_dimensions, vertex_uv};

use super::RenderContext;

pub struct LightMapAtlasAllocation {
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_scale: f32,
    pub y_scale: f32,
}

pub struct LightMapAtlasBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub allocations: HashMap<usize, LightMapAtlasAllocation>,
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
}

impl RenderContext { 
    pub fn load_lightmap(&self, bsp: &bsp::Bsp) -> LightMapAtlasBuffer {
        // let's do 4K
        // todo: multiple atlases
        const DIMENSION: u32 = 4096;

        let mut atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(
            DIMENSION as i32,
            DIMENSION as i32,
        ));
        let mut allocations = HashMap::new();

        let mut atlas_image = RgbaImage::new(DIMENSION, DIMENSION);

        bsp.faces.iter().enumerate().for_each(|(idx, face)| {
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

            let allocation = atlas
                .allocate(guillotiere::size2(
                    lightmap_dimensions.width,
                    lightmap_dimensions.height,
                ))
                .unwrap();

            let atlas_allocation = LightMapAtlasAllocation {
                x_offset: allocation.rectangle.min.x as f32 / DIMENSION as f32,
                y_offset: allocation.rectangle.min.y as f32 / DIMENSION as f32,
                x_scale: allocation.rectangle.width() as f32 / DIMENSION as f32,
                y_scale: allocation.rectangle.height() as f32 / DIMENSION as f32,
            };

            allocations.insert(idx, atlas_allocation);

            let lightmap_run_end = lightmap_dimensions.height * lightmap_dimensions.width;
            let tupled_offset = face.lightmap_offset as usize / 3; // the the original offset is on byte but we have rgb

            let lightmap_run =
                &bsp.lightmap[tupled_offset..(tupled_offset + lightmap_run_end as usize)];

            for y in 0..allocation.rectangle.height() {
                for x in 0..allocation.rectangle.width() {
                    let curr_element = x + y * allocation.rectangle.width();
                    let curr_pixel = lightmap_run[curr_element as usize];
                    let curr_rgba = [curr_pixel[0], curr_pixel[1], curr_pixel[2], 255];
                    atlas_image.put_pixel(
                        (x + allocation.rectangle.min.x) as u32,
                        (y + allocation.rectangle.min.y) as u32,
                        image::Rgba(curr_rgba),
                    );
                }
            }
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
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
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
