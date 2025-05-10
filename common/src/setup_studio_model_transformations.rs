use std::array::from_fn;

use cgmath::{One, Rotation, Rotation3, Zero};
use mdl::{BlendBone, Bone, Mdl};

/// `[[[[(position, rotation); bone count]; frame count]; blend count]; sequence count]`
// first vec is sequences
pub type MdlPosRot = Vec<
    // blends
    Vec<
        // frames
        // the order is swapped when we compare it to the studiomdl implementatino
        // this seems like a better order when we make the data
        // just remember that we go frame before bone
        Vec<
            // bones
            Vec<PosRot>,
        >,
    >,
>;

pub type PosRot = (
    // position
    cgmath::Vector3<f32>,
    // rotation
    cgmath::Quaternion<f32>,
);

pub fn setup_studio_model_transformations(mdl: &Mdl) -> MdlPosRot {
    let bone_order = get_traversal_order(mdl);

    mdl.sequences
        .iter()
        .map(|sequence| {
            sequence
                .anim_blends
                .iter()
                .map(|blend| {
                    let frame_count = sequence.header.num_frames as usize;

                    // iterate over frame and then iterate over bone
                    // moving to a different frame doesn't change the numbers
                    // but the numbers within a frame are used together
                    // eg, bone hierarchy
                    (0..frame_count)
                        .map(|frame_idx| {
                            // caching the result
                            // based on the bone count
                            let mut transforms = vec![
                                (
                                    cgmath::Vector3::<f32>::zero(),
                                    cgmath::Quaternion::<f32>::one()
                                );
                                bone_order.len()
                            ];

                            bone_order.iter().for_each(|&bone_idx| {
                                let blend_bone = &blend[bone_idx];
                                let bone = &mdl.bones[bone_idx];

                                let (local_pos, local_rot) =
                                    compute_local_transformation(bone, blend_bone, frame_idx);

                                let (parent_pos, parent_rot) = if bone.parent == -1 {
                                    (cgmath::Vector3::zero(), cgmath::Quaternion::one())
                                } else {
                                    transforms[bone.parent as usize]
                                };

                                // compute hierarchy transformation
                                let rotated_local_pos =
                                    parent_rot.rotate_vector(cgmath::Vector3::from(local_pos));

                                let accum_pos = parent_pos + rotated_local_pos;
                                let accum_rot = parent_rot * local_rot;

                                transforms[bone_idx] = (accum_pos, accum_rot);
                            });

                            transforms
                        })
                        .collect::<Vec<Vec<PosRot>>>()
                })
                .collect()
        })
        .collect()
}

// visiting parents and then its children so that we can nicely cache parent's result to avoid duplicated calculations
fn get_traversal_order(mdl: &Mdl) -> Vec<usize> {
    let mut order = Vec::with_capacity(mdl.bones.len());
    let mut visited = vec![false; mdl.bones.len()];

    // need to be a function to be recursive
    fn visit(bone_idx: usize, mdl: &Mdl, order: &mut Vec<usize>, visited: &mut Vec<bool>) {
        if visited[bone_idx] {
            return;
        }

        let parent = mdl.bones[bone_idx].parent;

        // if has parent and parent is not visited
        if parent != -1 && !visited[parent as usize] {
            // then visit parent
            visit(parent as usize, mdl, order, visited);
        }

        // add current bone to the order and then mark bone visited
        order.push(bone_idx as usize);
        visited[bone_idx] = true;
    }

    for bone_idx in 0..mdl.bones.len() {
        visit(bone_idx, mdl, &mut order, &mut visited);
    }

    order
}

fn compute_local_transformation(bone: &Bone, blend_bone: &BlendBone, frame_idx: usize) -> PosRot {
    let pos: [f32; 3] = from_fn(|i| {
        blend_bone[i] // motion type
                    [frame_idx] // frame animation
            as f32 // casting
                * bone.scale[i] // scale factor
                + bone.value[i] // bone default value
    });

    let angles: [f32; 3] =
        from_fn(|i| blend_bone[i + 3][frame_idx] as f32 * bone.scale[i + 3] + bone.value[i + 3]);

    let rot = cgmath::Quaternion::from_angle_z(cgmath::Rad(angles[2]))
        * cgmath::Quaternion::from_angle_y(cgmath::Rad(angles[1]))
        * cgmath::Quaternion::from_angle_x(cgmath::Rad(angles[0]));

    (pos.into(), rot)
}

pub fn model_to_world_transformation(
    (model_pos, model_rot): PosRot,
    world_pos: cgmath::Vector3<f32>,
    world_rot: cgmath::Quaternion<f32>,
) -> PosRot {
    let new_rot = world_rot * model_rot;

    let entity_world_rotated_origin = world_rot.rotate_vector(model_pos);

    let new_pos = world_pos + entity_world_rotated_origin;

    (new_pos, new_rot)
}
