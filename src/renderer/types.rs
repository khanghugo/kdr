// one buffer contains a polygon
pub struct BspFaceBuffer {
    // one face right now is one mesh
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: usize,
    pub texture_index: usize,
}

impl Drop for BspFaceBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

pub struct MeshBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_length: usize,

    // vector of indices, pointing to render object textures
    // for some convenient reasons, .obj will have 1 texture per mesh!!!
    pub material: Option<usize>,
}

impl Drop for MeshBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}
