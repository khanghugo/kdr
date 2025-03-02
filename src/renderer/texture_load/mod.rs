pub mod texture;
pub mod texture_array;

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
