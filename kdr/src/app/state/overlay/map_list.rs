use std::collections::HashSet;

use loader::MapIdentifier;
use tracing::warn;

use crate::app::{
    AppEvent,
    state::{AppState, file::SelectedFileType},
};

#[derive(Default)]
pub struct MapListUIState {
    pub is_search_enabled: bool,
    pub search_text: String,
    // contains items that are allowed to display
    // the vector length matches self.other_resource.map_list.len()
    // [[map list]; mod count]
    pub filtered_items: Option<Vec<Vec<usize>>>,
    should_focus: bool,
}

impl AppState {
    pub fn map_list(&mut self, ctx: &egui::Context) {
        if self.other_resources.map_list.is_empty() {
            return;
        }

        let (width, height) = self.window_dimensions().unwrap();
        let row_width = width as f32 * 0.12;

        let look_up_table: Vec<usize> = if self.ui_state.map_list.is_search_enabled
            && self.ui_state.map_list.filtered_items.is_some()
        {
            self.ui_state
                .map_list
                .filtered_items
                .as_ref()
                .unwrap()
                .iter()
                .map(|a| a.len())
                .collect()
        } else {
            self.other_resources
                .map_list
                .iter()
                .map(|(_, a)| a.len())
                .collect()
        };

        let window_max_height = height as f32 * 0.5;

        let look_up = |row: usize| {
            let mut remaining = row;
            for (game_mod_idx, &count) in look_up_table.iter().enumerate() {
                if remaining < count {
                    return (game_mod_idx, remaining);
                }
                remaining -= count;
            }
            unreachable!("indexing out of bound");
        };

        let map_count: usize = look_up_table.iter().sum();

        let map_list_id = egui::Id::new("Map list");

        egui::Window::new("Map list")
            .id(map_list_id)
            .resizable(false)
            .default_open(false)
            .collapsible(true)
            .max_width(row_width)
            .max_height(window_max_height)
            .default_height(window_max_height)
            .show(ctx, |ui| {
                // need this row height to match the label height
                // otherwise, the scroll area won't properly render all entries
                let row_height = ui.text_style_height(&egui::TextStyle::Body);

                // cant contain it inside the windwo
                ui.input(|input| {
                    if input.key_pressed(egui::Key::F) && input.modifiers.ctrl && input.focused {
                        self.ui_state.map_list.is_search_enabled =
                            !self.ui_state.map_list.is_search_enabled;

                        if self.ui_state.map_list.is_search_enabled {
                            self.ui_state.map_list.should_focus = true;
                        }
                    }
                });

                egui::ScrollArea::vertical()
                    .drag_to_scroll(false)
                    .auto_shrink(false)
                    .stick_to_right(true)
                    .show_rows(ui, row_height, map_count, |ui, row_range| {
                        for row in row_range {
                            let (game_mod_idx, what_idx_is_this) = look_up(row);
                            let (game_mod, v) = &self.other_resources.map_list[game_mod_idx];

                            let map_name = if self.ui_state.map_list.filtered_items.is_none() {
                                &v[what_idx_is_this]
                            } else {
                                let map_name_idx =
                                    self.ui_state.map_list.filtered_items.as_ref().unwrap()
                                        [game_mod_idx][what_idx_is_this];
                                &v[map_name_idx]
                            };

                            let selectable_label = egui::SelectableLabel::new(false, map_name);

                            if ui.add(selectable_label).clicked() {
                                let identifier = MapIdentifier {
                                    map_name: map_name.to_string(),
                                    game_mod: game_mod.to_string(),
                                };

                                self.event_loop_proxy
                                    .send_event(AppEvent::RequestMap(identifier))
                                    .unwrap_or_else(|_| warn!("Cannot send RequestResource"));

                                // auxillary stuffs
                                self.file_state.selected_file_type = SelectedFileType::Bsp;
                                self.file_state.selected_file = map_name.clone().into();
                            }
                        }
                    });

                // vibe code, holy
                if self.ui_state.map_list.is_search_enabled {
                    ui.allocate_ui_with_layout(
                        egui::Vec2::new(
                            ui.available_width(),
                            ui.text_style_height(&egui::TextStyle::Body) * 1.5,
                        ),
                        egui::Layout::bottom_up(egui::Align::LEFT),
                        |ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let search_box = egui::TextEdit::singleline(
                                        &mut self.ui_state.map_list.search_text,
                                    )
                                    .hint_text("Search...");

                                    // now some clever shit
                                    let search_box = ui.add(search_box);

                                    if self.ui_state.map_list.should_focus {
                                        search_box.request_focus();
                                        self.ui_state.map_list.should_focus = false;
                                    }

                                    if search_box.changed() {
                                        // on changed, we will update our list of filtered item
                                        // since we have a lot of text, we will use index insetad

                                        // clear every time
                                        self.ui_state.map_list.filtered_items =
                                            Some(vec![vec![]; self.other_resources.map_list.len()]);

                                        for (game_mod_idx, (_, maps)) in
                                            self.other_resources.map_list.iter().enumerate()
                                        {
                                            let mut new_set = HashSet::new();

                                            for (map_idx, map) in maps.iter().enumerate() {
                                                // case insensitive
                                                if map.to_lowercase().contains(
                                                    &self
                                                        .ui_state
                                                        .map_list
                                                        .search_text
                                                        .to_lowercase(),
                                                ) {
                                                    new_set.insert(map_idx);
                                                }
                                            }

                                            self.ui_state.map_list.filtered_items.as_mut().map(
                                                |what| {
                                                    what[game_mod_idx] =
                                                        new_set.into_iter().collect();
                                                },
                                            );
                                        }
                                    };
                                },
                            );
                        },
                    );
                } else {
                    // when it is disable, just discard our hard work
                    self.ui_state.map_list.filtered_items = None;
                    self.ui_state.map_list.search_text.clear();
                }
            });
    }
}
