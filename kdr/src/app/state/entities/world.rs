use loader::bsp_resource::EntityModel;

use crate::app::state::AppState;

impl AppState {
    pub(super) fn world_entity_tick(&mut self) {
        let Some(entity_state) = self.entity_state.as_mut() else {
            return;
        };

        let Some(world_buffer) = self.render_state.world_buffer.as_ref() else {
            return;
        };

        entity_state
            .entity_dictionary
            .iter_mut()
            .for_each(|(_, entity)| {
                match &entity.model {
                    // this shouldnt move
                    EntityModel::Bsp | EntityModel::NoDrawBrush(_) => {}
                    // for brush entities
                    EntityModel::OpaqueEntityBrush(_) | EntityModel::TransparentEntityBrush(_) => {}
                    // sprite
                    EntityModel::Sprite => todo!(),
                    // studio model entites
                    EntityModel::BspMdlEntity { .. } => {
                        let skeletal_transformation = entity.transformation.get_skeletal_mut();

                        // only update when we have more than 1 frames
                        if skeletal_transformation.model_transformations
                            [skeletal_transformation.current_sequence_index] // sequence
                            [0] // blend 1
                        // now we have all frames
                        .len()
                            > 1
                        {
                            let mvps = skeletal_transformation.build_mvp(self.time);

                            // bone 0
                            world_buffer
                                .mvp_buffer
                                .update_mvp_buffer(mvps[0], entity.world_index);

                            // bone rest
                            if let Some(mvp_index_start) =
                                world_buffer.mvp_lookup.get(&entity.world_index)
                            {
                                world_buffer
                                    .mvp_buffer
                                    .update_mvp_buffer_many(mvps[1..].to_vec(), *mvp_index_start);
                            }
                        }
                    }
                }
            });
    }
}
