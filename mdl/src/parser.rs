use std::{array::from_fn, ffi::OsStr, fs::OpenOptions, io::Read, path::Path};

use eyre::eyre;
use nom::{
    bytes::complete::take,
    combinator::map,
    multi::count,
    number::complete::{le_f32, le_i16, le_i32, le_u8, le_u16},
    sequence::tuple,
};

use crate::{
    Attachment, Blend, Bodypart, BodypartHeader, Bone, BoneController, Hitbox, Mesh, MeshHeader,
    MeshTriangles, Model, ModelHeader, PALETTE_COUNT, Sequence, SequenceGroup, SkinFamilies,
    Trivert, TrivertHeader, VEC3_T_SIZE,
    nom_helpers::{IResult, vec3},
    types::{Header, Mdl, SequenceHeader, Texture, TextureFlag, TextureHeader},
};

impl Mdl {
    pub fn open_from_bytes(bytes: &[u8]) -> eyre::Result<Self> {
        match parse_mdl(bytes) {
            Ok((_, mdl)) => Ok(mdl),
            Err(_) => Err(eyre!("cannot parse mdl")),
        }
    }

    pub fn open_from_file(path: impl AsRef<OsStr> + AsRef<Path>) -> eyre::Result<Self> {
        let mut file = OpenOptions::new().read(true).open(path)?;
        let mut bytes = vec![];

        file.read_to_end(&mut bytes)?;

        Self::open_from_bytes(&bytes)
    }
}

fn parse_mdl(i: &[u8]) -> IResult<Mdl> {
    let start = i;
    let (_, mdl_header) = parse_header(start)?;

    let (_, sequences) = parse_sequences(start, &mdl_header)?;

    let (_, textures) = parse_textures(start, &mdl_header)?;

    let (_, bodyparts) = parse_bodyparts(start, &mdl_header)?;

    let (_, bones) = parse_bones(start, &mdl_header)?;

    let (_, bone_controllers) = parse_bone_controllers(start, &mdl_header)?;

    let (_, hitboxes) = parse_hitboxes(start, &mdl_header)?;

    let (_, sequence_groups) = parse_sequence_groups(start, &mdl_header)?;

    let (_, skin_families) = parse_skin_families(start, &mdl_header)?;

    let (_, attachments) = parse_attachments(start, &mdl_header)?;

    Ok((
        i,
        Mdl {
            header: mdl_header,
            sequences,
            textures,
            bodyparts,
            bones,
            bone_controllers,
            hitboxes,
            sequence_groups,
            skin_families,
            attachments,
        },
    ))
}

fn parse_header(i: &[u8]) -> IResult<Header> {
    map(
        tuple((
            tuple((
                le_i32,
                le_i32,
                count(le_u8, 64),
                le_i32,
                vec3,
                vec3,
                vec3,
                vec3,
                vec3,
                le_i32,
            )),
            tuple((
                le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32,
                le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32, le_i32,
            )),
            tuple((le_i32, le_i32, le_i32, le_i32, le_i32, le_i32)),
        )),
        |(
            (id, version, name, length, eye_position, min, max, bbmin, bbmax, flags),
            (
                num_bones,
                bone_index,
                num_bone_controllers,
                bone_controller_index,
                num_hitboxes,
                hitbox_index,
                num_seq,
                seq_index,
                num_seq_group,
                seq_group_index,
                num_textures,
                texture_index,
                texture_data_index,
                num_skin_ref,
                num_skin_families,
                skin_index,
                num_body_parts,
                body_part_index,
                num_attachments,
                attachment_index,
            ),
            (
                sound_table,
                sound_index,
                sound_groups,
                sound_group_index,
                num_transitions,
                transition_index,
            ),
        )| Header {
            id,
            version,
            name: from_fn(|i| name[i]),
            length,
            eye_position,
            min,
            max,
            bbmin,
            bbmax,
            flags,
            num_bones,
            bone_index,
            num_bone_controllers,
            bone_controller_index,
            num_hitboxes,
            hitbox_index,
            num_seq,
            seq_index,
            num_seq_group,
            seq_group_index,
            num_textures,
            texture_index,
            texture_data_index,
            num_skin_ref,
            num_skin_families,
            skin_index,
            num_bodyparts: num_body_parts,
            bodypart_index: body_part_index,
            num_attachments,
            attachment_index,
            sound_table,
            sound_index,
            sound_groups,
            sound_group_index,
            num_transitions,
            transition_index,
        },
    )(i)
}

