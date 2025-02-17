use std::collections::{HashMap, VecDeque};

use image::RgbaImage;

use super::lightmap_load::LightMapAtlasBuffer;

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

pub fn face_to_tri_strip(face: &[bsp::Vec3]) -> Vec<bsp::Vec3> {
    let mut dequeue: VecDeque<bsp::Vec3> = VecDeque::from_iter(face.to_owned().into_iter());

    let mut front = true;
    let mut strip = vec![];

    strip.push(dequeue.pop_front().unwrap());
    strip.push(dequeue.pop_front().unwrap());

    while !dequeue.is_empty() {
        if front {
            strip.push(dequeue.pop_back().unwrap());
        } else {
            strip.push(dequeue.pop_front().unwrap());
        }

        front = !front
    }

    strip
}

pub fn face_to_tri_strip2(face: &[bsp::Vec3]) -> Vec<bsp::Vec3> {
    let mut res = vec![];

    res.push(face[0]);
    res.push(face[1]);

    let mut left = 2;
    let mut right = face.len() - 1;

    while left <= right {
        res.push(face[right]);

        right -= 1;

        if left <= right {
            res.push(face[left]);

            left += 1;
        }
    }

    res
}

pub fn convex_polygon_to_triangle_strip_indices(polygon_vertices: &[bsp::Vec3]) -> Vec<u32> {
    let mut triangle_strip_indices: Vec<u32> = Vec::new();
    let num_vertices = polygon_vertices.len();

    if num_vertices < 3 {
        return triangle_strip_indices; // Not enough vertices for a triangle
    }

    // First triangle: use the first three vertices
    triangle_strip_indices.push(0);
    triangle_strip_indices.push(1);
    triangle_strip_indices.push(2);

    // Extend the strip for the remaining vertices
    for i in 3..num_vertices {
        triangle_strip_indices.push((i - 1) as u32);
        triangle_strip_indices.push((i - 2) as u32);
        triangle_strip_indices.push(i as u32);
    }

    triangle_strip_indices
}

// deepseek wrote this
pub fn triangulate_convex_polygon(vertices: &[bsp::Vec3]) -> Vec<u32> {
    // For convex polygons, we can simply fan-triangulate from the first vertex
    // This creates triangle indices: 0-1-2, 0-2-3, 0-3-4, etc.
    assert!(vertices.len() >= 3, "Polygon needs at least 3 vertices");

    let mut indices = Vec::with_capacity((vertices.len() - 2) * 3);
    for i in 1..vertices.len() - 1 {
        indices.push(0);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    indices
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

pub fn process_face(
    face: &bsp::Face,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
    face_idx: usize,
) -> (Vec<f32>, Vec<u32>) {
    let face_vertices = face_vertices(face, bsp);

    let indices = triangulate_convex_polygon(&face_vertices);

    // very inefficient right now
    // becuase all vertices here have the same normal
    let normal = bsp.planes[face.plane as usize].normal;
    let texinfo = &bsp.texinfo[face.texinfo as usize];

    // uv
    let miptex = &bsp.textures[texinfo.texture_index as usize];

    let vertices_texcoords: Vec<[f32; 2]> = face_vertices
        .iter()
        .map(|pos| vertex_uv(pos, &texinfo))
        .collect();

    let vertices_normalized_texcoords: Vec<[f32; 2]> = vertices_texcoords
        .iter()
        .map(|uv| [uv[0] / miptex.width as f32, uv[1] / miptex.height as f32])
        .collect();

    // lightmap
    let what = lightmap.allocations.get(&face_idx);

    let (face_width, face_height) = get_face_uv_dimensions(&vertices_texcoords);

    // in here, we take the un-normalized coordinate then we normalize it on 0-1 so that number can sample the lightmap
    let lightmap_texcoords: Vec<[f32; 2]> = if let Some(allocation) = what {
        let lightmap_texcoords = vertices_texcoords.iter().map(|&[u, v, ..]| {
            [
                allocation.atlas_x
                    + (u - allocation.min_x) / face_width as f32 * allocation.atlas_width,
                allocation.atlas_y
                    + (v - allocation.min_y) / face_height as f32 * allocation.atlas_height,
            ]
        });

        lightmap_texcoords.collect()
    } else {
        vertices_normalized_texcoords
            .iter()
            .map(|_| [0., 0.])
            .collect()
    };

    // collect to buffer
    let interleaved: Vec<f32> = face_vertices
        .into_iter()
        .zip(vertices_normalized_texcoords.into_iter())
        .zip(lightmap_texcoords.into_iter())
        .flat_map(|((pos, texcoord), lightmap_coord)| {
            [
                // no need to flip any of the geometry
                // we will do that to the camera
                // -pos.x,
                // pos.z, // flip y and z because the game is z up
                // pos.y,
                pos.x,
                pos.y,
                pos.z,
                normal.x,
                normal.y,
                normal.z,
                texcoord[0],
                texcoord[1],
                lightmap_coord[0],
                lightmap_coord[1],
            ]
        })
        .collect();

    (interleaved, indices)
}

// the dimension of the face on texture coordinate
fn get_face_uv_dimensions(uvs: &[[f32; 2]]) -> (i32, i32) {
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

    return (max_u - min_u + 1, max_v - min_v + 1);
}

#[derive(Debug)]
pub struct LightmapDimension {
    pub width: i32,
    pub height: i32,
    pub min_u: i32,
    pub min_v: i32,
}

// minimum light map size is always 2x2
// https://github.com/rein4ce/hlbsp/blob/1546eaff4e350a2329bc2b67378f042b09f0a0b7/js/hlbsp.js#L499
pub fn get_lightmap_dimensions(uvs: &[[f32; 2]]) -> LightmapDimension {
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

    // light map dimension is basically the face dimensions divided by 16
    // because luxel is 1 per 16 texel
    return LightmapDimension {
        width: ((max_u as f32 / 16.0).ceil() as i32) - ((min_u as f32 / 16.0).floor() as i32) + 1,
        height: ((max_v as f32 / 16.0).ceil() as i32) - ((min_v as f32 / 16.0).floor() as i32) + 1,
        min_u,
        min_v,
    };
}
