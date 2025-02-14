use std::collections::VecDeque;

use image::RgbaImage;

pub fn eightbpp_to_rgba8(img: &[u8], palette: &[[u8; 3]], width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_raw(
        width,
        height,
        img.iter()
            .flat_map(|&idx| {
                let color = palette[idx as usize];
                [color[0], color[1], color[2], 255]
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
