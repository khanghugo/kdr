use std::{array::from_fn, collections::HashMap};

use image::RgbaImage;
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
                // if the image is masked and the index is 255
                // alpha is 0 and it is all black
                // this makes things easier
                if is_probably_masked_image && idx == 255 {
                    [0, 0, 0, 0]
                } else {
                    [color[0], color[1], color[2], override_alpha.unwrap_or(255)]
                }
            })
            .collect(),
    )
    .expect("cannot create rgba8 from 8pp")
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

pub fn get_bsp_textures(bsp: &bsp::Bsp, external_wads: &[Wad]) -> Vec<RgbaImage> {
    bsp.textures
        .iter()
        .map(|texture| {
            let texture_name = texture.texture_name.get_string_standard();
            let override_alpha = if texture_name == "SKY" {
                // 16.into()
                None
            } else {
                None
            };

            // offset 0 means it is using external wad
            if texture.mip_offsets[0] == 0 {
                external_wads
                    .iter()
                    .find_map(|wad| {
                        wad.entries.iter().find_map(|entry| {
                            let Some(external_texture) = entry.file_entry.get_mip_tex() else {
                                return None;
                            };

                            if external_texture.texture_name.get_string_standard() == texture_name {
                                return Some(eightbpp_to_rgba8(
                                    external_texture.mip_images[0].data.get_bytes(),
                                    external_texture.palette.get_bytes(),
                                    external_texture.width,
                                    external_texture.height,
                                    override_alpha,
                                ));
                            }

                            None
                        })
                    })
                    // TODO maybe do magenta black checker pattern
                    .unwrap_or_else(|| panic!("cannot find texture name `{texture_name}`"))
            } else {
                eightbpp_to_rgba8(
                    texture.mip_images[0].data.get_bytes(),
                    texture.palette.get_bytes(),
                    texture.width,
                    texture.height,
                    override_alpha,
                )
            }
        })
        .collect()
}

pub fn get_mdl_textures(mdl: &mdl::Mdl) -> Vec<RgbaImage> {
    mdl.textures
        .iter()
        .map(|texture| {
            eightbpp_to_rgba8(
                &texture.image,
                &texture.palette,
                texture.dimensions().0,
                texture.dimensions().1,
                None,
            )
        })
        .collect()
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

pub fn maybe_web_debug_print(s: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        crate::browser_console_log(s);
    }
}
