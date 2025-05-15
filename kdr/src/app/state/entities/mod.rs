use loader::bsp_resource::EntityDictionary;
use playermodel::PlayerModelState;
use viewmodel::ViewModelState;

use super::AppState;

pub mod playermodel;
pub mod viewmodel;
mod world;

pub struct EntityState {
    pub entity_dictionary: EntityDictionary,
    pub viewmodel_state: ViewModelState,
    pub playermodel_state: PlayerModelState,
}

impl AppState {
    pub(super) fn entity_tick(&mut self) {
        self.world_entity_tick();
        self.viewmodel_tick();
        self.playermodel_tick();
    }
}
