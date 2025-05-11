use cgmath::EuclideanSpace;
use loader::bsp_resource::EntityDictionary;

use super::AppState;

mod viewmodel;

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
            .for_each(|(_, entity)| {
                match &entity.model {
                    // this shouldnt move
                    loader::bsp_resource::EntityModel::Bsp
                    | loader::bsp_resource::EntityModel::NoDrawBrush(_) => {}
                    // for brush entities
                    loader::bsp_resource::EntityModel::OpaqueEntityBrush(_)
                    | loader::bsp_resource::EntityModel::TransparentEntityBrush(_) => {}
                    // sprite
                    loader::bsp_resource::EntityModel::Sprite => todo!(),
                    // studio model entites
                    loader::bsp_resource::EntityModel::Mdl { .. } => {
                        // just update like normal
                        self.render_state.world_buffer[0]
                            .mvp_buffer
                            .update_entity_mvp_buffer(&entity, self.time);
                    }
                    loader::bsp_resource::EntityModel::ViewModel { .. } => {
                        // like mdl but here we will update the world offsets for it
                        let skeletal = entity.transformation.get_skeletal_mut();

                        // move vieworigin z down 1, this seems pretty smart
                        // """pushing the view origin down off of the same X/Z plane as the ent's origin will give the
                        // gun a very nice 'shifting' effect when the player looks up/down. If there is a problem
                        // with view model distortion, this may be a cause. (SJB)."""
                        let view_origin =
                            self.render_state.camera.pos.to_vec() - cgmath::Vector3::unit_z();

                        skeletal.world_transformation =
                            (view_origin, self.render_state.camera.orientation);

                        self.render_state.world_buffer[0]
                            .mvp_buffer
                            .update_entity_mvp_buffer(&entity, self.time);
                    }
                }
            });
    }
}
