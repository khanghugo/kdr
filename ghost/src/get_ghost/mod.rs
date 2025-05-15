use std::{array::from_fn, path::Path};

use common::angle_diff;
use dem::types::Demo;
use glam::{FloatExt, Vec3};

pub(crate) mod demo;
pub(crate) mod romanian_jumpers;
pub(crate) mod simen;
pub(crate) mod sourceruns_hlkz;
pub(crate) mod surf_gateway;

#[derive(Debug, Clone)]
pub struct GhostFrameSound {
    pub file_name: String,
    pub channel: i32,
    pub volume: f32,
    pub origin: Option<[f32; 3]>,
}

#[derive(Debug, Clone)]
pub struct GhostFrameEntityText {
    pub text: String,
    // normalized [0, 1]
    // demo default location goes [-8192, 8192]
    pub location: [f32; 2],
    // normalized rgba [0, 1]
    pub color: [f32; 4],
    // how long it lives for, in seconds
    // demo life is counted in miliseconds, so "190" = 0.19s
    pub life: f32,
    // the channel of the text where only 1 text can occupy
    // with this, we can render text more accurately like how the game does
    pub channel: i8,
}

// need to process text at the get_ghost level, not the top level
#[derive(Debug, Clone)]
pub struct GhostFrameSayText {
    /// (Color index, Text)
    ///
    /// Color index is from HLSDK
    ///
    /// ```c++
    /// float g_ColorBlue[3]	= { 0.6, 0.8, 1.0 };
    /// float g_ColorRed[3]		= { 1.0, 0.25, 0.25 };
    /// float g_ColorGreen[3]	= { 0.6, 1.0, 0.6 };
    /// float g_ColorYellow[3]	= { 1.0, 0.7, 0.0 };
    /// float g_ColorGrey[3]	= { 0.8, 0.8, 0.8 };
    ///
    /// float *GetClientColor( int clientIndex )
    /// {
    ///     switch ( g_PlayerExtraInfo[clientIndex].teamnumber )
    ///     {
    ///         case 1:	return g_ColorBlue;
    ///         case 2: return g_ColorRed;
    ///         case 3: return g_ColorYellow;
    ///         case 4: return g_ColorGreen;
    ///         case 0: return g_ColorYellow;
    ///             default	: return g_ColorGrey;
    ///     }
    ///     return NULL;
    /// }
    /// ```
    pub text: Vec<(u8, String)>,
}

