use loader::bsp_resource::{EntityDictionary, WorldTransformationType};

use super::AppState;

pub struct EntityState {
    pub entity_dictionary: EntityDictionary,
}

impl AppState {
    pub(super) fn entity_tick(&mut self) {
        let Some(entity_state) = self.entity_state.as_mut() else {
            return;
        };

        entity_state
            .entity_dictionary
            .iter_mut()
            .for_each(|(&bsp_idx, entity)| {
                if bsp_idx != 8 {
                    return;
                }

                match &mut entity.transformation {
                    WorldTransformationType::Entity(world_transformation) => {
                        self.render_state.world_buffer[0]
                            .mvp_buffer
                            .update_entity_mvp_buffer(&entity, self.time);
                    }
                    WorldTransformationType::Skeletal {
                        current_sequence_index,
                        world_transformation,
                        model_transformations,
                        model_transformation_infos,
                    } => {}
                };

                // let mvp = entity.build_mvp()
            });
    }
}
