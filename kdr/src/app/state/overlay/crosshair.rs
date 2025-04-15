use crate::app::state::AppState;

impl AppState {
    pub fn crosshair(&mut self, ctx: &egui::Context) {
        let Some((width, height)) = self.window_dimensions() else {
            return;
        };

        const STROKE_LENGTH: f32 = 8.0;
        const STROKE_START: f32 = 4.0;

        // very sad, stroke length has to be 1.5 becuase
        // screensize is usually even number, infact, twice the even
        // so the center is even, that means, we cannot have 1 pixel stroke unless there's some subpixel stuffs
        let stroke = egui::Stroke::new(1.5, egui::Color32::GREEN);
        let center_x = width as f32 / 2.;
        let center_y = height as f32 / 2.;

        let top_stroke = egui::Shape::LineSegment {
            points: [
                [center_x, center_y - STROKE_START].into(), // start
                [center_x, center_y - STROKE_START - STROKE_LENGTH].into(), // end
            ],
            stroke,
        };

        let bottom_stroke = egui::Shape::LineSegment {
            points: [
                [center_x, center_y + STROKE_START].into(), // start
                [center_x, center_y + STROKE_START + STROKE_LENGTH].into(), // end
            ],
            stroke,
        };

        let left_stroke = egui::Shape::LineSegment {
            points: [
                [center_x - STROKE_START, center_y].into(), // start
                [center_x - STROKE_START - STROKE_LENGTH, center_y].into(), // end
            ],
            stroke,
        };

        let right_stroke = egui::Shape::LineSegment {
            points: [
                [center_x + STROKE_START, center_y].into(), // start
                [center_x + STROKE_START + STROKE_LENGTH, center_y].into(), // end
            ],
            stroke,
        };

        egui::Area::new(egui::Id::new("crosshair"))
            .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
            .show(ctx, |ui| {
                let painter = ui.painter();

                painter.add(top_stroke);
                painter.add(bottom_stroke);
                painter.add(left_stroke);
                painter.add(right_stroke);
            });
    }
}
