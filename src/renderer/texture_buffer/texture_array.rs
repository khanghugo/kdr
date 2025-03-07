pub struct TextureArrayBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl TextureArrayBuffer {
    pub fn bind_group_layout_descriptor() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
            label: Some("texture array bind group layout descriptor"),
            entries: &[
                // texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                // sampler
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

use eyre::eyre;
use image::RgbaImage;

use super::mipmap::{MipMapGenerator, calculate_mipmap_count};

// this is assuming that they all have the same dimensions
pub fn create_texture_array(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    textures: &[&RgbaImage],
) -> eyre::Result<TextureArrayBuffer> {
    // some checks just to make sure
    if textures.is_empty() {
        return Err(eyre!("texture array length is 0"));
    }

    let tex0 = &textures[0];
    let (width, height) = tex0.dimensions();

    if !textures
        .iter()
        .all(|texture| tex0.dimensions() == texture.dimensions())
    {
        return Err(eyre!("not all textures have the same length"));
    }

    let mip_level_count = calculate_mipmap_count(width, height);
    let texture_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mipmap_generator = MipMapGenerator::create_render_pipeline(device, queue, texture_format);

    let texture_descriptor = wgpu::TextureDescriptor {
        label: Some("texture array descriptor"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: textures.len() as u32,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT, // to generate mipmap
        view_formats: &[],
    };

    let texture_array = device.create_texture(&texture_descriptor);

    textures
        .iter()
        .enumerate()
        .for_each(|(layer_idx, texture)| {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture_array,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer_idx as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &texture,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        });

    mipmap_generator.generate_mipmaps_texture_array(
        &texture_array,
        mip_level_count,
        textures.len() as u32,
    );

    // bind layout
    let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("texture array sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        anisotropy_clamp: 16,
        lod_min_clamp: 0.0,
        lod_max_clamp: 2.0, // change the max mipmap level here
        ..Default::default()
    });

    let view = texture_array.create_view(&wgpu::TextureViewDescriptor {
        label: Some("texture array view"),
        format: None,
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(mip_level_count),
        base_array_layer: 0,
        array_layer_count: None,
        usage: None,
    });

    let bind_group_layout =
        device.create_bind_group_layout(&TextureArrayBuffer::bind_group_layout_descriptor());

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("texture bind group"),
        layout: &bind_group_layout,
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

    Ok(TextureArrayBuffer {
        texture: texture_array,
        view,
        bind_group,
    })
}
