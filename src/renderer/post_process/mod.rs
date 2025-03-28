pub trait PostProcessingModule {
    fn execute(&self, render_pass: &mut wgpu::RenderPass, render_target: &wgpu::TextureView);
}

pub struct PostProcesing<T>
where
    T: PostProcessingModule,
{
    modules: Vec<T>,
}

impl<T> PostProcesing<T>
where
    T: PostProcessingModule,
{
    pub fn create_pipelines(
        device: &wgpu::Device,
        swapchain_format: wgpu::TextureFormat,
        render_format: wgpu::TextureFormat,
    ) -> Self {
        todo!()
    }

    pub fn execute(
        &self,
        render_pass: &mut wgpu::RenderPass,
        render_target: &wgpu::TextureView,
        swapchain_target: &wgpu::TextureView,
    ) {
        self.modules
            .iter()
            .for_each(|module| module.execute(render_pass, render_target));
    }
}
