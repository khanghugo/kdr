use std::{collections::HashMap, str::from_utf8};

use dem::{
    bit::BitSliceCast,
    types::{EngineMessage, FrameData, MessageData, NetMessage, TempEntity},
};
use nom::{
    IResult,
    bytes::complete::{tag, take_till},
};

use super::*;

pub fn demo_ghost_parse(filename: &str, demo: &Demo) -> eyre::Result<GhostInfo> {
    // Because player origin/viewangles and animation are on different frame, we have to sync it.
    // Order goes: players info > animation > player info > ...
    // TODO parses everything within netmsg
    let mut sequence: Option<i32> = None;
    let mut anim_frame: Option<f32> = None;
    let mut animtime: Option<f32> = None;
    let mut gaitsequence: Option<i32> = None;
    let mut blending: [u8; 2] = [0u8; 2];

    let mut origin = [0f32; 3];
    let mut viewangles = [0f32; 3];

    let mut fov: Option<f32> = None;

    // sound
    let mut sound_vec = vec![];

    // player names
    let mut player_names: HashMap<u8, String> = HashMap::new();

    // key is the resource index
    let mut resource_lookup: HashMap<u32, String> = HashMap::new();

    // weapon/viewmodel related stuffs
    let mut weapon_list: HashMap<u8, String> = HashMap::new();
    let mut weapon_sequence = None;

    // can only build resource lookup from entry 0
    demo.directory.entries[0]
        .frames
        .iter()
        .for_each(|frame| match &frame.frame_data {
            FrameData::NetworkMessage(a) => {
                let netmessage = &a.as_ref().1;

                let MessageData::Parsed(ref messages) = netmessage.messages else {
                    return;
                };

                messages.iter().for_each(|message| match &message {
                    NetMessage::EngineMessage(engine_message) => {
                        let a = engine_message.as_ref();

                        match a {
                            EngineMessage::SvcResourceList(resource_list) => {
                                resource_list.resources.iter().for_each(|resource| {
                                    // do not insert if somethign is already there, dont ask me why it is this way,
                                    let idx = resource.index.to_u32();

                                    if !resource_lookup.contains_key(&idx) {
                                        resource_lookup.insert(idx, resource.name.get_string());
                                    }
                                });
                            }
                            _ => (),
                        }
                    }
                    NetMessage::UserMessage(user_message) => {
                        let name = user_message.name.to_str().unwrap();

                        // this is only in entry 0
                        if name == "WeaponList" {
                            let null_pos = user_message
                                .data
                                .iter()
                                .position(|&x| x == 0)
                                .expect("invalid WeaponList user message");

                            let weapon_name = from_utf8(&user_message.data[..null_pos])
                                .unwrap()
                                .strip_prefix("weapon_")
                                .expect("weapon does not have `weapon_` prefix");
                            let weapon_id = user_message.data[user_message.data.len() - 2];

                            weapon_list.insert(weapon_id, weapon_name.to_string());
                        }
                    }
                });
            }
            _ => (),
        });

    // now ghost frames here
    let ghost_frames = demo.directory.entries[1]
        .frames
        .iter()
        .enumerate()
        .filter_map(|(_frame_idx, frame)| match &frame.frame_data {
            // FrameData::ClientData(client) => {
            //     Some(GhostFrame {
            //         origin: client.origin.into(),
            //         viewangles: client.viewangles.into(),
            //         frametime: Some(frame.time as f64), /* time here is accummulative, will fix
            //                                              * after */
            //         sequence: None,
            //         frame: None,
            //         animtime: None,
            //         buttons: None,
            //     })
            // }
            FrameData::WeaponAnimation(weapon_anim) => {
                weapon_sequence = weapon_anim.sequence.into();

                None
            }
            FrameData::ClientData(client) => {
                // origin = [client.origin[0], client.origin[1], client.origin[2]];

                viewangles = [
                    client.viewangles[0],
                    client.viewangles[1],
                    client.viewangles[2],
                ];
                fov = client.fov.into();

                // ClientData happens before NetMsg so we can reset some values here.
                sequence = None;
                anim_frame = None;
                animtime = None;
                gaitsequence = None;

                None
            }
            FrameData::Sound(sound) => {
                let sound_frame = GhostFrameSound {
                    file_name: sound.sample.to_str().unwrap().to_owned(),
                    channel: sound.channel,
                    volume: sound.volume,
                    origin: None,
                };

                sound_vec.push(sound_frame);
                None
            }
            FrameData::NetworkMessage(box_type) => {
                let netmessage = &box_type.as_ref().1;

                let MessageData::Parsed(ref messages) = netmessage.messages else {
                    return None;
                };

                let sim_org = &netmessage.info.refparams.sim_org;
                let view_height = &netmessage.info.refparams.view_height;
                // origin = [sim_org[0] + vie, sim_org[1], sim_org[2]];
                origin = from_fn(|i| sim_org[i] + view_height[i]);

                let mut entity_text = vec![];
                let mut say_text = vec![];
                let mut weapon_change = None;

                messages.iter().for_each(|message| {
                    match message {
                        // get player names
                        NetMessage::UserMessage(user_message) => {
                            let message_name = user_message.name.to_str().unwrap();

                            if message_name == "SayText" {
                                // need to sub 1 because this idx is higher than 1 in our name list
                                let player_idx = user_message.data[0] - 1;
                                let player_name = player_names.get(&player_idx).unwrap();

                                // println!("{:?}", user_message.data);

                                let (_, saytext) =
                                    processing_saytext2(&user_message.data[1..], player_name)
                                        .unwrap();

                                say_text.push(saytext);
                            }

                            if message_name == "CurWeapon" {
                                let weapon_state = user_message.data[0];
                                let weapon_id = user_message.data[1];

                                if weapon_state != 0 {
                                    weapon_change =
                                        weapon_list.get(&weapon_id).unwrap().to_string().into()
                                }
                            }
                        }
                        NetMessage::EngineMessage(engine_message) => match &**engine_message {
                            // entity text on screen
                            EngineMessage::SvcTempEntity(temp_entity) => {
                                if let TempEntity::TeTextMessage(ref text_entity) =
                                    temp_entity.entity
                                {
                                    let text_color: Vec<f32> = text_entity
                                        .text_color
                                        .iter()
                                        .map(|&c| c as f32 / 255.)
                                        .collect();

                                    let text_color: [f32; 4] = from_fn(|i| text_color[i]);

                                    let normalize_pos = |x: i16| {
                                        if x == -8192 {
                                            return 0.5;
                                        }

                                        x as f32 / 8192.
                                    };

                                    let frame_text = GhostFrameEntityText {
                                        text: text_entity.message.to_str().unwrap().to_string(),
                                        // need to normalize position
                                        location: [
                                            normalize_pos(text_entity.x),
                                            normalize_pos(text_entity.y),
                                        ],
                                        color: text_color,
                                        // life is in msec, not sec
                                        life: (text_entity.hold_time
                                            + text_entity.fade_in_time
                                            + text_entity.fade_out_time)
                                            as f32
                                            / 1000.,
                                        channel: text_entity.channel,
                                    };

                                    entity_text.push(frame_text);
                                }
                            }
                            // sounds
                            EngineMessage::SvcSound(sound) => {
                                let sound_index = sound
                                    .sound_index_short
                                    .as_ref()
                                    .or(sound.sound_index_long.as_ref())
                                    .map(|i| i.to_u32())
                                    .expect("sound does not have a resource index");

                                let Some(sound_name) = resource_lookup.get(&sound_index) else {
                                    println!("no sound found");
                                    return;
                                };

                                let volume = sound
                                    .volume
                                    .as_ref()
                                    .map(|volume| volume.to_u32() as f32 / 255.)
                                    .unwrap_or(1.0);

                                let mut origin = None;
                                if let Some(x) = &sound.origin_x {
                                    if let Some(y) = &sound.origin_y {
                                        if let Some(z) = &sound.origin_z {
                                            let x = x.to_number();
                                            let y = y.to_number();
                                            let z = z.to_number();

                                            origin = [x, y, z].into();
                                        }
                                    }
                                }

                                let sound_name_length = sound_name.len();

                                let sound_frame = GhostFrameSound {
                                    // excluding null terminator
                                    file_name: sound_name[..sound_name_length - 1].to_owned(),
                                    channel: sound.channel.to_i32(),
                                    volume,
                                    origin,
                                };

                                sound_vec.push(sound_frame);
                            }
                            // build name list
                            EngineMessage::SvcUpdateUserInfo(user_info) => {
                                // this index is 1 lower than the index in the say text
                                // this means if user index is 2, the index inside saytext is 3
                                let user_index = user_info.index;

                                // "\\bottomcolor\\6\\cl_dlmax\\512\\cl_lc\\1\\cl_lw\\1\\cl_updaterate\\102\\topcolor\\30\\rate\\100000\\name\\hono dille\\*sid\\76561198152358431\\model\\sas"
                                let info_str = user_info.user_info.to_str().unwrap();

                                let key = "\\name\\";
                                if let Some(name_start) = info_str.find("\\name\\") {
                                    let name_really_start = name_start + key.len();
                                    let name_length = info_str[name_really_start..]
                                        .find("\\")
                                        .unwrap_or(info_str.len() - name_really_start);

                                    player_names.insert(
                                        user_index,
                                        info_str
                                            [name_really_start..(name_really_start + name_length)]
                                            .to_string(),
                                    );
                                }
                            }
                            // animations
                            EngineMessage::SvcDeltaPacketEntities(delta_packet_entities) => {
                                let Some(player_delta) = delta_packet_entities.entity_states.get(0)
                                else {
                                    return;
                                };

                                let Some(delta) = player_delta.delta.as_ref() else {
                                    return;
                                };

                                if let Some(sequence_bytes) = delta.get("sequence\0") {
                                    let sequence_bytes: [u8; 4] = from_fn(|i| sequence_bytes[i]);
                                    sequence = Some(i32::from_le_bytes(sequence_bytes));
                                }

                                if let Some(anim_frame_bytes) = delta.get("frame\0") {
                                    let anim_frame_bytes: [u8; 4] =
                                        from_fn(|i: usize| anim_frame_bytes[i]);
                                    anim_frame = Some(f32::from_le_bytes(anim_frame_bytes));
                                }

                                if let Some(animtime_bytes) = delta.get("animtime\0") {
                                    let animtime_bytes: [u8; 4] = from_fn(|i| animtime_bytes[i]);
                                    animtime = Some(f32::from_le_bytes(animtime_bytes));
                                }

                                if let Some(gaitsequence_bytes) = delta.get("gaitsequence\0") {
                                    let gaitsequence_bytes: [u8; 4] =
                                        from_fn(|i| gaitsequence_bytes[i]);
                                    gaitsequence = Some(i32::from_le_bytes(gaitsequence_bytes));
                                }

                                if let Some(blending0) = delta.get("blending[0]\0") {
                                    // blending is just [u8; 1]
                                    blending[0] = blending0[0];
                                }

                                if let Some(blending1) = delta.get("blending[1]\0") {
                                    // blending is just [u8; 1]
                                    blending[1] = blending1[0];
                                }
                            }
                            _ => (),
                        },
                    }
                });

                let frame_extra = GhostFrameExtra {
                    sound: sound_vec.to_owned(),
                    entity_text,
                    anim: Some(GhostFrameAnim {
                        sequence,
                        frame: anim_frame,
                        animtime,
                        gaitsequence,
                        blending,
                    }),
                    say_text,
                    weapon_change,
                    weapon_sequence,
                };

                weapon_sequence = None;
                sound_vec.clear();

                Some(GhostFrame {
                    origin: Vec3::from_array(origin),
                    viewangles: Vec3::from_array(viewangles),
                    frametime: Some(frame.time), /* time here is accummulative, will fix
                                                  * after */
                    buttons: None,
                    fov,
                    extras: frame_extra.into(),
                })
            }
            _ => None,
        })
        .scan(0., |acc, mut frame: GhostFrame| {
            // Cummulative time is 1 2 3 4, so do subtraction to get the correct frametime
            // TODO i think iam stupid, there is no neeed to calculate frame time like this
            // can just derive from demo right away
            // matter of fact, rewrite this horseshit
            let cum_time = frame.frametime.unwrap();

            frame.frametime = Some(cum_time - *acc);
            *acc = cum_time;

            Some(frame)
        })
        .collect::<Vec<GhostFrame>>();

    let map_name = demo.header.map_name.to_str()?.to_string();
    let game_mod = demo.header.game_directory.to_str()?.to_string();

    Ok(GhostInfo {
        ghost_name: filename.to_owned(),
        map_name,
        game_mod,
        frames: ghost_frames,
    })
}