// https://github.com/LogicAndTrick/sledge-formats/blob/7a3bfb33562aece483e15796b8573b23d71319ab/Sledge.Formats.Model/Goldsource/MdlFile.cs#L442
fn parse_animation_frame_values(br: &[u8], read_count: usize) -> IResult<Vec<u16>> {
    let mut values: Vec<u16> = vec![0; read_count];

    let mut i = 0;

    let mut reader = br;

    while i < read_count {
        let (br, run) = take(2usize)(reader)?;
        let (br, vals) = count(le_u16, run[0] as usize)(br)?;

        reader = br;

        let mut j = 0;

        while j < run[1] && i < read_count {
            if !vals.is_empty() {
                let idx = (run[0] - 1).min(j);
                values[i] = vals[idx as usize];
            }

            i += 1;
            j += 1;
        }
    }

    Ok((reader, values))
}

// parse starting from animation offset
fn parse_blend<'a>(
    // panimvalue points to the current blend
    // the layout goes
    // - blend 1 offsets
    // - - bone 0 offsets
    // - - bone 1 offsets
    // - - ...
    // - blend 2 offsets
    // - - bone 0 offsets
    // - - ...
    //
    // bone N offsets starts from panimvalue
    // starting from the offset is a RLE
    // this RLE contains all animation values for that one motion type
    //
    // so, the result for 1 blend is: X amount of bone for 6 arrays of Y animation value for that motion type
    // the type is [[[short animation value; animation count]; 6 motion types]; X bone]
    panimvalue: &'a [u8],
    mdl_header: &Header,
    sequence_header: &SequenceHeader,
) -> IResult<'a, Blend> {
    let offset_parser = map(count(le_u16, 6 as usize), |res| {
        [res[0], res[1], res[2], res[3], res[4], res[5]]
    });

    let (end_of_blend, blend) = count(offset_parser, mdl_header.num_bones as usize)(panimvalue)?;

    // the animation frame is offset from the beginning of the panim "struct", which is anim_offset + current blend number
    // https://github.com/ValveSoftware/halflife/blob/c7240b965743a53a29491dd49320c88eecf6257b/utils/mdlviewer/studio_render.cpp#L190
    let mut res: Blend = vec![];
    let num_frames = sequence_header.num_frames as usize;

    // at the moment, we have the bone count and the offsets
    // now we have to fit animations inside the bone count
    for bone in blend {
        let mut bone_values: [Vec<u16>; 6] = from_fn(|_| vec![0; num_frames]);

        for (motion_idx, offset) in bone.into_iter().enumerate() {
            if offset == 0 {
                continue;
            }

            let rle_start = &panimvalue[offset as usize..];
            let (_, values) = parse_animation_frame_values(rle_start, num_frames)?;

            bone_values[motion_idx] = values;
        }

        res.push(bone_values);
    }

    Ok((end_of_blend, res))
}

fn parse_sequence<'a>(start: &'a [u8], i: &'a [u8], mdl_header: &Header) -> IResult<'a, Sequence> {
    let (_, header) = parse_sequence_description(i)?;

    let animation_frame_parser = |i| parse_blend(i, mdl_header, &header);

    let (_, anim_blends) = count(animation_frame_parser, header.num_blends as usize)(
        &start[header.anim_index as usize..],
    )?;

    Ok((
        &[],
        Sequence {
            header,
            anim_blends,
        },
    ))
}

fn parse_sequences<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Sequence>> {
    let parser = |i| parse_sequence(start, i, mdl_header);
    count(parser, mdl_header.num_seq as usize)(&start[mdl_header.seq_index as usize..])
}

