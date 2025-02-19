pub struct TextureBuffer {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

// impl drop will does some unholy shit
// impl Drop for TextureBuffer {
//     fn drop(&mut self) {
//         self.texture.destroy();
//     }
// }
