use std::{array::from_fn, collections::HashMap};

use image::RgbaImage;

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

pub fn get_bsp_textures(bsp: &bsp::Bsp) -> Vec<RgbaImage> {
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

            eightbpp_to_rgba8(
                texture.mip_images[0].data.get_bytes(),
                texture.palette.get_bytes(),
                texture.width,
                texture.height,
                override_alpha,
            )
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

pub fn build_mvp_from_origin_angles(
    origin: [f32; 3],
    angles: cgmath::Quaternion<f32>,
) -> cgmath::Matrix4<f32> {
    let rotation: cgmath::Matrix4<f32> = angles.into();

    cgmath::Matrix4::from_translation(origin.into()) * rotation
}

pub struct MdlAngles(pub [f32; 3]);

impl MdlAngles {
    // "The Half-Life engine uses a left handed coordinate system, where X is forward, Y is left and Z is up."
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[0], angles[1], angles[2]]
    }
}

pub struct BspAngles(pub [f32; 3]);

impl BspAngles {
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [-angles[0], angles[2], angles[1]]
    }
}

// all assuming that we only have 1 bone
pub fn get_idle_sequence_origin_angles(mdl: &mdl::Mdl) -> ([f32; 3], MdlAngles) {
    let sequence0 = &mdl.sequences[0];
    let blend0 = &sequence0.anim_blends[0];
    let bone_blend0 = &blend0[0];
    let bone0 = &mdl.bones[0];

    // let origin: [f32; 3] = from_fn(|i| {
    //     bone_blend0[i] // motion type
    //                 [0] // frame 0
    //         as f32 // casting
    //             * bone0.scale[i] // scale factor
    //             + bone0.value[i] // bone default value
    // });

    // apparently origin doesnt matter
    let origin = [0f32; 3];

    let angles: [f32; 3] =
        from_fn(|i| bone_blend0[i + 3][0] as f32 * bone0.scale[i + 3] + bone0.value[i + 3]);

    (origin, MdlAngles(angles))
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
