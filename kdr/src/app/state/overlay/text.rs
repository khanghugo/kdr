use ghost::{GhostFrameEntityText, GhostFrameSayText};

use crate::app::state::AppState;

pub const MAX_SAY_TEXT: usize = 5;
pub const SAY_TEXT_LIFE: f32 = 5.;

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
    /// (Frame Index, Ghost Frame Entity Text)
    pub entity_text: Vec<(usize, GhostFrameEntityText)>,
    /// (Life, Ghost Frame Say Text)
    ///
    /// FIFO structure. If text is 0, it will be the first one to disappear or removed when new text comes in
    pub say_text: Vec<(f32, GhostFrameSayText)>,
}

impl TextState {
    pub fn clear_text(&mut self) {
        self.entity_text.clear();
        self.say_text.clear();
    }
}

const COLOR_BLUE: [u8; 3] = [153, 204, 255];
const COLOR_RED: [u8; 3] = [255, 64, 64];
const COLOR_YELLOW: [u8; 3] = [255, 179, 0];
const COLOR_GREEN: [u8; 3] = [153, 255, 153];
const COLOR_GREY: [u8; 3] = [204, 204, 204];

const fn color_lookup(i: u8) -> [u8; 3] {
    match i {
        1 => COLOR_BLUE,
        2 => COLOR_RED,
        3 => COLOR_YELLOW,
        4 => COLOR_GREEN,
        0 => COLOR_YELLOW,
        _ => COLOR_GREY,
    }
}

impl AppState {
    pub(super) fn draw_entity_text(&mut self, ctx: &egui::Context) {
        let Some((width, height)) = self.egui_window_dimensions(ctx) else {
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
                    ui.vertical_centered_justified(|ui| {
                        let styled_text = egui::RichText::new(&text.text)
                            .color(egui::Color32::from_rgba_premultiplied(
                                (text.color[0] * 255.) as u8,
                                (text.color[1] * 255.) as u8,
                                (text.color[2] * 255.) as u8,
                                (text.color[3] * 255.) as u8,
                            ))
                            .font(egui::FontId::new(
                                14.,
                                egui::FontFamily::Name("verdana".into()),
                            ));

                        let main_text = egui::Label::new(styled_text).selectable(false);

                        ui.add(main_text);
                    });
                });
            });
    }

    pub(super) fn draw_say_text(&mut self, ctx: &egui::Context) {
        if self.text_state.say_text.is_empty() {
            return;
        }

        let Some((width, height)) = self.egui_window_dimensions(ctx) else {
            return;
        };

        let content_width = width as f32 * 0.75;

        egui::Area::new(egui::Id::new("say text area"))
            .default_size([content_width, height as f32 * 0.25])
            .anchor(
                egui::Align2::LEFT_TOP,
                [width as f32 * 0.025, (height as f32) * 0.7],
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.set_width(content_width);

                    self.text_state.say_text.iter().for_each(|(_, say_text)| {
                        // need to be on one line
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                            say_text.text.iter().for_each(|(header, text)| {
                                let [r, g, b] = color_lookup(*header);

                                let rich_text = egui::RichText::new(text)
                                    .strong()
                                    .color(egui::Color32::from_rgb(r, g, b))
                                    .font(egui::FontId::new(
                                        18.,
                                        egui::FontFamily::Name("tahoma".into()),
                                    ));

                                let say_text_label = egui::Label::new(rich_text).selectable(false);

                                ui.add(say_text_label);
                            });
                        });
                    });
                });
            });
    }

    // what it does here is to decay the text life and then remove them accordingly
    pub fn text_tick(&mut self) {
        self.text_state
            .entity_text
            .retain(|(_, text)| text.life > self.time);

        self.text_state
            .say_text
            .retain(|&(life, _)| life > self.time);
    }
}
