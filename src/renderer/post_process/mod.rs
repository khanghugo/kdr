use bloom::Bloom;
use gray_scale::GrayScale;
use pp_trait::PostProcessingModule;

use super::utils::FullScrenTriVertexShader;

mod bloom;
mod gray_scale;
mod pp_trait;

pub struct PostProcessing {
    effects: Vec<PostEffect>,
    intermediate_textures: [wgpu::Texture; 2],
    intermediate_views: [wgpu::TextureView; 2],
}

pub enum PostEffect {
    GrayScale(GrayScale),
    Bloom(Bloom),
}

impl PostProcessing {
    pub fn create_pipelines(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
    ) -> Self {
        let create_texture = || {
            device.create_texture(&wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: input_texture_format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
                label: Some("post processing intermediate texture"),
            })
        };

        let tex0 = create_texture();
        let tex1 = create_texture();

        let mut res = Self {
            effects: vec![],
            intermediate_views: [
                tex0.create_view(&wgpu::TextureViewDescriptor::default()),
                tex1.create_view(&wgpu::TextureViewDescriptor::default()),
            ],
            intermediate_textures: [tex0, tex1],
        };

        // res.add_effect(PostEffect::GrayScale(GrayScale::new(
        //     device,
        //     input_texture_format,
        //     fullscreen_tri_vertex_shader,
        // )));

        // res.add_effect(PostEffect::Bloom(Bloom::new2(
        //     device,
        //     input_texture_format,
        //     fullscreen_tri_vertex_shader,
        //     width,
        //     height,
        // )));

        res
    }

    pub fn add_effect<T: Into<PostEffect>>(&mut self, effect: T) {
        self.effects.push(effect.into());
    }

    pub fn execute(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        // main render
        input_texture: &wgpu::Texture,
        // composite
        output_texture: &wgpu::Texture,
    ) {
        let effect_count = self.effects.len();

        // if no effect, copy from input (main render) to output (composite) directly
        if self.effects.is_empty() {
            encoder.copy_texture_to_texture(
                input_texture.as_image_copy(),
                output_texture.as_image_copy(),
                input_texture.size(),
            );
            return;
        }

        // ping pong with two intermediate textures
        // the reason why this even happens is because we have a lot of effects
        // and we want to chain them easily
        // we cannot write to the input texture so we have an output texture
        // so we need two intermediate textures to do things
        let mut current_input_texture = input_texture;
        let mut current_intermediate_output_texture = &self.intermediate_textures[0];

        // effect_count is at least 1 here
        for (effect_index, effect) in self.effects.iter_mut().enumerate() {
            let is_last = effect_index == effect_count - 1;

            // if last, the output must be the specified output in the execute function
            current_intermediate_output_texture = if is_last {
                output_texture
            } else {
                current_intermediate_output_texture
            };

            match effect {
                PostEffect::GrayScale(x) => {
                    x.execute(
                        device,
                        encoder,
                        current_input_texture,
                        current_intermediate_output_texture,
                    );
                }
                PostEffect::Bloom(x) => {
                    x.bloom(
                        device,
                        encoder,
                        current_input_texture,
                        current_intermediate_output_texture,
                    );
                }
            };

            // ping pong intermediate textures
            // this condition means we have at least 2 effects
            if !is_last {
                current_input_texture = current_intermediate_output_texture;
                current_intermediate_output_texture =
                    &self.intermediate_textures[(effect_index + 1) % 2];
            }
        }
    }
}
