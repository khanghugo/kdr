use std::array::from_fn;

pub fn build_mvp_from_origin_angles(
    origin: [f32; 3],
    angles: cgmath::Quaternion<f32>,
) -> cgmath::Matrix4<f32> {
    let rotation: cgmath::Matrix4<f32> = angles.into();

    cgmath::Matrix4::from_translation(origin.into()) * rotation
}

pub struct MdlAngles(pub [f32; 3]);

impl MdlAngles {
    // "The Half-Life engine uses a left handed coordinate system, where X is forward, Y is left and Z is up."
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [angles[0], angles[1], angles[2]]
    }
}

pub struct BspAngles(pub [f32; 3]);

impl BspAngles {
    pub fn get_world_angles(&self) -> [f32; 3] {
        let angles = self.0;
        [-angles[0], angles[2], angles[1]]
    }
}

// all assuming that we only have 1 bone
pub fn get_idle_sequence_origin_angles(mdl: &mdl::Mdl) -> ([f32; 3], MdlAngles) {
    let sequence0 = &mdl.sequences[0];
    let blend0 = &sequence0.anim_blends[0];
    let bone_blend0 = &blend0[0];
    let bone0 = &mdl.bones[0];

    // let origin: [f32; 3] = from_fn(|i| {
    //     bone_blend0[i] // motion type
    //                 [0] // frame 0
    //         as f32 // casting
    //             * bone0.scale[i] // scale factor
    //             + bone0.value[i] // bone default value
    // });

    // apparently origin doesnt matter
    let origin = [0f32; 3];

    let angles: [f32; 3] =
        from_fn(|i| bone_blend0[i + 3][0] as f32 * bone0.scale[i + 3] + bone0.value[i + 3]);

    (origin, MdlAngles(angles))
}

#[macro_export]
macro_rules! err {
    ($e: ident) => {{
        use eyre::eyre;

        Err(eyre!($e))
    }};

    ($format_string: literal) => {{
        use eyre::eyre;

        Err(eyre!($format_string))
    }};

    ($($arg:tt)*) => {{
        use eyre::eyre;

        Err(eyre!($($arg)*))
    }};
}
