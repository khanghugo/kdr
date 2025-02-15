use wgpu::Extent3d;

use super::{RenderContext, types::TextureBuffer, utils::eightbpp_to_rgba8};

pub struct BspMipTex {
    // todo miplevels
    pub index_texture: wgpu::Texture,
    pub palette_texture: wgpu::Texture,
    pub index_view: wgpu::TextureView,
    pub palette_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl BspMipTex {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("miptex bind group layout descriptor"),
            entries: &[
                // index view
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        // when fed to shader, u8 becomes u32
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // index sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    // disable filtering because we don't want interpolation and it doesnt even exist
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // palette texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        // color is srgb and converted to float in shader stage
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D1,
                        multisampled: false,
                    },
                    count: None,
                },
                // palette sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }
    }
}

impl RenderContext {
    pub fn load_miptex_to_rgba8(&self, miptex: &bsp::Texture) -> TextureBuffer {
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

    pub fn load_miptex(&self, mip_tex: &bsp::Texture) -> BspMipTex {
        // TODO: maybe this needs checking??
        let mip_image = &mip_tex.mip_images[0];
        // let rgba8 = eightbpp_to_rgba8(
        //     mip_image.data.get_bytes(),
        //     miptex.palette.get_bytes(),
        //     miptex.width,
        //     miptex.height,
        // );

        let index_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loading miptex"),
            size: wgpu::Extent3d {
                width: mip_tex.width,
                height: mip_tex.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, // TODO mip levels
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let palette_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loading palette"),
            size: wgpu::Extent3d {
                width: 256, // 256
                height: 1,  // * 1 = 256 entries
                depth_or_array_layers: 1,
            },
            mip_level_count: 1, // TODO mip levels
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // remember to add alpha channel
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &index_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            mip_image.data.get_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(mip_tex.width), // just u8 index
                rows_per_image: Some(mip_tex.height),
            },
            wgpu::Extent3d {
                width: mip_tex.width,
                height: mip_tex.height,
                depth_or_array_layers: 1,
            },
        );

        let palette: Vec<[u8; 4]> = mip_tex
            .palette
            .get_bytes()
            .iter()
            // 255 but not 1.0 because the color is specified to be srgb
            .map(|color| [color[0], color[1], color[2], 255])
            .collect();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &palette_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&palette),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                // hopefully it is always 256 colors because people have been fucking over the wad format
                // *4 because we have alpha channel now
                bytes_per_row: Some(256 * 4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let index_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture same name sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let palette_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("texture same name sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let index_view = index_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let palette_view = palette_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("miptex index bind group"),
            layout: &self.texture_bind_group_layout,
            entries: &[
                // index view
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&index_view),
                },
                // index sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&index_sampler),
                },
                // palette view
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&palette_view),
                },
                // palette sampler
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&palette_sampler),
                },
            ],
        });

        BspMipTex {
            index_texture,
            palette_texture,
            index_view,
            palette_view,
            bind_group,
        }
    }
}
