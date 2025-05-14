use cgmath::{EuclideanSpace, One, Zero};
use loader::bsp_resource::{EntityDictionary, EntityModel};
use tracing::info;
use viewmodel::ViewModelState;

use super::AppState;

pub mod viewmodel;

pub struct EntityState {
    pub entity_dictionary: EntityDictionary,
    pub viewmodel_state: ViewModelState,
}

impl AppState {
    pub(super) fn entity_tick(&mut self) {
        let Some(entity_state) = self.entity_state.as_mut() else {
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
                        // just update like normal
                        self.render_state.world_buffer[0]
                            .mvp_buffer
                            .update_entity_mvp_buffer(&entity, self.time);
                    } // EntityModel::ViewModel {
                      //     model_name, active, ..
                      // } => {
                      //     // like mdl but here we will update the world offsets for it
                      //     let skeletal = entity.transformation.get_skeletal_mut();

                      //     // move vieworigin z down 1, this seems pretty smart
                      //     // """pushing the view origin down off of the same X/Z plane as the ent's origin will give the
                      //     // gun a very nice 'shifting' effect when the player looks up/down. If there is a problem
                      //     // with view model distortion, this may be a cause. (SJB)."""
                      //     let view_origin =
                      //         self.render_state.camera.pos.to_vec() - cgmath::Vector3::unit_z();

                      //     skeletal.world_transformation =
                      //         (view_origin, self.render_state.camera.orientation);

                      //     // zero quaternion to have nothing for the mvp
                      //     if !*active {
                      //         skeletal.world_transformation.1 = cgmath::Quaternion::zero();
                      //     }

                      //     skeletal.world_transformation.1 = cgmath::Quaternion::zero();

                      //     self.render_state.world_buffer[0]
                      //         .mvp_buffer
                      //         .update_entity_mvp_buffer(&entity, entity_state.viewmodel_state.time);

                      //     // need to do this so that the time is guaranteed to hit once
                      //     if *active {
                      //         entity_state.viewmodel_state.time += self.frame_time;
                      //     }
                      // }
                      // EntityModel::PlayerModel {
                      //     model_name,
                      //     submodel,
                      //     player_index,
                      // } => {
                      //     // this should only be responsible for updating player mvps
                      //     // it is up to either replay or puppet to update the entities
                      //     let skeletal = entity.transformation.get_skeletal_mut();

                      //     if player_index.is_none() {
                      //         // if no player, make the model invisible
                      //         skeletal.world_transformation.1 = cgmath::Quaternion::one();
                      //     };

                      //     skeletal.world_transformation.1 = cgmath::Quaternion::one();

                      //     self.render_state.world_buffer[0]
                      //         .mvp_buffer
                      //         .update_entity_mvp_buffer(&entity, self.time);
                      // }
                }
            });
    }
}