fn parse_sequence_description(i: &[u8]) -> IResult<SequenceHeader> {
    map(
        tuple((
            tuple((
                count(le_u8, 32),
                le_f32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                le_i32,
                vec3,
                le_i32,
                le_i32,
            )),
            tuple((
                vec3,
                vec3,
                le_i32,
                le_i32,
                count(le_i32, 2),
                count(le_f32, 2),
                count(le_f32, 2),
                le_i32,
            )),
            tuple((le_i32, le_i32, le_i32, le_i32, le_i32)),
        )),
        |(
            (
                label,
                fps,
                flags,
                activity,
                act_weight,
                num_events,
                event_index,
                num_frames,
                num_pivots,
                pivot_index,
                motion_type,
                motion_bone,
                linear_movement,
                auto_move_pos_index,
                auto_move_angle_index,
            ),
            (
                bbmin,
                bbmax,
                num_blends,
                anim_index,
                blend_type,
                blend_start,
                blend_end,
                blend_parent,
            ),
            (seq_group, entry_node, exit_node, node_flags, next_seq),
        )| SequenceHeader {
            label: from_fn(|i| label[i]),
            fps,
            flags,
            activity,
            act_weight,
            num_events,
            event_index,
            num_frames,
            num_pivots,
            pivot_index,
            motion_type,
            motion_bone,
            linear_movement,
            auto_move_pos_index,
            auto_move_angle_index,
            bbmin,
            bbmax,
            num_blends,
            anim_index,
            blend_type: from_fn(|i| blend_type[i]),
            blend_start: from_fn(|i| blend_start[i]),
            blend_end: from_fn(|i| blend_end[i]),
            blend_parent,
            seq_group,
            entry_node,
            exit_node,
            node_flags,
            next_seq,
        },
    )(i)
}

fn parse_texture_header(i: &[u8]) -> IResult<TextureHeader> {
    map(
        tuple((count(le_u8, 64), le_i32, le_i32, le_i32, le_i32)),
        |(name, flags, width, height, index)| TextureHeader {
            name: from_fn(|i| name[i]),
            flags: TextureFlag::from_bits(flags).unwrap_or_else(|| {
                println!("unknown texture flag {flags}");
                TextureFlag::empty()
            }),
            width,
            height,
            index,
        },
    )(i)
}

fn parse_texture<'a>(i: &'a [u8], start: &'a [u8]) -> IResult<'a, Texture> {
    let (end_of_header, texture_header) = parse_texture_header(i)?;

    let (end_of_texture, texture_bytes): (_, &[u8]) =
        take((texture_header.width * texture_header.height) as usize)(
            &start[texture_header.index as usize..],
        )?;

    let (_, palette) = take(PALETTE_COUNT * 3)(end_of_texture)?;
    let palette: [[u8; 3]; PALETTE_COUNT] = from_fn(|i| {
        palette
            .chunks(3)
            .map(|i| [i[0], i[1], i[2]])
            .collect::<Vec<[u8; 3]>>()[i]
    });

    Ok((
        end_of_header,
        Texture {
            header: texture_header,
            image: texture_bytes.to_vec(),
            palette,
        },
    ))
}

fn parse_textures<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Texture>> {
    let parser = |i| parse_texture(i, start);

    count(parser, mdl_header.num_textures as usize)(&start[mdl_header.texture_index as usize..])
}

fn parse_trivert_header(i: &[u8]) -> IResult<TrivertHeader> {
    map(
        tuple((le_i16, le_i16, le_i16, le_i16)),
        |(vert_index, norm_index, s, t)| TrivertHeader {
            vert_index,
            norm_index,
            s,
            t,
        },
    )(i)
}

fn parse_trivert<'a>(
    i: &'a [u8],
    start: &'a [u8],
    model_header: &ModelHeader,
) -> IResult<'a, Trivert> {
    let (end_of_header, trivert_header) = parse_trivert_header(i)?;

    let vert_offset = VEC3_T_SIZE * trivert_header.vert_index as usize;
    let norm_offset = VEC3_T_SIZE * trivert_header.norm_index as usize;

    let (_, vertex) = vec3(&start[(model_header.vert_index as usize + vert_offset)..])?;
    let (_, normal) = vec3(&start[(model_header.norm_index as usize + norm_offset)..])?;

    Ok((
        end_of_header,
        Trivert {
            header: trivert_header,
            vertex,
            normal,
        },
    ))
}

