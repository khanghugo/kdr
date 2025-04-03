use std::sync::Arc;

pub struct RenderTargets {
    pub main_texture: wgpu::Texture,
    pub main_view: wgpu::TextureView,
    // need a composite texture because wgpu cannot sample a texture while writing on it
    // it happens when we want to do post processing, we have to read the current image and then write on it
    pub composite_texture: wgpu::Texture,
    pub composite_view: wgpu::TextureView,
    pub depth_texture: Arc<wgpu::Texture>,
    pub depth_view: wgpu::TextureView,
}

impl RenderTargets {
    pub fn main_texture_format() -> wgpu::TextureFormat {
        wgpu::TextureFormat::Rgba16Float
        // wgpu::TextureFormat::Bgra8Unorm
    }

    pub fn composite_texture_format() -> wgpu::TextureFormat {
        Self::main_texture_format()
    }

    pub fn depth_texture_format() -> wgpu::TextureFormat {
        wgpu::TextureFormat::Depth32FloatStencil8
    }

    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let main_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("main render texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::main_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let main_view = main_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // depth stuffs
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::depth_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_texture = Arc::new(depth_texture);

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let composite_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("composite texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::main_texture_format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let composite_view = composite_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            main_texture,
            main_view,
            depth_texture,
            depth_view,
            composite_texture,
            composite_view,
        }
    }
}

impl Drop for RenderTargets {
    fn drop(&mut self) {
        self.main_texture.destroy();
        self.depth_texture.destroy();
        self.composite_texture.destroy();
    }
}
