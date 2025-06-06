use std::sync::Arc;

use bloom::Bloom;
use chromatic_aberration::ChromaticAberration;
use gray_scale::GrayScale;
use kuwahara::Kuwahara;
use posterize::Posterize;
use pp_trait::PostProcessingModule;

use super::utils::FullScrenTriVertexShader;

mod bloom;
mod chromatic_aberration;
mod gray_scale;
mod kuwahara;
mod posterize;
mod pp_trait;

pub struct PostProcessing {
    // effect and whether it is enabled or not
    // the reason why we do a vector in the first place is that it is easier to ping pong the effect
    // and the intermediate texture is correctly drawn over
    // doing individual effect stacking by name works, but that means the pipeline will run over nothing
    // that means we have to blit the image for no reasons
    // maybe that is good? i dont know, but i dont like extra work on gpu here
    effects: Vec<(bool, PostEffect)>,
    intermediate_textures: [wgpu::Texture; 2],
    input_texture_format: wgpu::TextureFormat,
}

impl Drop for PostProcessing {
    fn drop(&mut self) {
        self.intermediate_textures[0].destroy();
        self.intermediate_textures[1].destroy();
    }
}

pub enum PostEffect {
    Kuwahara(Kuwahara),
    Bloom(Bloom),
    ChromaticAberration(ChromaticAberration),
    GrayScale(GrayScale),
    Posterize(Posterize),
}

impl PostProcessing {
    pub fn create_intermediate_textures(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        input_texture_format: wgpu::TextureFormat,
    ) -> [wgpu::Texture; 2] {
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

        [tex0, tex1]
    }

    pub fn create_pipelines(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        input_texture_format: wgpu::TextureFormat,
        fullscreen_tri_vertex_shader: &FullScrenTriVertexShader,
        // i dont like reasoning with lifetime
        _depth_texture: Arc<wgpu::Texture>,
    ) -> Self {
        let [intermediate_texture0, intermediate_texture1] =
            Self::create_intermediate_textures(device, width, height, input_texture_format);

        let mut res = Self {
            effects: vec![],
            intermediate_textures: [intermediate_texture0, intermediate_texture1],
            input_texture_format,
        };

        // make sure the order is correct
        res.add_effect(PostEffect::Kuwahara(Kuwahara::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        )));

        res.add_effect(PostEffect::Bloom(Bloom::new2(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
            width,
            height,
        )));

        res.add_effect(PostEffect::ChromaticAberration(ChromaticAberration::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        )));

        res.add_effect(PostEffect::GrayScale(GrayScale::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        )));

        res.add_effect(PostEffect::Posterize(Posterize::new(
            device,
            queue,
            input_texture_format,
            fullscreen_tri_vertex_shader,
        )));

        res
    }

    pub fn add_effect<T: Into<PostEffect>>(&mut self, effect: T) {
        // all effects start disabled
        self.effects.push((false, effect.into()));
    }

    pub fn run_post_processing_effects(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        // main render
        input_texture: &wgpu::Texture,
        // composite
        output_texture: &wgpu::Texture,
    ) {
        let effect_count = self
            .effects
            .iter()
            .filter(|(is_enabled, _)| *is_enabled)
            .count();

        // if no effect, copy from input (main render) to output (composite) directly
        if effect_count == 0 {
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
        for (effect_index, (_, effect)) in self
            .effects
            .iter()
            .filter(|(is_enabled, _)| *is_enabled)
            .enumerate()
        {
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
                PostEffect::Kuwahara(x) => {
                    x.execute(
                        device,
                        encoder,
                        current_input_texture,
                        current_intermediate_output_texture,
                    );
                }
                PostEffect::ChromaticAberration(x) => {
                    x.execute(
                        device,
                        encoder,
                        current_input_texture,
                        current_intermediate_output_texture,
                    );
                }
                PostEffect::Posterize(x) => {
                    x.execute(
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

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let [new0, new1] =
            Self::create_intermediate_textures(device, width, height, self.input_texture_format);

        self.effects.iter_mut().for_each(|(_, fx)| match fx {
            PostEffect::Bloom(bloom) => {
                bloom.resize(device, width, height);
            }
            _ => {}
        });

        self.intermediate_textures = [new0, new1];
    }
}

impl PostProcessing {
    pub fn get_grayscale_toggle(&mut self) -> Option<&mut bool> {
        self.effects
            .iter_mut()
            .find(|(_, e)| matches!(e, PostEffect::GrayScale(_)))
            .map(|(is_enabled, _)| is_enabled)
    }

    pub fn get_bloom_toggle(&mut self) -> Option<&mut bool> {
        self.effects
            .iter_mut()
            .find(|(_, e)| matches!(e, PostEffect::Bloom(_)))
            .map(|(is_enabled, _)| is_enabled)
    }

    pub fn get_chromatic_aberration_toggle(&mut self) -> Option<&mut bool> {
        self.effects
            .iter_mut()
            .find(|(_, e)| matches!(e, PostEffect::ChromaticAberration(_)))
            .map(|(is_enabled, _)| is_enabled)
    }

    pub fn get_kuwahara_toggle(&mut self) -> Option<&mut bool> {
        self.effects
            .iter_mut()
            .find(|(_, e)| matches!(e, PostEffect::Kuwahara(_)))
            .map(|(is_enabled, _)| is_enabled)
    }

    pub fn get_posterize_toggle(&mut self) -> Option<&mut bool> {
        self.effects
            .iter_mut()
            .find(|(_, e)| matches!(e, PostEffect::Posterize(_)))
            .map(|(is_enabled, e)| {
                let PostEffect::Posterize(x) = e else {
                    unreachable!()
                };

                // update color every time this is called
                x.update_color_buffer();

                is_enabled
            })
    }
}
