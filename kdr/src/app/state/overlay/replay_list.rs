use tracing::warn;

use crate::app::{
    AppEvent,
    state::{AppState, file::SelectedFileType},
};

#[derive(Default)]
pub struct ReplayListUIState {
    pub is_search_enabled: bool,
    pub search_text: String,
    // contains items that are allowed to display
    pub filtered_items: Option<Vec<usize>>,
    should_focus: bool,
}

impl AppState {
    // mimicking map_list.rs
    // make sure they both behave similarly
    pub(super) fn replay_list(&mut self, ctx: &egui::Context) {
        if self.other_resources.replay_list.is_empty() {
            return;
        }

        let (width, height) = self.winit_window_dimensions().unwrap();
        let row_width = width as f32 * 0.12;

        let window_max_height = height as f32 * 0.5;

        let replay_count: usize = self
            .ui_state
            .replay_list
            .filtered_items
            .as_ref()
            .map(|s| s.len())
            .unwrap_or(self.other_resources.replay_list.len());

        let replay_list_id = egui::Id::new("replay list ui");

        egui::Window::new("Replay list")
            .id(replay_list_id)
            .resizable(false)
            .default_open(false)
            .collapsible(true)
            .max_width(row_width)
            .max_height(window_max_height)
            .default_height(window_max_height)
            .show(ctx, |ui| {
                let row_height = ui.text_style_height(&egui::TextStyle::Body);

                ui.input(|input| {
                    if input.key_pressed(egui::Key::F) && input.modifiers.ctrl && input.focused {
                        self.ui_state.replay_list.is_search_enabled =
                            !self.ui_state.replay_list.is_search_enabled;

                        if self.ui_state.replay_list.is_search_enabled {
                            self.ui_state.replay_list.should_focus = true;
                        }
                    }
                });

                egui::ScrollArea::vertical()
                    .drag_to_scroll(false)
                    .auto_shrink(false)
                    .stick_to_right(true)
                    .show_rows(ui, row_height, replay_count, |ui, row_range| {
                        for row in row_range {
                            let replay_name =
                                if let Some(filtered) = &self.ui_state.replay_list.filtered_items {
                                    &self.other_resources.replay_list[filtered[row]]
                                } else {
                                    &self.other_resources.replay_list[row]
                                };

                            let selectable_label = egui::SelectableLabel::new(false, replay_name);

                            if ui.add(selectable_label).clicked() {
                                self.event_loop_proxy
                                    .send_event(AppEvent::RequestReplay(replay_name.to_string()))
                                    .unwrap_or_else(|_| warn!("Failed to send RequestReplay"));

                                self.file_state.selected_file_type = SelectedFileType::Replay;
                                self.file_state.selected_file = replay_name.clone().into();
                            }
                        }
                    });

                // vibe code, holy
                if self.ui_state.replay_list.is_search_enabled {
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
                                        &mut self.ui_state.replay_list.search_text,
                                    )
                                    .hint_text("Search...");

                                    // now some clever shit
                                    let search_box = ui.add(search_box);

                                    if self.ui_state.replay_list.should_focus {
                                        search_box.request_focus();
                                        self.ui_state.replay_list.should_focus = false;
                                    }

                                    if search_box.changed() {
                                        // on changed, we will update our list of filtered item
                                        // since we have a lot of text, we will use index insetad

                                        // clear every time
                                        self.ui_state.replay_list.filtered_items = Some(vec![]);

                                        // for (game_mod_idx, (_, maps)) in
                                        //     self.other_resources.replay_list.iter().enumerate()
                                        // {
                                        //     let mut new_set = HashSet::new();

                                        //     for (map_idx, map) in maps.iter().enumerate() {
                                        //         // case insensitive
                                        //         if map.to_lowercase().contains(
                                        //             &self
                                        //                 .ui_state
                                        //                 .replay_list
                                        //                 .search_text
                                        //                 .to_lowercase(),
                                        //         ) {
                                        //             new_set.insert(map_idx);
                                        //         }
                                        //     }

                                        //     self.ui_state.replay_list.filtered_items.as_mut().map(
                                        //         |what| {
                                        //             what[game_mod_idx] =
                                        //                 new_set.into_iter().collect();
                                        //         },
                                        //     );
                                        // }
                                    };
                                },
                            );
                        },
                    );
                } else {
                    // when it is disable, just discard our hard work
                    self.ui_state.replay_list.filtered_items = None;
                    self.ui_state.replay_list.search_text.clear();
                }
            });
    }
}
