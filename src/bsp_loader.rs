use std::path::Path;

pub struct BspResource {
    pub bsp: bsp::Bsp,
    pub model_entities: Vec<MdlEntity>,
}

#[derive(Clone, Copy)]
pub struct MdlEntityInfo {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub model_view_projection: cgmath::Matrix4<f32>,
}

pub struct MdlEntity {
    pub mdl: mdl::Mdl,
    pub info: MdlEntityInfo,
}

const MODEL_ENTITIES: &[&str] = &["cycler_sprite", "env_sprite"];

// all of the resources to feed to the render context
pub fn get_bsp_resources(bsp: bsp::Bsp, bsp_path: &Path) -> BspResource {
    let mut res = BspResource {
        bsp,
        model_entities: vec![],
    };

    let mut gamedir = bsp_path;

    if let Some(bsp_folder) = bsp_path.parent() {
        if let Some(gamedir_folder) = bsp_folder.parent() {
            gamedir = gamedir_folder;
        } else {
            return res;
        }
    } else {
        return res;
    };

    let mdls: Vec<MdlEntity> = res
        .bsp
        .entities
        .iter()
        .filter_map(|entity| {
            let classname = entity.get("classname")?;

            if MODEL_ENTITIES.contains(&classname.as_str()) {
                let Some(model_path) = entity.get("model") else {
                    return None;
                };

                // env_sprite does load sprite
                if !model_path.ends_with(".mdl") {
                    return None;
                }

                let model_path = gamedir.join(model_path);

                let origin = entity
                    .get("origin")
                    .and_then(|res| vec3(res))
                    .unwrap_or([0., 0., 0.]);

                let angles = entity
                    .get("angles")
                    .and_then(|res| vec3(res))
                    .unwrap_or([0., 0., 0.]);

                let transformation_matrix = cgmath::Matrix4::from_translation(origin.into())
                    * cgmath::Matrix4::from_angle_x(cgmath::Deg(angles[0]))
                    * cgmath::Matrix4::from_angle_y(cgmath::Deg(angles[1]))
                    * cgmath::Matrix4::from_angle_z(cgmath::Deg(angles[2]));

                let model_info = MdlEntityInfo {
                    origin,
                    angles,
                    model_view_projection: transformation_matrix,
                };

                println!("path is {}", model_path.display());
                return mdl::Mdl::open_from_file(model_path)
                    .ok()
                    .map(|mdl| MdlEntity {
                        mdl,
                        info: model_info,
                    });
            }

            None
        })
        .collect();

    res.model_entities = mdls;

    return res;
}

fn vec3(i: &str) -> Option<[f32; 3]> {
    let res: Vec<f32> = i
        .split_whitespace()
        .filter_map(|n| n.parse::<f32>().ok())
        .collect();

    if res.len() < 3 {
        return None;
    }

    Some([res[0], res[1], res[2]])
}