fn parse_mesh_triangles<'a>(
    start: &'a [u8],
    model_header: &ModelHeader,
    mesh_header: &MeshHeader,
) -> IResult<'a, Vec<MeshTriangles>> {
    let mut res: Vec<MeshTriangles> = vec![];

    let parser = |i| parse_trivert(i, start, model_header);

    let mut trivert_run_start = &start[mesh_header.tri_index as usize..];

    loop {
        let (i, trivert_count) = le_i16(trivert_run_start)?;
        let trivert_count_abs = trivert_count.abs();

        if trivert_count_abs == 0 {
            break;
        }

        let (i, triverts) = count(parser, trivert_count_abs as usize)(i)?;

        let triangles = if trivert_count.is_positive() {
            MeshTriangles::Strip(triverts)
        } else {
            MeshTriangles::Fan(triverts)
        };

        res.push(triangles);

        trivert_run_start = i;
    }

    Ok((trivert_run_start, res))
}

fn parse_mesh_header(i: &[u8]) -> IResult<MeshHeader> {
    map(
        tuple((le_i32, le_i32, le_i32, le_i32, le_i32)),
        |(num_tris, tri_index, skin_ref, num_norms, norm_index)| MeshHeader {
            num_tris,
            tri_index,
            skin_ref,
            num_norms,
            norm_index,
        },
    )(i)
}

fn parse_mesh<'a>(i: &'a [u8], start: &'a [u8], model_header: &ModelHeader) -> IResult<'a, Mesh> {
    let (end_of_header, mesh_header) = parse_mesh_header(i)?;
    let (_end_of_triverts, triangles) = parse_mesh_triangles(start, model_header, &mesh_header)?;

    Ok((
        end_of_header,
        Mesh {
            header: mesh_header,
            triangles,
        },
    ))
}

fn parse_meshes<'a>(start: &'a [u8], model_header: &ModelHeader) -> IResult<'a, Vec<Mesh>> {
    let parser = |i| parse_mesh(i, start, model_header);

    count(parser, model_header.num_mesh as usize)(&start[model_header.mesh_index as usize..])
}

fn parse_model_header(i: &[u8]) -> IResult<ModelHeader> {
    map(
        tuple((
            count(le_u8, 64),
            le_i32,
            le_f32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
            le_i32,
        )),
        |(
            name,
            type_,
            bounding_radius,
            num_mesh,
            mesh_index,
            num_verts,
            vert_info_index,
            vert_index,
            num_norms,
            norm_info_index,
            norm_index,
            num_groups,
            group_index,
        )| ModelHeader {
            name: from_fn(|i| name[i]),
            type_,
            bounding_radius,
            num_mesh,
            mesh_index,
            num_verts,
            vert_info_index,
            vert_index,
            num_norms,
            norm_info_index,
            norm_index,
            num_groups,
            group_index,
        },
    )(i)
}

fn parse_model<'a>(i: &'a [u8], start: &'a [u8]) -> IResult<'a, Model> {
    let (end_of_header, model_header) = parse_model_header(i)?;
    let (_end_of_meshes, meshes) = parse_meshes(start, &model_header)?;

    Ok((
        end_of_header,
        Model {
            header: model_header,
            meshes,
        },
    ))
}

fn parse_models<'a>(start: &'a [u8], bodypart_header: &BodypartHeader) -> IResult<'a, Vec<Model>> {
    let parser = |i| parse_model(i, start);

    count(parser, bodypart_header.num_models as usize)(
        &start[bodypart_header.model_index as usize..],
    )
}

fn parse_bodypart_header(i: &[u8]) -> IResult<BodypartHeader> {
    map(
        tuple((count(le_u8, 64), le_i32, le_i32, le_i32)),
        |(name, num_models, base, model_index)| BodypartHeader {
            name: from_fn(|i| name[i]),
            num_models,
            base,
            model_index,
        },
    )(i)
}

