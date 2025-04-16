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

// all assuming that we only have 1 bone
pub fn get_idle_sequence_origin_angles(mdl: &mdl::Mdl) -> ([f32; 3], MdlAngles) {
    let sequence0 = &mdl.sequences[0];
    let blend0 = &sequence0.anim_blends[0];
    let bone_blend0 = &blend0[0];
    let bone0 = &mdl.bones[0];

    let origin: [f32; 3] = from_fn(|i| {
        bone_blend0[i] // motion type
                    [0] // frame 0
            as f32 // casting
                * bone0.scale[i] // scale factor
                + bone0.value[i] // bone default value
    });

    // ~~apparently origin doesnt matter~~
    // UPDATE: it does matter
    // let origin = [0f32; 3];

    let angles: [f32; 3] =
        from_fn(|i| bone_blend0[i + 3][0] as f32 * bone0.scale[i + 3] + bone0.value[i + 3]);

    (origin, MdlAngles(angles))
}
