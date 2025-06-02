use mdl::Mdl;

use crate::renderer::world_buffer::utils::triangulate_mdl_triverts;

use super::{BatchLookup, WorldTextureLookupTable, WorldVertex};

pub(super) fn create_world_model_vertices(
    mdl: &Mdl,
    submodel: usize,
    world_entity_index: usize,
    world_texture_lookup: &WorldTextureLookupTable,
    batch_lookup: &mut BatchLookup,
    vertex_type: u32,
    assign_bone_idx: impl Fn(u8) -> u32,
) {
    mdl.bodyparts.iter().for_each(|bodypart| {
        bodypart.models.get(submodel).map(|model| {
            model.meshes.iter().for_each(|mesh| {
                // one mesh has the same texture everything
                let texture_idx = mesh.header.skin_ref as usize;
                let texture = &mdl.textures[texture_idx];
                let texture_flags = &texture.header.flags;
                let (width, height) = texture.dimensions();

                // let triangle_list = triangle_strip_to_triangle_list(&mesh.vertices);

                mesh.triangles.iter().for_each(|triangles| {
                    // it is possible for a mesh to have both fan and strip run
                    let (is_strip, triverts) = match triangles {
                        mdl::MeshTriangles::Strip(triverts) => (true, triverts),
                        mdl::MeshTriangles::Fan(triverts) => (false, triverts),
                    };

                    // now just convert triverts into mdl vertex data
                    // then do some clever stuff with index buffer to make it triangle list
                    let (array_idx, layer_idx) = world_texture_lookup
                        .get(&(world_entity_index, texture_idx))
                        .expect("cannot get world texture");
                    let batch = batch_lookup.entry(*array_idx).or_insert((vec![], vec![]));

                    let new_vertices_offset = batch.0.len();

                    // create vertex buffer here
                    let vertices = triverts.iter().map(|trivert| {
                        let [u, v] = [
                            trivert.header.s as f32 / width as f32,
                            trivert.header.t as f32 / height as f32,
                        ];

                        let bone_index = model.vertex_info[trivert.header.vert_index as usize];

                        // now this is some stuffs that seems dumb
                        // in skeletal mvp, we just don't use index 0
                        // with this, if bone index is 0, it means model uses entity mvp
                        // so, skeletal mvp 0 is empty
                        let buffer_bone_idx = assign_bone_idx(bone_index);

                        WorldVertex {
                            pos: trivert.vertex.to_array(),
                            tex_coord: [u, v],
                            normal: trivert.normal.to_array(),
                            layer: *layer_idx as u32,
                            type_: vertex_type,
                            data_a: [0f32; 3],
                            data_b: [texture_flags.bits() as u32, buffer_bone_idx as u32, 0],
                        }
                    });

                    batch.0.extend(vertices);

                    let mut index_buffer: Vec<u32> = vec![];

                    // create index buffer here
                    // here we will create triangle list
                    triangulate_mdl_triverts(
                        &mut index_buffer,
                        triverts,
                        is_strip,
                        new_vertices_offset,
                    );

                    batch.1.extend(index_buffer);
                });
            });
        });
    });
}
