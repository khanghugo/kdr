use std::collections::{HashMap, VecDeque};

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
pub fn eightbpp_to_rgba8(img: &[u8], palette: &[[u8; 3]], width: u32, height: u32) -> RgbaImage {
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
                    [color[0], color[1], color[2], 255]
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

// written by deepseek
pub fn triangle_strip_to_triangle_list(strip_vertices: &[mdl::Trivert]) -> Vec<mdl::Trivert> {
    let mut triangles = Vec::new();
    for i in 0..strip_vertices.len().saturating_sub(2) {
        if i % 2 == 0 {
            triangles.extend_from_slice(&[
                strip_vertices[i],
                strip_vertices[i + 1],
                strip_vertices[i + 2],
            ]);
        } else {
            triangles.extend_from_slice(&[
                strip_vertices[i + 1],
                strip_vertices[i],
                strip_vertices[i + 2],
            ]);
        }
    }
    triangles
}
