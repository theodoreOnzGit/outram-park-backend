
use std::f32::consts::TAU;

use egui::{include_image, vec2, Color32, Frame, Painter, Pos2, Rect, Sense, Stroke, TextStyle, Ui, Vec2};
use egui_extras::{Size, StripBuilder};

use crate::ciet_simulator_v1::CIETApp;

impl CIETApp {

    pub fn ciet_sim_heater_page(&mut self, ui: &mut Ui){



        // this seems to be a decent way to place images at specific places
        let rect: Rect = Rect {
            // top left
            min: Pos2 { x: 350.5, y: 350.5 },
            // bottom right
            max: Pos2 { x: 500.5, y: 500.5 },
        };
        //let rect = egui::Rect::from_min_size(Default::default(), egui::Vec2::splat(100.0));
        let _ferris = egui::Image::new(include_image!("../../ferris.png"))
            .rounding(5.0)
            .paint_at(ui, rect);

    }

}
