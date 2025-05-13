use std::collections::HashMap;

use image::{Rgba, RgbaImage};
use tracing::warn;
use wad::types::Wad;

fn most_repeating_number<T>(a: &[T]) -> T
where
    T: std::hash::Hash + Eq + Copy,
{
    let mut h: HashMap<T, u32> = HashMap::new();
    for x in a {
        *h.entry(*x).or_insert(0) += 1;
    }
    let mut r: Option<T> = None;
    let mut m: u32 = 0;
    for (x, y) in h.iter() {
        if *y > m {
            m = *y;
            r = Some(*x);
        }
    }
    r.unwrap()
}

const VERY_BLUE: [u8; 3] = [0, 0, 255];

/// This does some tricks to render masked texture, read the code
pub fn eightbpp_to_rgba8(
    img: &[u8],
    palette: &[[u8; 3]],
    width: u32,
    height: u32,
    override_alpha: Option<u8>,
) -> RgbaImage {
    // very dumb hack, but what can i do
    // the alternative way i can think of is to do two textures, 1 for index, 1 for palette
    // but with that, it will be very hard to do simple thing such as texture filtering
    let is_probably_masked_image = most_repeating_number(img) == 255;

    RgbaImage::from_raw(
        width,
        height,
        img.iter()
            .flat_map(|&idx| {
                let color = palette[idx as usize];

                // due to how we do our data, we don't know how to render entities
                // we only know the texture at this stage
                // that means, we cannot assume that the texture is supposed to be alpha tested
                // so here, we will go against our idea and assume it anyway
                // maybe in the future, we might need to add more colors
                let is_blue = color == VERY_BLUE;

                if idx == 255 && (is_probably_masked_image || is_blue) {
                    [0, 0, 0, 0]
                } else {
                    [color[0], color[1], color[2], override_alpha.unwrap_or(255)]
                }
            })
            .collect(),
    )
    .expect("cannot create rgba8 from 8pp")
}

pub struct FullScrenTriVertexShader {
    pub shader_module: wgpu::ShaderModule,
}

impl FullScrenTriVertexShader {
    pub fn entry_point() -> &'static str {
        "vs_main"
    }

    pub fn create_shader_module(device: &wgpu::Device) -> Self {
        Self {
            shader_module: device
                .create_shader_module(wgpu::include_wgsl!("./shader/fullscreen_tri.wgsl")),
        }
    }

    pub fn vertex_state(&self) -> wgpu::VertexState {
        wgpu::VertexState {
            module: &self.shader_module,
            entry_point: Self::entry_point().into(),
            compilation_options: Default::default(),
            buffers: &[],
        }
    }
}

// vibe code
// Helper function to create a magenta/black checkerboard image
pub fn create_missing_texture_placeholder(width: u32, height: u32) -> RgbaImage {
    // You can adjust the checker_size for different pattern granularity
    let checker_size = 16; // 16x16 pixels per checker square
    let magenta = Rgba([255, 0, 255, 255]); // RGBA for magenta
    let black = Rgba([0, 0, 0, 255]); // RGBA for black

    let mut img = RgbaImage::new(width, height);

    for y in 0..height {
        for x in 0..width {
            // Determine the color based on the checkerboard pattern
            let color = if (x / checker_size + y / checker_size) % 2 == 0 {
                magenta
            } else {
                black
            };
            img.put_pixel(x, y, color);
        }
    }

    img
}

pub fn face_vertices(face: &bsp::Face, bsp: &bsp::Bsp) -> Vec<bsp::Vec3> {
    let mut face_vertices = vec![];

    for edge_idx in (face.first_edge as u32)..(face.first_edge as u32 + face.edge_count as u32) {
        let surf_edge = bsp.surf_edges[edge_idx as usize];

        let [v1_idx, v2_idx] = bsp.edges[surf_edge.abs() as usize];

        if surf_edge.is_positive() {
            face_vertices.push(bsp.vertices[v1_idx as usize]);
        } else {
            face_vertices.push(bsp.vertices[v2_idx as usize]);
        }
    }

    face_vertices
}

pub fn vertex_uv(pos: &bsp::Vec3, texinfo: &bsp::TexInfo) -> [f32; 2] {
    [
        (pos.dot(texinfo.u) + texinfo.u_offset),
        (pos.dot(texinfo.v) + texinfo.v_offset),
    ]
}