#[derive(Debug, Clone)]
pub struct GhostFrameExtra {
    pub sound: Vec<GhostFrameSound>,
    pub entity_text: Vec<GhostFrameEntityText>,
    pub anim: Option<GhostFrameAnim>,
    pub say_text: Vec<GhostFrameSayText>,
    /// The string is the simplified name of it, eg "v_usp" or "weapon_usp" is all reduced to "usp".
    ///
    /// With that, it is easier to check for weapon.
    pub weapon_change: Option<String>,
    pub weapon_sequence: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct GhostFrame {
    pub origin: Vec3,
    pub viewangles: Vec3,
    pub frametime: Option<f32>,
    pub buttons: Option<u32>,
    pub fov: Option<f32>,
    pub extras: Option<GhostFrameExtra>,
}

#[derive(Debug, Clone)]
pub struct GhostFrameAnim {
    pub sequence: Option<i32>,
    pub frame: Option<f32>,
    pub animtime: Option<f32>,
    pub gaitsequence: Option<i32>,
    // 0 is the same as no blending so there is no need to do optional type.
    pub blending: [u8; 2],
}

#[derive(Debug)]
pub struct GhostInfo {
    pub ghost_name: String,
    pub map_name: String,
    pub game_mod: String,
    pub frames: Vec<GhostFrame>,
}

impl GhostInfo {
    /// Returns an interpolated [`GhostFrame`] based on current time and the round down frame index.
    ///
    /// Takes an optional argument to force frametime.
    pub fn get_frame(&self, time: f32, frametime: Option<f32>) -> Option<(usize, GhostFrame)> {
        let frame0 = self.frames.first()?;

        // No frame time, not sure how to accumulate correctly
        if frame0.frametime.is_none() && frametime.is_none() {
            return None;
        }

        let mut from_time = 0f32;
        let mut to_time = 0f32;
        let mut to_index = 0usize;

        for (index, frame) in self.frames.iter().enumerate() {
            let add_time = if let Some(frametime) = frametime {
                frametime
            } else {
                frame.frametime.unwrap()
            };

            // only exit when greater means we are having the "to" frame
            if to_time > time {
                break;
            }

            from_time = to_time;
            to_time += add_time;
            to_index = index;
        }

        if to_index == 0 {
            return Some((0, frame0.clone()));
        }

        // If exceeding the number of available frames then we have nothing.
        // This is to make sure that we know when it ends.
        if to_index == self.frames.len() - 1 && time >= to_time {
            return None;
        }

        let to_frame = self.frames.get(to_index)?;

        let from_frame = self.frames.get(to_index - 1).unwrap();

        let target = (time - from_time) / (to_time - from_time);
        // clamp because vec lerp extrapolates as well.
        let target = target.clamp(0., 1.);

        let new_origin = from_frame.origin.lerp(to_frame.origin, target as f32);

        let viewangles_diff: [f32; 3] = from_fn(|i| {
            angle_diff(
                // normalize is not what we want as we are in between +/-
                from_frame.viewangles[i],
                to_frame.viewangles[i],
            )
        });
        let viewangles_diff = Vec3::from(viewangles_diff);
        let new_viewangles = from_frame
            .viewangles
            // attention, lerp to `from + diff`
            .lerp(from_frame.viewangles + viewangles_diff, target as f32);

        let new_fov = if from_frame.fov.is_some() && to_frame.fov.is_some() {
            let from_fov = from_frame.fov.unwrap();
            let to_fov = to_frame.fov.unwrap();
            Some(from_fov.lerp(to_fov, target as f32))
        } else {
            None
        };

        // Maybe do some interpolation for sequence in the future? Though only demo would have it.
        Some((
            // to index is guaranteed to not be 0
            to_index - 1,
            GhostFrame {
                origin: new_origin,
                viewangles: new_viewangles,
                frametime: from_frame.frametime,
                buttons: from_frame.buttons,
                fov: new_fov,
                extras: from_frame.extras.clone(),
            },
        ))
    }

    /// Returns the frame index from a given time.
    pub fn get_frame_index(&self, time: f32, frametime: Option<f32>) -> usize {
        let mut to_time = 0f32;
        let mut to_index = 0usize;

        for (index, frame) in self.frames.iter().enumerate() {
            let add_time = if let Some(frametime) = frametime {
                frametime
            } else {
                frame.frametime.unwrap()
            };

            // only exit when greater means we are having the "to" frame
            if to_time > time {
                break;
            }

            to_time += add_time;
            to_index = index;
        }

        if to_index == 0 {
            return 0;
        }

        to_index
    }

    // /// Rotates viewangle and vieworigin around origin z axis (height) by `rotation` value
    // pub fn rotate(&mut self, rotation: f32) -> &mut Self {
    //     for frame in &mut self.frames {
    //         frame.
    //     }
    //     self
    // }

    // Returns a closure that we can re-use
    // This closure can also take in an argument in case the ghost doesn't have frame time.
    pub fn get_ghost_length(&self) -> Box<dyn Fn(f32) -> f32 + '_> {
        let has_frametime = self.frames[0].frametime.is_some();
        let maybe_total_length: f32 = self.frames.iter().filter_map(|frame| frame.frametime).sum();

        let use_custom_frametime = move |frametime: f32| frametime * self.frames.len() as f32;
        let ignore_custom_frametime = move |_: f32| maybe_total_length;

        if has_frametime {
            return Box::new(ignore_custom_frametime);
        } else {
            return Box::new(use_custom_frametime);
        }
    }

    pub fn has_sound(&self) -> bool {
        self.frames.iter().any(|frame| {
            frame
                .extras
                .as_ref()
                .map(|extra| !extra.sound.is_empty())
                .unwrap_or(false)
        })
    }
}

pub fn file_name_get_stem(file_name: &str) -> Option<String> {
    let p = Path::new(file_name);

    p.file_name()?.to_str()?.to_string().into()
}
