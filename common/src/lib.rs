use nom::{IResult as _IResult, combinator::fail};

pub type IResult<'a, T> = _IResult<&'a str, T>;

mod constants;
mod setup_studio_model_transformations;

pub use constants::*;
pub use setup_studio_model_transformations::*;

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

/// "The Half-Life engine uses a left handed coordinate system, where X is forward, Y is left and Z is up."
pub struct MdlAngles(pub [f32; 3]);

impl MdlAngles {
    /// XYZ -> XYZ
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[0], angles[1], angles[2]]
    }
}

/// Y-ZX
pub struct BspAngles(pub [f32; 3]);

impl BspAngles {
    /// Y-ZX -> XYZ
    ///
    /// But pitch in this game is flipped since Doom.
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[2], -angles[0], angles[1]]
    }
}

pub fn vec3(i: &str) -> Option<[f32; 3]> {
    let res: Vec<f32> = i
        .split_whitespace()
        .filter_map(|n| n.parse::<f32>().ok())
        .collect();

    if res.len() < 3 {
        return None;
    }

    Some([res[0], res[1], res[2]])
}

/// Difference between curr and next
pub fn angle_diff(curr: f32, next: f32) -> f32 {
    let curr = curr.to_radians();
    let next = next.to_radians();

    (-(curr - next).sin()).asin().to_degrees()
}

use glam::FloatExt;

pub fn lerp_viewangles(from: [f32; 3], to: [f32; 3], target: f32) -> [f32; 3] {
    let viewangles_diff: [f32; 3] = from_fn(|i| {
        angle_diff(
            // normalize is not what we want as we are in between +/-
            from[i], to[i],
        )
    });

    let actual_to: [f32; 3] = from_fn(|i| from[i] + viewangles_diff[i]);

    lerp_arr3(from, actual_to, target)
}

pub fn lerp_arr3(from: [f32; 3], to: [f32; 3], target: f32) -> [f32; 3] {
    from_fn(|i| from[i].lerp(to[i], target))
}