fn parse_bodypart<'a>(i: &'a [u8], start: &'a [u8]) -> IResult<'a, Bodypart> {
    let (end_of_header, bodypart_header) = parse_bodypart_header(i)?;
    let (_end_of_models, models) = parse_models(start, &bodypart_header)?;

    Ok((
        end_of_header,
        Bodypart {
            header: bodypart_header,
            models,
        },
    ))
}

fn parse_bodyparts<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Bodypart>> {
    let parser = |i| parse_bodypart(i, start);

    count(parser, mdl_header.num_bodyparts as usize)(&start[mdl_header.bodypart_index as usize..])
}

fn parse_bone(i: &[u8]) -> IResult<Bone> {
    map(
        tuple((
            count(le_u8, 32),
            le_i32,
            le_i32,
            count(le_i32, 6),
            count(le_f32, 6),
            count(le_f32, 6),
        )),
        |(name, parent, flags, bone_controller, value, scale)| Bone {
            name: from_fn(|i| name[i]),
            parent,
            flags,
            bone_controller: from_fn(|i| bone_controller[i]),
            value: from_fn(|i| value[i]),
            scale: from_fn(|i| scale[i]),
        },
    )(i)
}

fn parse_bones<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Bone>> {
    count(parse_bone, mdl_header.num_bones as usize)(&start[mdl_header.bone_index as usize..])
}

fn parse_bone_controller(i: &[u8]) -> IResult<BoneController> {
    map(
        tuple((le_i32, le_i32, le_f32, le_f32, le_i32, le_i32)),
        |(bone, type_, start, end, rest, index)| BoneController {
            bone,
            type_,
            start,
            end,
            rest,
            index,
        },
    )(i)
}

fn parse_bone_controllers<'a>(
    start: &'a [u8],
    mdl_header: &Header,
) -> IResult<'a, Vec<BoneController>> {
    count(
        parse_bone_controller,
        mdl_header.num_bone_controllers as usize,
    )(&start[mdl_header.bone_controller_index as usize..])
}

pub fn parse_hitbox(i: &[u8]) -> IResult<Hitbox> {
    map(
        tuple((le_i32, le_i32, vec3, vec3)),
        |(bone, group, bbmin, bbmax)| Hitbox {
            bone,
            group,
            bbmin,
            bbmax,
        },
    )(i)
}

pub fn parse_hitboxes<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Hitbox>> {
    count(parse_hitbox, mdl_header.num_hitboxes as usize)(
        &start[mdl_header.hitbox_index as usize..],
    )
}

pub fn parse_sequence_group(i: &[u8]) -> IResult<SequenceGroup> {
    map(
        tuple((count(le_u8, 32), count(le_u8, 64), le_i32, le_i32)),
        |(label, name, unused1, unused2)| SequenceGroup {
            label: from_fn(|i| label[i]),
            name: from_fn(|i| name[i]),
            unused1,
            unused2,
        },
    )(i)
}

pub fn parse_sequence_groups<'a>(
    start: &'a [u8],
    mdl_header: &Header,
) -> IResult<'a, Vec<SequenceGroup>> {
    count(parse_sequence_group, mdl_header.num_seq as usize)(
        &start[mdl_header.seq_group_index as usize..],
    )
}

pub fn parse_skin_families<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, SkinFamilies> {
    count(
        count(le_i16, mdl_header.num_skin_ref as usize),
        mdl_header.num_skin_families as usize,
    )(&start[mdl_header.skin_index as usize..])
}

pub fn parse_attachment(i: &[u8]) -> IResult<Attachment> {
    map(
        tuple((count(le_u8, 32), le_i32, le_i32, vec3, count(vec3, 3))),
        |(name, type_, bone, org, vectors)| Attachment {
            name: from_fn(|i| name[i]),
            type_,
            bone,
            org,
            vectors: from_fn(|i| vectors[i]),
        },
    )(i)
}

pub fn parse_attachments<'a>(start: &'a [u8], mdl_header: &Header) -> IResult<'a, Vec<Attachment>> {
    count(parse_attachment, mdl_header.num_attachments as usize)(
        &start[mdl_header.attachment_index as usize..],
    )
}
