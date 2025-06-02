use image::RgbaImage;
use tracing::warn;
use wad::types::Wad;

use crate::renderer::{
    bsp_lightmap::LightMapAtlasBuffer,
    utils::{create_missing_texture_placeholder, eightbpp_to_rgba8, face_vertices, vertex_uv},
    world_buffer::WorldVertex,
};

use super::ProcessBspFaceData;

pub(super) fn process_bsp_face(
    face_data: ProcessBspFaceData,
    bsp: &bsp::Bsp,
    lightmap: &LightMapAtlasBuffer,
) -> (Vec<WorldVertex>, Vec<u32>) {
    let ProcessBspFaceData {
        bsp_face_index: face_index,
        face,
        custom_render,
        world_entity_index,
        texture_layer_index,
        type_,
    } = face_data;

    let face_vertices = face_vertices(face, bsp);

    let indices = triangulate_convex_polygon(&face_vertices);

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
    let what = lightmap.allocations.get(&face_index);

    // https://github.com/magcius/noclip.website/blob/66595465295720f8078a53d700988241b0adc2b0/src/GoldSrc/BSPFile.ts#L285
    let (face_min_u, _face_max_u, face_min_v, _face_max_v) = get_face_uv_box(&vertices_texcoords);

    let lightmap_texcoords: Vec<[f32; 2]> = if let Some(allocation) = what {
        let lightmap_texcoords = vertices_texcoords.iter().map(|&[u, v, ..]| {
            let lightmap_u =
                ((u / 16.0) - (face_min_u / 16.0).floor() + 0.5) / allocation.lightmap_width;
            let lightmap_v =
                ((v / 16.0) - (face_min_v / 16.0).floor() + 0.5) / allocation.lightmap_height;

            [
                allocation.atlas_x + lightmap_u * allocation.atlas_width,
                allocation.atlas_y + lightmap_v * allocation.atlas_height,
            ]
        });

        lightmap_texcoords.collect()
    } else {
        vertices_normalized_texcoords
            .iter()
            .map(|_| [0., 0.])
            .collect()
    };

    let rendermode = custom_render
        .as_ref()
        .map(|v| v.rendermode)
        // amount 1 = 255 = full aka opaque
        // transparent objects have their own default so this won't be a problem
        .unwrap_or(0);

    let renderamt = custom_render
        .as_ref()
        .map(|v| v.renderamt / 255.0)
        // amount 1 = 255 = full aka opaque
        // transparent objects have their own default so this won't be a problem
        .unwrap_or(1.0);

    // collect to buffer
    let vertices: Vec<WorldVertex> = face_vertices
        .into_iter()
        .zip(vertices_normalized_texcoords.into_iter())
        .zip(lightmap_texcoords.into_iter())
        .map(|((pos, texcoord), lightmap_coord)| WorldVertex {
            pos: pos.to_array().into(),
            tex_coord: texcoord.into(),
            normal: normal.to_array().into(),
            layer: texture_layer_index as u32,
            type_: 0,
            data_a: [lightmap_coord[0], lightmap_coord[1], renderamt],
            data_b: [rendermode as u32, type_, 0],
        })
        .collect();

    (vertices, indices)
}

// the dimension of the face on texture coordinate
pub(super) fn get_face_uv_box(uvs: &[[f32; 2]]) -> (f32, f32, f32, f32) {
    let mut min_u = uvs[0][0];
    let mut min_v = uvs[0][1];
    let mut max_u = uvs[0][0];
    let mut max_v = uvs[0][1];

    for i in 1..uvs.len() {
        let u = uvs[i][0];
        let v = uvs[i][1];

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

    return (min_u, max_u, min_v, max_v);
}

// deepseek wrote this
// input is a winding order polygon
// the output is the vertex index
// so there is no need to do anythign to the vertex buffer, only play around with the index buffer
pub(super) fn triangulate_convex_polygon(vertices: &[bsp::Vec3]) -> Vec<u32> {
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

pub(super) fn get_bsp_textures(bsp: &bsp::Bsp, external_wads: &[Wad]) -> Vec<RgbaImage> {
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
                    .unwrap_or_else(|| {
                        warn!("cannot find texture name `{texture_name}`");

                        create_missing_texture_placeholder(texture.width, texture.height)
                    })
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
