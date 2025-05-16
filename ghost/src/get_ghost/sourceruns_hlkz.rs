use eyre::eyre;
use nom::{
    IResult as _IResult,
    combinator::{all_consuming, map},
    multi::{count, many0},
    number::complete::{le_f32, le_u16},
    sequence::tuple,
};

use super::*;

struct SRHLKZGhostFrame {
    // accummulative
    pub time: f32,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub buttons: u16,
    // horizontal speed
    // pub _speed: f32,
}

type IResult<'a, T> = _IResult<&'a [u8], T>;

fn srhlkz_ghost_frame_parse(i: &[u8]) -> IResult<SRHLKZGhostFrame> {
    map(
        tuple((
            le_f32,
            count(le_f32, 3usize),
            count(le_f32, 3usize),
            le_u16,
            // le_f32,
        )),
        |(
            time,
            origin,
            angles,
            buttons,
            //  _speed
        )| SRHLKZGhostFrame {
            time,
            origin: from_fn(|i| origin[i]),
            angles: from_fn(|i| angles[i]),
            buttons,
            // _speed,
        },
    )(i)
}

pub fn srhlkz_ghost_parse(file_name: &str, file: &[u8]) -> eyre::Result<GhostInfo> {
    let (_, frames) = all_consuming(many0(srhlkz_ghost_frame_parse))(file)
        .map_err(|_| eyre!("Cannot parse SourceRuns HLKZ replay data"))?;

    // todo: do better
    let file_name = file_name_get_stem(file_name).unwrap();

    let map_name_end = file_name
        .find("_0_0_")
        .or(file_name.find("_0_1_"))
        .expect("cannot find delimiter to find map name");
    let map_name = &file_name[..map_name_end];

    // track prev time to derive frmame time
    // frame time is based on server time
    // so we start with this
    let mut prev_time = frames[0].time;

    Ok(GhostInfo {
        ghost_name: file_name.to_string(),
        map_name: map_name.to_string(),
        game_mod: "ag".into(),
        frames: frames
            .into_iter()
            .map(|frame| {
                let res = GhostFrame {
                    origin: frame.origin.into(),
                    viewangles: frame.angles.into(),
                    viewoffset_z: 0.,
                    frametime: (frame.time - prev_time).into(),
                    buttons: (frame.buttons as u32).into(),
                    fov: None,
                    extras: None,
                };

                prev_time = frame.time;

                res
            })
            .collect(),
    })
}
