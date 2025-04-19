use std::collections::HashMap;

use dem::{
    bit::BitSliceCast,
    types::{EngineMessage, FrameData, MessageData, NetMessage, TempEntity},
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
    // No need to do optional type for this.
    // Just make sure that blending is persistent across frames.
    let mut blending = [0u8; 2];

    let mut origin = [0f32; 3];
    let mut viewangles = [0f32; 3];

    let mut fov: Option<f32> = None;

    // sound
    let mut sound_vec = vec![];

    // key is the resource index
    let mut resource_lookup: HashMap<u32, String> = HashMap::new();

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
                    _ => (),
                });
            }
            _ => (),
        });

    // now ghost frames here
    let ghost_frames = demo.directory.entries[1]
        .frames
        .iter()
        .filter_map(|frame| match &frame.frame_data {
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
                origin = [sim_org[0], sim_org[1], sim_org[2]];

                // Every time there is svc_clientdata, there is svc_deltapacketentities
                // Even if there isn't, this is more safe to make sure that we have the client data.
                let client_data = messages.iter().find_map(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcClientData(ref client_data) = **engine_message {
                            Some(client_data)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                // If no client_dat then we that means there won't be packet entity. Typically.
                client_data?;

                // Cannot use client_data here because it only reports delta.
                // Even though it is something that can be worked with. Ehh.
                // let client_data = client_data.unwrap();

                // let (origin, viewangles) = if let Some(client_data) = client_data {
                //     (client_data.client_data.get(""))
                // } else {
                //     (None, None)
                // };

                let delta_packet_entities = messages.iter().find_map(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcDeltaPacketEntities(ref delta_packet_entities) =
                            **engine_message
                        {
                            Some(delta_packet_entities)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                if let Some(delta_packet_entities) = delta_packet_entities {
                    if !delta_packet_entities.entity_states.is_empty()
                        && delta_packet_entities.entity_states[0].delta.is_some()
                    {
                        let delta = &delta_packet_entities.entity_states[0]
                            .delta
                            .as_ref()
                            .unwrap();

                        if let Some(sequence_bytes) = delta.get("sequence\0") {
                            let sequence_bytes: [u8; 4] = from_fn(|i| sequence_bytes[i]);
                            sequence = Some(i32::from_le_bytes(sequence_bytes));
                        }

                        if let Some(anim_frame_bytes) = delta.get("frame\0") {
                            let anim_frame_bytes: [u8; 4] = from_fn(|i: usize| anim_frame_bytes[i]);
                            anim_frame = Some(f32::from_le_bytes(anim_frame_bytes));
                        }

                        if let Some(animtime_bytes) = delta.get("animtime\0") {
                            let animtime_bytes: [u8; 4] = from_fn(|i| animtime_bytes[i]);
                            animtime = Some(f32::from_le_bytes(animtime_bytes));
                        }

                        if let Some(gaitsequence_bytes) = delta.get("gaitsequence\0") {
                            let gaitsequence_bytes: [u8; 4] = from_fn(|i| gaitsequence_bytes[i]);
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
                }

                let mut text = vec![];

                // text entity
                messages.iter().for_each(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcTempEntity(ref temp_entity) = **engine_message {
                            if let TempEntity::TeTextMessage(ref text_entity) = temp_entity.entity {
                                let text_color: Vec<f32> = text_entity
                                    .text_color
                                    .iter()
                                    .map(|&c| c as f32 / 255.)
                                    .collect();

                                // println!("{:?}", text_entity);
                                let text_color: [f32; 4] = from_fn(|i| text_color[i]);

                                let normalize_pos = |x: i16| {
                                    if x == -8192 {
                                        return 0.5;
                                    }

                                    x as f32 / 8192.
                                };

                                let frame_text = GhostFrameText {
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

                                text.push(frame_text);
                            }
                        }
                    }
                });

                messages.iter().for_each(|message| {
                    if let NetMessage::EngineMessage(engine_message) = message {
                        if let EngineMessage::SvcSound(ref sound) = **engine_message {
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
                    }
                });

                let frame_extra = GhostFrameExtra {
                    sound: sound_vec.to_owned(),
                    text,
                    anim: Some(GhostFrameAnim {
                        sequence,
                        frame: anim_frame,
                        animtime,
                        gaitsequence,
                        blending,
                    }),
                };

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

    ghost_frames
        .iter()
        .filter_map(|f| f.extras.as_ref())
        .filter(|f| !f.sound.is_empty())
        .for_each(|f| println!("{:?}", f.sound));

    Ok(GhostInfo {
        ghost_name: filename.to_owned(),
        map_name,
        game_mod,
        frames: ghost_frames,
    })
}
