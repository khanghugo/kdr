//! Dynamic buffer only concerns about models that aren't part of the world such as view models or player models.
//! aka things that are loaded upon load time and can be changed easily.
//!
//! The world is static in a sense that the models don't swap out. This is dynamic because it is exactly opposite of that.

use std::collections::HashMap;

use common::{
    MdlPosRot, ModelTransformationInfo, WorldTransformationSkeletal, origin_posrot,
    setup_studio_model_transformations,
};
use image::RgbaImage;
use mdl::{Mdl, SequenceFlag};

use crate::renderer::{
    mvp_buffer::MvpBuffer,
    texture_buffer::texture_array::{TextureArrayBuffer, create_texture_array},
};

use super::{
    WorldLoader, WorldVertex, WorldVertexBuffer,
    utils::{BatchLookup, create_world_vertex_buffer, get_mdl_textures, triangulate_mdl_triverts},
};

pub struct WorldDynamicBuffer {
    pub name: String,
    pub opaque: Vec<WorldVertexBuffer>,
    pub textures: Vec<TextureArrayBuffer>,
    pub mvp_buffer: MvpBuffer,
    pub transformations: WorldTransformationSkeletal,
}

type TextureTableLookup = HashMap<usize, (usize, usize)>;

// TODO somehow loads sprite
// TODO transparent model
impl WorldLoader {
    pub fn load_dynamic_world(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        mdl: &Mdl,
        submodel_index: usize,
    ) -> WorldDynamicBuffer {
        let mdl_textures = get_mdl_textures(mdl);

        let (texture_arrays, lookup_table) =
            Self::load_dynamic_world_textures(device, queue, mdl_textures);

        let mut batch_lookup = BatchLookup::new();

        // TODO some mdl transparency stuffs
        mdl.bodyparts.iter().for_each(|bodypart| {
            bodypart.models.get(submodel_index).map(|model| {
                model.meshes.iter().for_each(|mesh| {
                    let texture_idx = mesh.header.skin_ref as usize;
                    let texture = &mdl.textures[texture_idx];
                    let texture_flags = &texture.header.flags;
                    let (width, height) = texture.dimensions();

                    mesh.triangles.iter().for_each(|triangles| {
                        // it is possible for a mesh to have both fan and strip run
                        let (is_strip, triverts) = match triangles {
                            mdl::MeshTriangles::Strip(triverts) => (true, triverts),
                            mdl::MeshTriangles::Fan(triverts) => (false, triverts),
                        };

                        // now just convert triverts into mdl vertex data
                        // then do some clever stuff with index buffer to make it triangle list
                        let (array_idx, layer_idx) = lookup_table
                            .get(&texture_idx)
                            .expect("cannot get world dynamic buffer texture");
                        let batch = batch_lookup.entry(*array_idx).or_insert((vec![], vec![]));

                        let new_vertices_offset = batch.0.len();

                        // create vertex buffer here
                        let vertices = triverts.iter().map(|trivert| {
                            let [u, v] = [
                                trivert.header.s as f32 / width as f32,
                                trivert.header.t as f32 / height as f32,
                            ];

                            let bone_index = model.vertex_info[trivert.header.vert_index as usize];

                            WorldVertex {
                                pos: trivert.vertex.to_array(),
                                tex_coord: [u, v],
                                normal: trivert.normal.to_array(),
                                layer: *layer_idx as u32,
                                type_: 1,
                                data_a: [0f32; 3],
                                data_b: [texture_flags.bits() as u32, bone_index as u32, 0],
                            }
                        });

                        batch.0.extend(vertices);

                        let mut local_index_buffer: Vec<u32> = vec![];

                        // create index buffer here
                        // here we will create triangle list
                        triangulate_mdl_triverts(
                            &mut local_index_buffer,
                            triverts,
                            is_strip,
                            new_vertices_offset,
                        );

                        batch.1.extend(local_index_buffer);
                    });
                });
            });
        });

        let world_vertex_buffers = create_world_vertex_buffer(device, batch_lookup);

        let model_transformations = setup_studio_model_transformations(mdl);
        let model_transformation_infos: Vec<ModelTransformationInfo> = mdl
            .sequences
            .iter()
            .map(|sequence| ModelTransformationInfo {
                frame_per_second: sequence.header.fps,
                looping: sequence.header.flags.contains(SequenceFlag::LOOPING),
            })
            .collect();

        let skeletal_transformation = WorldTransformationSkeletal {
            current_sequence_index: 0,
            world_transformation: origin_posrot(),
            model_transformations,
            model_transformation_infos,
        };

        let initial_transformations = skeletal_transformation.build_mvp(0.);

        let mvp_buffer = MvpBuffer::create_mvp(device, queue, initial_transformations);

        WorldDynamicBuffer {
            name: name.to_string(),
            opaque: world_vertex_buffers,
            textures: texture_arrays,
            mvp_buffer: mvp_buffer,
            transformations: skeletal_transformation,
        }
    }

    // samey code as the static world but it is a lot simpler
    // the goal is to return an array of texture array and then texture look up
    // texture look up takes in the texture index and returns texture array and layer index
    fn load_dynamic_world_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mdl_textures: Vec<RgbaImage>,
    ) -> (Vec<TextureArrayBuffer>, TextureTableLookup) {
        // grouping all the textures in its own samey dimensions first
        let mut texture_arrays_lookup: HashMap<(u32, u32), Vec<usize>> = HashMap::new();

        mdl_textures
            .iter()
            .enumerate()
            .for_each(|(texture_idx, texture)| {
                texture_arrays_lookup
                    .entry(texture.dimensions())
                    .or_insert(vec![])
                    .push(texture_idx);
            });

        // convert the hash table into a normal vector (ordered)
        let texture_arrays_lookup: Vec<Vec<usize>> =
            texture_arrays_lookup.into_iter().map(|(_, x)| x).collect();

        // create a (texture index) -> (array index, layer index) look up
        let mut lookup_table: TextureTableLookup = TextureTableLookup::new();

        texture_arrays_lookup
            .iter()
            .enumerate()
            .for_each(|(array_idx, textures)| {
                textures
                    .iter()
                    .enumerate()
                    .for_each(|(layer_idx, texture)| {
                        lookup_table.insert(*texture, (array_idx, layer_idx));
                    });
            });

        // now create texture arrays
        let texture_arrays: Vec<TextureArrayBuffer> = texture_arrays_lookup
            .iter()
            .map(|textures| {
                let ref_vec: Vec<&RgbaImage> = textures
                    .iter()
                    .filter_map(|&texture_idx| mdl_textures.get(texture_idx))
                    .collect();

                create_texture_array(device, queue, &ref_vec)
                    .expect("cannot load dynamic world buffer texture")
            })
            .collect();

        (texture_arrays, lookup_table)
    }
}
