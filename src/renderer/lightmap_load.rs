use std::collections::HashMap;

use super::RenderContext;

pub struct LightMapAtlasAllocation {
    x_offset: f32,
    y_offset: f32,
    x_scale: f32,
    y_scale: f32,
}

pub struct LightMapAtlasBuffer {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    allocations: HashMap<usize, LightMapAtlasAllocation>
}

struct LightmapDimension {
    width: i32,
    height: i32,
    min_u: i32,
    min_v: i32,
}

impl RenderContext {
    pub fn load_lightmap(&self, bsp: &bsp::Bsp) {
        // let's do 4K
        const DIMENSION: i32 = 4096;

        let mut atlas = guillotiere::AtlasAllocator::new(guillotiere::size2(DIMENSION, DIMENSION));
        let mut allocations = HashMap::new();

        bsp.faces.iter().enumerate().for_each(|(idx, face)| {
            // let 
        });
    }
}

fn get_lightmap_dimensions(uvs: &[[f32; 2]]) -> LightmapDimension {
    let mut min_u = uvs[0][0].floor() as i32;
    let mut min_v = uvs[0][1].floor() as i32;
    let mut max_u = uvs[0][0].floor() as i32;
    let mut max_v = uvs[0][1].floor() as i32;

    for i in 1..uvs.len() {
        let u = uvs[i][0].floor() as i32;
        let v = uvs[i][1].floor() as i32;

        if u < min_u {
            min_u = u;
        }
        if v < min_v {
            min_v = v;
        }
        if u > max_u {
            max_u = u;
        }
        if v > max_v {
            max_v = v;
        }
    }

    return LightmapDimension {
        width: ((max_u as f32 / 16.0).ceil() as i32) - ((min_u as f32 / 16.0).floor() as i32) + 1,
        height: ((max_v as f32 / 16.0).ceil() as i32) - ((min_v as f32 / 16.0).floor() as i32) + 1,
        min_u,
        min_v,
    };
}
