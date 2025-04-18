use ghost::GhostFrameText;

use crate::app::state::AppState;

#[derive(Default, Debug, Clone)]
pub struct TextState {
    /// Entity text from SVC Temp Entity (23) Text Message (29)
    ///
    /// ~~Key: Text channel~~
    ///
    /// ~~Value: (Ghost frame index, Frame text)~~
    ///
    /// ~~We need the channel to correctly erase another text like how the game does.~~ And we need the ghost frame index
    /// to correctly create an egui Id for the widget.
    ///
    /// UPDATE: I am in favor of not doing text erasure becuase of channel. This seems like a bug from the plugin side and I will not be complicit.
    /// This means, a vector is used instead of a hash map to remove the limit of text channel.
    ///
    /// (Frame Index, Ghost Frame Text)
    pub entity_text: Vec<(usize, GhostFrameText)>,
}

impl AppState {
    pub fn draw_entity_text(&mut self, ctx: &egui::Context) {
        let Some((width, height)) = self.window_dimensions() else {
            return;
        };

        self.text_state
            .entity_text
            .iter()
            .for_each(|(frame_idx, text)| {
                egui::Area::new(
                    // need to use two values for hash so that we have a truly unique area
                    egui::Id::new((text.channel, frame_idx)),
                )
                // this is to make sure that the text is in the center top of the new area
                // this means the text will be able to centered nicely
                .pivot(egui::Align2::CENTER_TOP)
                // even though rect is kind of flexing, why not
                .constrain(false)
                .fixed_pos([
                    text.location[0] * width as f32,
                    text.location[1] * height as f32,
                ])
                .show(ctx, |ui| {
                    let styled_text = egui::RichText::new(&text.text)
                        .color(egui::Color32::WHITE)
                        .strong()
                        .size(14.0);

                    let main_text = egui::Label::new(styled_text).selectable(false);

                    ui.add(main_text);
                });
            });
    }
    // what it does here is to decay the text life and then remove them accordingly
    pub fn text_tick(&mut self) {
        self.text_state
            .entity_text
            .retain(|(_, text)| text.life > self.time);
    }
}
