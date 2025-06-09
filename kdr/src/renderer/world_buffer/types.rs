use bitflags::bitflags;
use bytemuck::{Pod, Zeroable};

#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct PushConstantRenderFlags(u32);

bitflags! {
    impl PushConstantRenderFlags: u32 {
        const RenderNoDraw      = (1 << 0);
        const FullBright        = (1 << 1);
    }
}

// TODO, bit packing. maybe that is better?
#[derive(Pod, Zeroable, Clone, Copy)]
#[repr(C)]
pub struct WorldPushConstants {
    pub render_flags: PushConstantRenderFlags,
    pub time: f32,
}

pub enum WorldVertexType {
    Bsp,
    Mdl,
    Sprite,
}

impl From<WorldVertexType> for u32 {
    fn from(value: WorldVertexType) -> Self {
        match value {
            WorldVertexType::Bsp => 0,
            WorldVertexType::Mdl => 1,
            WorldVertexType::Sprite => 2,
        }
    }
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
/// Common vertex data structure for both bsp and mdl
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub tex_coord: [f32; 2],
    pub normal: [f32; 3],
    // layer index to get the texture from texture array
    pub layer: u32,
    // type of the vertex, bsp vertex or mdl vertex
    // 0: bsp, 1: mdl, 2 is sprite
    pub type_: u32,
    // for bsp: [lightmap_u, lightmap_v, renderamt]
    // for mdl: unused
    // for sprite: [framerate, unused, renderamt]
    pub data_a: [f32; 3],
    // for bsp: [rendermode, mvp index, face type]
    //  face type meaning it is normal 0, sky 1, or nodraw 2
    // for mdl: [textureflag, mvp/bone index, unused]
    // for sprite: [rendermode, mvp index, frame count (u16) | orientation type (u16)]
    pub data_b: [u32; 3],
}

impl WorldVertex {
    pub fn buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // pos
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // texcoord
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                // normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 20,
                    shader_location: 2,
                },
                // packed layer model
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 32,
                    shader_location: 3,
                },
                // vertex type
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32,
                    offset: 36,
                    shader_location: 4,
                },
                // data_a
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 40,
                    shader_location: 5,
                },
                // data_b
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32x3,
                    offset: 52,
                    shader_location: 6,
                },
            ],
        }
    }
}

pub struct WorldVertexBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: usize,
    pub texture_array_index: usize,
}

impl Drop for WorldVertexBuffer {
    fn drop(&mut self) {
        self.vertex_buffer.destroy();
        self.index_buffer.destroy();
    }
}

pub enum WorldPipelineType {
    /// Standard Z Pre Pass
    ZPrepass,
    /// Masking SKY texture in the scene with depth value of 1.0 (farthest possible)
    ///
    /// With this, it is easier to draw skybox over it while also being able to occlude objects behind it.
    ///
    /// This means 3D skybox tricks don't work :()
    SkyboxMask,
    Opaque,
    Transparent,
}
