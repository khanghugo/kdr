use std::{array::from_fn, collections::HashSet};

use cgmath::{One, Rotation, Rotation3, VectorSpace, Zero};
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

pub struct WorldTransformationSkeletal {
    pub current_sequence_index: usize,
    // storing base world transformation
    pub world_transformation: PosRot,
    // storing model transformation on top of that
    pub model_transformations: MdlPosRot,
    // data related to each model transformation
    pub model_transformation_infos: Vec<ModelTransformationInfo>,
}

impl WorldTransformationSkeletal {
    pub fn build_mvp_with_gait_sequence(
        &self,
        time: f32,
        gaitsequence: usize,
    ) -> Vec<cgmath::Matrix4<f32>> {
        // TODO: reset the sequence on timeline scrub
        let current_sequence_index =
            (self.current_sequence_index).min(self.model_transformations.len() - 1);

        let current_sequence = &self.model_transformations[current_sequence_index];
        let current_sequence_info = &self.model_transformation_infos[current_sequence_index];

        let current_gait = &self.model_transformations[gaitsequence];
        let current_gait_info = &self.model_transformation_infos[gaitsequence];

        // TODO blending
        let current_anim_blend = &current_sequence[0];
        let current_gait_blend = &current_gait[0];

        let anim_frame_count = current_anim_blend.len();
        let gait_frame_count = current_gait_blend.len();

        let anim_total_time = anim_frame_count as f32 / current_sequence_info.frame_per_second;
        let gait_total_time = gait_frame_count as f32 / current_gait_info.frame_per_second;

        let anim_time = if current_sequence_info.looping {
            time % anim_total_time
        } else {
            time
        };
        let gait_time = if true { time % gait_total_time } else { time };

        let anim_frame_from_idx = ((anim_time * current_sequence_info.frame_per_second as f32)
            .floor() as usize)
            .min(anim_frame_count - 1);
        let gait_frame_from_idx = ((gait_time * current_gait_info.frame_per_second as f32).floor()
            as usize)
            .min(gait_frame_count - 1);

        // usually, the first condition will never hit, but whatever
        // gaits usually have more frames than our normal sequence
        let anim_from_frame = &current_anim_blend[anim_frame_from_idx];
        let anim_to_frame =
            &current_anim_blend[(anim_frame_from_idx + 1).min(anim_frame_count - 1)];

        let gait_from_frame = &current_gait_blend[gait_frame_from_idx];
        let gait_to_frame =
            &current_gait_blend[(gait_frame_from_idx + 1).min(gait_frame_count - 1)];

        let anim_target = (anim_time * current_sequence_info.frame_per_second).fract();
        let gait_target = (gait_time * current_gait_info.frame_per_second).fract();

        if gaitsequence == 0 {
            anim_from_frame
                .iter()
                .zip(anim_to_frame.iter())
                .map(|((from_pos, from_rot), (to_pos, to_rot))| {
                    let lerped_posrot = (
                        from_pos.lerp(*to_pos, anim_target),
                        from_rot.nlerp(*to_rot, anim_target),
                    );

                    let (pos, rot) = model_to_world_transformation(
                        lerped_posrot,
                        self.world_transformation.0,
                        self.world_transformation.1,
                    );

                    build_mvp_from_pos_and_rot(pos, rot)
                })
                .collect()
        } else {
            let gait_bones: HashSet<usize> = (40..56).into_iter().chain(0..=1).collect();

            (0..anim_from_frame.len())
                .map(|bone_idx| {
                    if gait_bones.contains(&bone_idx) {
                        let ((from_pos, from_rot), (to_pos, to_rot)) =
                            (&gait_from_frame[bone_idx], &gait_to_frame[bone_idx]);

                        let lerped_posrot = (
                            from_pos.lerp(*to_pos, gait_target),
                            from_rot.nlerp(*to_rot, gait_target),
                        );

                        model_to_world_transformation(
                            lerped_posrot,
                            self.world_transformation.0,
                            self.world_transformation.1,
                        )
                    } else {
                        let ((from_pos, from_rot), (to_pos, to_rot)) =
                            (&anim_from_frame[bone_idx], &anim_to_frame[bone_idx]);

                        let lerped_posrot = (
                            from_pos.lerp(*to_pos, anim_target),
                            from_rot.nlerp(*to_rot, anim_target),
                        );

                        model_to_world_transformation(
                            lerped_posrot,
                            self.world_transformation.0,
                            self.world_transformation.1,
                        )
                    }
                })
                .map(|(pos, rot)| build_mvp_from_pos_and_rot(pos, rot))
                .collect()
        }
    }

    pub fn build_mvp(&self, time: f32) -> Vec<cgmath::Matrix4<f32>> {
        self.build_mvp_with_gait_sequence(time, 0)
    }
}

pub type WorldTransformationEntity = PosRot;

pub enum BuildMvpResult {
    Entity(cgmath::Matrix4<f32>),
    Skeletal(Vec<cgmath::Matrix4<f32>>),
}

pub fn build_mvp_from_pos_and_rot(
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
) -> cgmath::Matrix4<f32> {
    let rotation: cgmath::Matrix4<f32> = rotation.into();

    cgmath::Matrix4::from_translation(position.into()) * rotation
}

pub enum WorldTransformation {
    /// For entity brushes, they only have one transformation, so that is good.
    Entity(PosRot),
    /// For skeletal system, multiple transformations means there are multiple bones.
    ///
    /// So, we store all bones transformation and then put it back in shader when possible.
    ///
    /// And we also store all information related to the model. Basically a lite mdl format
    Skeletal(WorldTransformationSkeletal),
}

impl WorldTransformation {
    pub fn worldspawn() -> Self {
        Self::Entity(origin_posrot())
    }

    pub fn get_entity(&self) -> &WorldTransformationEntity {
        match self {
            WorldTransformation::Entity(x) => x,
            WorldTransformation::Skeletal(_) => unreachable!(),
        }
    }

    pub fn get_skeletal_mut(&mut self) -> &mut WorldTransformationSkeletal {
        match self {
            WorldTransformation::Entity(_) => unreachable!(),
            WorldTransformation::Skeletal(x) => x,
        }
    }

    pub fn build_mvp(&self, time: f32) -> BuildMvpResult {
        match &self {
            Self::Entity((position, rotation)) => {
                BuildMvpResult::Entity(build_mvp_from_pos_and_rot(*position, *rotation))
            }
            Self::Skeletal(x) => BuildMvpResult::Skeletal(x.build_mvp(time)),
        }
    }
}

pub struct ModelTransformationInfo {
    pub frame_per_second: f32,
    pub looping: bool,
}

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
                                    origin_posrot()
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
    // welp, if the world rot is 0, which is intentional, then no model rendered
    if world_rot == cgmath::Quaternion::zero() {
        return (cgmath::Vector3::zero(), cgmath::Quaternion::zero());
    }

    let new_rot = world_rot * model_rot;

    let entity_world_rotated_origin = world_rot.rotate_vector(model_pos);

    let new_pos = world_pos + entity_world_rotated_origin;

    (new_pos, new_rot)
}

pub fn origin_posrot() -> PosRot {
    (cgmath::Vector3::zero(), cgmath::Quaternion::one())
}
