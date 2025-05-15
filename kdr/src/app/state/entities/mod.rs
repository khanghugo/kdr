use loader::bsp_resource::EntityDictionary;
use viewmodel::ViewModelState;

use super::AppState;

pub mod viewmodel;
mod world;

pub struct EntityState {
    pub entity_dictionary: EntityDictionary,
    pub viewmodel_state: ViewModelState,
}

impl AppState {
    pub(super) fn entity_tick(&mut self) {
        self.world_entity_tick();
        self.viewmodel_tick();
    }
}