fn processing_saytext2<'a>(
    byte_string: &'a [u8],
    player_name: &'a str,
) -> IResult<&'a [u8], GhostFrameSayText> {
    let is_header = |i: u8| i <= 4;

    let (mut remaining, initial_chunk) = take_till(is_header)(byte_string)?;
    let mut chunks = Vec::new();

    // beginning chunk with no header, eg spectator text
    if !initial_chunk.is_empty() {
        chunks.push((1, initial_chunk)); // 1 = system text
    }

    while !remaining.is_empty() {
        let (rest, header) =
            nom::branch::alt((tag(&[0]), tag(&[1]), tag(&[2]), tag(&[3]), tag(&[4])))(remaining)?;

        let (rest, content) = take_till(is_header)(rest)?;

        if !content.is_empty() {
            chunks.push((header[0], content));
        }

        remaining = rest;
    }

    Ok((
        remaining,
        GhostFrameSayText {
            text: chunks
                .into_iter()
                .map(|(header, text)| {
                    let mut text =
                        String::from_utf8_lossy(text).replace(std::char::REPLACEMENT_CHARACTER, "");

                    const ALL_CHAT_PTRN: &str = "#Cstrike_Chat_All";
                    const SPEC_CHAT_PTRN: &str = "#Cstrike_Chat_AllSpec";
                    const SPEC_TEAM_CHAT_PTRN: &str = "#Cstrike_Chat_Spec";

                    let is_all_chat = text.contains(ALL_CHAT_PTRN);
                    let is_spec_chat = text.contains(SPEC_CHAT_PTRN);
                    let is_spec_team_chat = text.contains(SPEC_TEAM_CHAT_PTRN);

                    if is_spec_chat {
                        text = text.replace(SPEC_CHAT_PTRN, "");

                        text = format!("*SPEC* {player_name}: {text}");
                    } else if is_all_chat {
                        text = text.replace(ALL_CHAT_PTRN, "");

                        text = format!("{player_name}: {text}");
                    } else if is_spec_team_chat {
                        text = text.replace(SPEC_TEAM_CHAT_PTRN, "");

                        text = format!("(Spectator) {player_name}: {text}");
                    }

                    // clean up the text
                    let text = text.replace("\n", "");

                    (header, text)
                })
                .collect(),
        },
    ))
}
