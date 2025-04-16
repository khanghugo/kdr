use cgmath::{Rad, Rotation, Rotation3};
use nom::{IResult as _IResult, combinator::fail};

pub type IResult<'a, T> = _IResult<&'a str, T>;

mod constants;

pub use constants::*;

// https://github.com/getreu/parse-hyperlinks/blob/5af034d14aa72ffb9e705da13bf557a564b1bebf/parse-hyperlinks/src/lib.rs#L41
pub fn take_until_unbalanced(
    opening_bracket: char,
    closing_bracket: char,
) -> impl Fn(&str) -> IResult<&str> {
    move |i: &str| {
        let mut index = 0;
        let mut bracket_counter = 0;
        let mut ignore_bracket = false;
        while let Some(n) = &i[index..].find(&[opening_bracket, closing_bracket, '\\', '"'][..]) {
            index += n;
            let mut it = i[index..].chars();
            match it.next() {
                Some('\\') => {
                    // Skip the escape char `\`.
                    index += '\\'.len_utf8();
                    // Skip also the following char.
                    if let Some(c) = it.next() {
                        index += c.len_utf8();
                    }
                }
                // ignore bracket inside quotation mark
                Some('"') => {
                    ignore_bracket = !ignore_bracket;
                    index += '"'.len_utf8();
                }
                Some(c) if c == opening_bracket => {
                    if !ignore_bracket {
                        bracket_counter += 1;
                    }

                    // need to increment when matching, otherwise deadlock
                    index += opening_bracket.len_utf8();
                }
                Some(c) if c == closing_bracket => {
                    if !ignore_bracket {
                        bracket_counter -= 1;
                    }

                    index += closing_bracket.len_utf8();
                }
                // Can not happen.
                _ => unreachable!(),
            };
            // We found the unmatched closing bracket.
            if bracket_counter == -1 {
                // We do not consume it.
                index -= closing_bracket.len_utf8();
                return Ok((&i[index..], &i[0..index]));
            };
        }

        if bracket_counter == 0 {
            Ok(("", i))
        } else {
            Ok(fail(i)?)
        }
    }
}

use std::array::from_fn;

pub fn build_mvp_from_origin_angles(
    origin: [f32; 3],
    angles: cgmath::Quaternion<f32>,
) -> cgmath::Matrix4<f32> {
    let rotation: cgmath::Matrix4<f32> = angles.into();

    cgmath::Matrix4::from_translation(origin.into()) * rotation
}

/// "The Half-Life engine uses a left handed coordinate system, where X is forward, Y is left and Z is up."
pub struct MdlAngles(pub [f32; 3]);

impl MdlAngles {
    /// XYZ -> XYZ
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[0], angles[1], angles[2]]
    }
}

/// YZX
pub struct BspAngles(pub [f32; 3]);

impl BspAngles {
    /// YZX -> XYZ
    ///
    /// But pitch in this game is flipped since Doom.
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[2], -angles[0], angles[1]]
    }
}

pub fn get_bone_sequence_anim_origin_angles(
    mdl: &mdl::Mdl,
    bone_idx: usize,
    sequence_idx: usize,
    anim_idx: usize,
) -> ([f32; 3], MdlAngles) {
    let sequence_x = &mdl.sequences[sequence_idx];

    // only take blend0 now
    // TODO blending animations
    let blend_0 = &sequence_x.anim_blends[0];

    let bone_blend_0 = &blend_0[bone_idx];
    let bone_x = &mdl.bones[bone_idx];

    let origin: [f32; 3] = from_fn(|i| {
        bone_blend_0[i] // motion type
                    [anim_idx] // frame animation
            as f32 // casting
                * bone_x.scale[i] // scale factor
                + bone_x.value[i] // bone default value
    });

    let angles: [f32; 3] = from_fn(|i| {
        bone_blend_0[i + 3][anim_idx] as f32 * bone_x.scale[i + 3] + bone_x.value[i + 3]
    });

    let parent = bone_x.parent;

    // need to accumulate transformation from parents
    // if this is parent, just return
    if parent == -1 {
        return (origin, MdlAngles(angles));
    }

    // now, some recursion stuffs
    let (parent_origin, parent_angles) =
        get_bone_sequence_anim_origin_angles(mdl, parent as usize, sequence_idx, anim_idx);

    let child_rotation = cgmath::Quaternion::from_angle_z(cgmath::Rad(angles[2]))
        * cgmath::Quaternion::from_angle_y(cgmath::Rad(angles[1]))
        * cgmath::Quaternion::from_angle_x(cgmath::Rad(angles[0]));

    let parent_rotation = cgmath::Quaternion::from_angle_z(cgmath::Rad(parent_angles.0[2]))
        * cgmath::Quaternion::from_angle_y(cgmath::Rad(parent_angles.0[1]))
        * cgmath::Quaternion::from_angle_x(cgmath::Rad(parent_angles.0[0]));

    let rotated_child_origin = parent_rotation.rotate_vector(cgmath::Vector3::from(origin));

    let accum_rotation = parent_rotation * child_rotation;
    let angles_cast: cgmath::Euler<Rad<f32>> = accum_rotation.into();
    let accum_angles = [angles_cast.x.0, angles_cast.y.0, angles_cast.z.0];

    let accum_origin = [
        parent_origin[0] + rotated_child_origin.x,
        parent_origin[1] + rotated_child_origin.y,
        parent_origin[2] + rotated_child_origin.z,
    ];

    (accum_origin.into(), MdlAngles(accum_angles))
}
