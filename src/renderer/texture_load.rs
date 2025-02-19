use image::RgbaImage;

use super::{RenderContext, utils::eightbpp_to_rgba8};

pub struct TextureBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

// impl drop will does some unholy shit
// basically it is supposed to be RA so don't destroy it before RA
// impl Drop for TextureBuffer {
//     fn drop(&mut self) {
//         self.texture.destroy();
//     }
// }

pub struct BspMipTex {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl Drop for BspMipTex {
    fn drop(&mut self) {
        self.texture.destroy();
    }
}

impl BspMipTex {
    /// For this layout, we will use the color index as the alpha channel.
    ///
    /// With this, we can do masked rendering
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("miptex bind group layout descriptor"),
            entries: &[
                // texture
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
                // linear sampler
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
    pub fn load_rgba8(&self, img: &RgbaImage) -> TextureBuffer {
        let (width, height) = img.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture same name"),
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
            &img,
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

        let linear_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
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
                // linear sampler
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
        });

        TextureBuffer {
            texture,
            view,
            bind_group,
        }
    }
    pub fn load_miptex(&self, miptex: &bsp::Texture) -> BspMipTex {
        // TODO: maybe this needs checking??
        let mip_image = &miptex.mip_images[0];
        let img = eightbpp_to_rgba8(
            mip_image.data.get_bytes(),
            miptex.palette.get_bytes(),
            miptex.width,
            miptex.height,
        );

        let TextureBuffer {
            texture,
            view,
            bind_group,
        } = self.load_rgba8(&img);

        BspMipTex {
            texture,
            view,
            bind_group,
        }
    }
}
