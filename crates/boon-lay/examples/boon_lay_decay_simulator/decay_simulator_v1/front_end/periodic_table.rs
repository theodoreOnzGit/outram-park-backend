use boon_lay::Nuclide;
use egui::Color32;
use egui::Widget;
use egui::Vec2;
use egui::Ui;
use egui::Stroke;
use egui::Sense;
use egui::Rect;
use egui::Pos2;
use egui::Painter;

use crate::decay_simulator_v1::DecaySimApp;

impl DecaySimApp {
    /// displays a periodic table of elements
    pub fn periodic_table(&mut self, ui: &mut Ui) {

        let periodic_table_size_x: f32 = 1700.0;
        let periodic_table_size_y: f32 = 1000.0;
        let (rect, _response) = ui.allocate_exact_size(
            egui::vec2(periodic_table_size_x, periodic_table_size_y),
            egui::Sense::hover()
        );

        // vibe coded
        // Layout constants (tweak as you like)
        let origin = rect.min; // top-left of the allocated area
        let tile_w = 80.0;
        let tile_h = 80.0;
        let gap_x = 8.0;
        let gap_y = 12.0;

        let periodic_table_origin = Pos2 {
            x: origin.x + 100.0,
            y: origin.y + 100.0,
        };

        // Helper to compute the top-left position at (group, period), 1-based
        let cell_pos = |group: usize, period: usize| -> egui::Pos2 {
            egui::Pos2::new(
                periodic_table_origin.x + (group as f32 - 1.0) * (tile_w + gap_x),
                periodic_table_origin.y + (period as f32 - 1.0) * (tile_h + gap_y),
            )
        };


        // Period 1
        {
            let p = cell_pos(1, 1);  Self::ui_element_box_at_position(ui, Nuclide::H1,   p.x, p.y);
            let p = cell_pos(18, 1); Self::ui_element_box_at_position(ui, Nuclide::He4,  p.x, p.y);
        }

        // Period 2
        {
            let p = cell_pos(1, 2);  Self::ui_element_box_at_position(ui, Nuclide::Li7,  p.x, p.y);
            let p = cell_pos(2, 2);  Self::ui_element_box_at_position(ui, Nuclide::Be9,  p.x, p.y);
            let p = cell_pos(13, 2); Self::ui_element_box_at_position(ui, Nuclide::B11,  p.x, p.y);
            let p = cell_pos(14, 2); Self::ui_element_box_at_position(ui, Nuclide::C12,  p.x, p.y);
            let p = cell_pos(15, 2); Self::ui_element_box_at_position(ui, Nuclide::N14,  p.x, p.y);
            let p = cell_pos(16, 2); Self::ui_element_box_at_position(ui, Nuclide::O16,  p.x, p.y);
            let p = cell_pos(17, 2); Self::ui_element_box_at_position(ui, Nuclide::F19,  p.x, p.y);
            let p = cell_pos(18, 2); Self::ui_element_box_at_position(ui, Nuclide::Ne20, p.x, p.y);
        }

        // Period 3
        {
            let p = cell_pos(1, 3);  Self::ui_element_box_at_position(ui, Nuclide::Na23, p.x, p.y);
            let p = cell_pos(2, 3);  Self::ui_element_box_at_position(ui, Nuclide::Mg24, p.x, p.y);
            let p = cell_pos(13, 3); Self::ui_element_box_at_position(ui, Nuclide::Al27, p.x, p.y);
            let p = cell_pos(14, 3); Self::ui_element_box_at_position(ui, Nuclide::Si28, p.x, p.y);
            let p = cell_pos(15, 3); Self::ui_element_box_at_position(ui, Nuclide::P31,  p.x, p.y);
            let p = cell_pos(16, 3); Self::ui_element_box_at_position(ui, Nuclide::S32,  p.x, p.y);
            let p = cell_pos(17, 3); Self::ui_element_box_at_position(ui, Nuclide::Cl35, p.x, p.y);
            let p = cell_pos(18, 3); Self::ui_element_box_at_position(ui, Nuclide::Ar40, p.x, p.y);
        }

        // Period 4
        {
            let p = cell_pos(1, 4);  Self::ui_element_box_at_position(ui, Nuclide::K39,  p.x, p.y);
            let p = cell_pos(2, 4);  Self::ui_element_box_at_position(ui, Nuclide::Ca40, p.x, p.y);
            let p = cell_pos(3, 4);  Self::ui_element_box_at_position(ui, Nuclide::Sc45, p.x, p.y);
            let p = cell_pos(4, 4);  Self::ui_element_box_at_position(ui, Nuclide::Ti48, p.x, p.y);
            let p = cell_pos(5, 4);  Self::ui_element_box_at_position(ui, Nuclide::V51,  p.x, p.y);
            let p = cell_pos(6, 4);  Self::ui_element_box_at_position(ui, Nuclide::Cr52, p.x, p.y);
            let p = cell_pos(7, 4);  Self::ui_element_box_at_position(ui, Nuclide::Mn55, p.x, p.y);
            let p = cell_pos(8, 4);  Self::ui_element_box_at_position(ui, Nuclide::Fe56, p.x, p.y);
            let p = cell_pos(9, 4);  Self::ui_element_box_at_position(ui, Nuclide::Co59, p.x, p.y);
            let p = cell_pos(10, 4); Self::ui_element_box_at_position(ui, Nuclide::Ni58, p.x, p.y);
            let p = cell_pos(11, 4); Self::ui_element_box_at_position(ui, Nuclide::Cu63, p.x, p.y);
            let p = cell_pos(12, 4); Self::ui_element_box_at_position(ui, Nuclide::Zn64, p.x, p.y);
            let p = cell_pos(13, 4); Self::ui_element_box_at_position(ui, Nuclide::Ga69, p.x, p.y);
            let p = cell_pos(14, 4); Self::ui_element_box_at_position(ui, Nuclide::Ge74, p.x, p.y);
            let p = cell_pos(15, 4); Self::ui_element_box_at_position(ui, Nuclide::As75, p.x, p.y);
            let p = cell_pos(16, 4); Self::ui_element_box_at_position(ui, Nuclide::Se80, p.x, p.y);
            let p = cell_pos(17, 4); Self::ui_element_box_at_position(ui, Nuclide::Br79, p.x, p.y);
            let p = cell_pos(18, 4); Self::ui_element_box_at_position(ui, Nuclide::Kr84, p.x, p.y);
        }

        // Period 5
        {
            let p = cell_pos(1, 5);  Self::ui_element_box_at_position(ui, Nuclide::Rb85, p.x, p.y);
            let p = cell_pos(2, 5);  Self::ui_element_box_at_position(ui, Nuclide::Sr88, p.x, p.y);
            let p = cell_pos(3, 5);  Self::ui_element_box_at_position(ui, Nuclide::Y89,  p.x, p.y);
            let p = cell_pos(4, 5);  Self::ui_element_box_at_position(ui, Nuclide::Zr90, p.x, p.y);
            let p = cell_pos(5, 5);  Self::ui_element_box_at_position(ui, Nuclide::Nb93, p.x, p.y);
            let p = cell_pos(6, 5);  Self::ui_element_box_at_position(ui, Nuclide::Mo98, p.x, p.y);
            let p = cell_pos(7, 5);  Self::ui_element_box_at_position(ui, Nuclide::Tc98, p.x, p.y);
            let p = cell_pos(8, 5);  Self::ui_element_box_at_position(ui, Nuclide::Ru102,p.x, p.y);
            let p = cell_pos(9, 5);  Self::ui_element_box_at_position(ui, Nuclide::Rh103,p.x, p.y);
            let p = cell_pos(10, 5); Self::ui_element_box_at_position(ui, Nuclide::Pd106,p.x, p.y);
            let p = cell_pos(11, 5); Self::ui_element_box_at_position(ui, Nuclide::Ag107,p.x, p.y);
            let p = cell_pos(12, 5); Self::ui_element_box_at_position(ui, Nuclide::Cd114,p.x, p.y);
            let p = cell_pos(13, 5); Self::ui_element_box_at_position(ui, Nuclide::In115,p.x, p.y);
            let p = cell_pos(14, 5); Self::ui_element_box_at_position(ui, Nuclide::Sn120,p.x, p.y);
            let p = cell_pos(15, 5); Self::ui_element_box_at_position(ui, Nuclide::Sb121,p.x, p.y);
            let p = cell_pos(16, 5); Self::ui_element_box_at_position(ui, Nuclide::Te130,p.x, p.y);
            let p = cell_pos(17, 5); Self::ui_element_box_at_position(ui, Nuclide::I127, p.x, p.y);
            let p = cell_pos(18, 5); Self::ui_element_box_at_position(ui, Nuclide::Xe132,p.x, p.y);
        }


        // Period 6 (main row)
        {
            let p = cell_pos(1, 6);  Self::ui_element_box_at_position(ui, Nuclide::Cs133, p.x, p.y);
            let p = cell_pos(2, 6);  Self::ui_element_box_at_position(ui, Nuclide::Ba138, p.x, p.y);
            let p = cell_pos(3, 6);  Self::ui_element_box_at_position(ui, Nuclide::La139, p.x, p.y);
            let p = cell_pos(4, 6);  Self::ui_element_box_at_position(ui, Nuclide::Hf178, p.x, p.y);
            let p = cell_pos(5, 6);  Self::ui_element_box_at_position(ui, Nuclide::Ta181, p.x, p.y);
            let p = cell_pos(6, 6);  Self::ui_element_box_at_position(ui, Nuclide::W184,  p.x, p.y);
            let p = cell_pos(7, 6);  Self::ui_element_box_at_position(ui, Nuclide::Re185, p.x, p.y);
            let p = cell_pos(8, 6);  Self::ui_element_box_at_position(ui, Nuclide::Os192, p.x, p.y);
            let p = cell_pos(9, 6);  Self::ui_element_box_at_position(ui, Nuclide::Ir193, p.x, p.y);
            let p = cell_pos(10,6);  Self::ui_element_box_at_position(ui, Nuclide::Pt195, p.x, p.y);
            let p = cell_pos(11,6);  Self::ui_element_box_at_position(ui, Nuclide::Au197, p.x, p.y);
            let p = cell_pos(12,6);  Self::ui_element_box_at_position(ui, Nuclide::Hg202, p.x, p.y);
            let p = cell_pos(13,6);  Self::ui_element_box_at_position(ui, Nuclide::Tl205, p.x, p.y);
            let p = cell_pos(14,6);  Self::ui_element_box_at_position(ui, Nuclide::Pb208, p.x, p.y);
            let p = cell_pos(15,6);  Self::ui_element_box_at_position(ui, Nuclide::Bi209, p.x, p.y);
            let p = cell_pos(16,6);  Self::ui_element_box_at_position(ui, Nuclide::Po209, p.x, p.y);
            let p = cell_pos(17,6);  Self::ui_element_box_at_position(ui, Nuclide::At210, p.x, p.y);
            let p = cell_pos(18,6);  Self::ui_element_box_at_position(ui, Nuclide::Rn222, p.x, p.y);
        }

        // f-block: Lanthanides (Ce–Lu), rendered on an extra row below the main table
        {
            let f_start_group = 3usize;
            let lanth_period = 8usize;
            let p = cell_pos(f_start_group + 0,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Ce140, p.x, p.y);
            let p = cell_pos(f_start_group + 1,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Pr141, p.x, p.y);
            let p = cell_pos(f_start_group + 2,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Nd142, p.x, p.y);
            let p = cell_pos(f_start_group + 3,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Pm145, p.x, p.y);
            let p = cell_pos(f_start_group + 4,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Sm152, p.x, p.y);
            let p = cell_pos(f_start_group + 5,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Eu153, p.x, p.y);
            let p = cell_pos(f_start_group + 6,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Gd158, p.x, p.y);
            let p = cell_pos(f_start_group + 7,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Tb159, p.x, p.y);
            let p = cell_pos(f_start_group + 8,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Dy164, p.x, p.y);
            let p = cell_pos(f_start_group + 9,  lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Ho165, p.x, p.y);
            let p = cell_pos(f_start_group + 10, lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Er166, p.x, p.y);
            let p = cell_pos(f_start_group + 11, lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Tm169, p.x, p.y);
            let p = cell_pos(f_start_group + 12, lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Yb174, p.x, p.y);
            let p = cell_pos(f_start_group + 13, lanth_period); Self::ui_element_box_at_position(ui, Nuclide::Lu175, p.x, p.y);
        }

        // Period 7 (main row)
        {
            let p = cell_pos(1, 7);  Self::ui_element_box_at_position(ui, Nuclide::Fr223, p.x, p.y);
            let p = cell_pos(2, 7);  Self::ui_element_box_at_position(ui, Nuclide::Ra226, p.x, p.y);
            let p = cell_pos(3, 7);  Self::ui_element_box_at_position(ui, Nuclide::Ac227, p.x, p.y);
            let p = cell_pos(4, 7);  Self::ui_element_box_at_position(ui, Nuclide::Rf267, p.x, p.y);
            let p = cell_pos(5, 7);  Self::ui_element_box_at_position(ui, Nuclide::Db268, p.x, p.y);
            let p = cell_pos(6, 7);  Self::ui_element_box_at_position(ui, Nuclide::Sg271, p.x, p.y);
            let p = cell_pos(7, 7);  Self::ui_element_box_at_position(ui, Nuclide::Bh270, p.x, p.y);
            let p = cell_pos(8, 7);  Self::ui_element_box_at_position(ui, Nuclide::Hs270, p.x, p.y);
            let p = cell_pos(9, 7);  Self::ui_element_box_at_position(ui, Nuclide::Mt278, p.x, p.y);
            let p = cell_pos(10,7);  Self::ui_element_box_at_position(ui, Nuclide::Ds281, p.x, p.y);
            let p = cell_pos(11,7);  Self::ui_element_box_at_position(ui, Nuclide::Rg282, p.x, p.y);
            let p = cell_pos(12,7);  Self::ui_element_box_at_position(ui, Nuclide::Cn285, p.x, p.y);
            let p = cell_pos(13,7);  Self::ui_element_box_at_position(ui, Nuclide::Nh286, p.x, p.y);
            let p = cell_pos(14,7);  Self::ui_element_box_at_position(ui, Nuclide::Fl289, p.x, p.y);
            let p = cell_pos(15,7);  Self::ui_element_box_at_position(ui, Nuclide::Mc290, p.x, p.y);
            let p = cell_pos(16,7);  Self::ui_element_box_at_position(ui, Nuclide::Lv293, p.x, p.y);
            let p = cell_pos(17,7);  Self::ui_element_box_at_position(ui, Nuclide::Ts294, p.x, p.y);
            let p = cell_pos(18,7);  Self::ui_element_box_at_position(ui, Nuclide::Og294, p.x, p.y);
        }
        // actinides
        {
            let f_start_group = 3usize;
            let actin_period = 9usize;
            let p = cell_pos(f_start_group + 0,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Th232, p.x, p.y);
            let p = cell_pos(f_start_group + 1,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Pa231, p.x, p.y);
            let p = cell_pos(f_start_group + 2,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::U238,  p.x, p.y);
            let p = cell_pos(f_start_group + 3,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Np237, p.x, p.y);
            let p = cell_pos(f_start_group + 4,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Pu244, p.x, p.y);
            let p = cell_pos(f_start_group + 5,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Am243, p.x, p.y);
            let p = cell_pos(f_start_group + 6,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Cm247, p.x, p.y);
            let p = cell_pos(f_start_group + 7,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Bk247, p.x, p.y);
            let p = cell_pos(f_start_group + 8,  actin_period); Self::ui_element_box_at_position(ui, Nuclide::Cf251, p.x, p.y);
            let p = cell_pos(f_start_group + 9,  actin_period);  Self::ui_element_box_at_position(ui, Nuclide::Es252, p.x, p.y);
            let p = cell_pos(f_start_group + 10, actin_period); Self::ui_element_box_at_position(ui, Nuclide::Fm257, p.x, p.y);
            let p = cell_pos(f_start_group + 11, actin_period); Self::ui_element_box_at_position(ui, Nuclide::Md258, p.x, p.y);
            let p = cell_pos(f_start_group + 12, actin_period); Self::ui_element_box_at_position(ui, Nuclide::No259, p.x, p.y);
            let p = cell_pos(f_start_group + 13, actin_period); Self::ui_element_box_at_position(ui, Nuclide::Lr262, p.x, p.y);
        }

        // Superheavy elements (Period 7, groups 4–18)
        {
            let p = cell_pos(4, 7);  Self::ui_element_box_at_position(ui, Nuclide::Rf267, p.x, p.y);
            let p = cell_pos(5, 7);  Self::ui_element_box_at_position(ui, Nuclide::Db268, p.x, p.y);
            let p = cell_pos(6, 7);  Self::ui_element_box_at_position(ui, Nuclide::Sg271, p.x, p.y);
            let p = cell_pos(7, 7);  Self::ui_element_box_at_position(ui, Nuclide::Bh270, p.x, p.y);
            let p = cell_pos(8, 7);  Self::ui_element_box_at_position(ui, Nuclide::Hs270, p.x, p.y);
            let p = cell_pos(9, 7);  Self::ui_element_box_at_position(ui, Nuclide::Mt278, p.x, p.y);
            let p = cell_pos(10,7);  Self::ui_element_box_at_position(ui, Nuclide::Ds281, p.x, p.y);
            let p = cell_pos(11,7);  Self::ui_element_box_at_position(ui, Nuclide::Rg282, p.x, p.y);
            let p = cell_pos(12,7);  Self::ui_element_box_at_position(ui, Nuclide::Cn285, p.x, p.y);
            let p = cell_pos(13,7);  Self::ui_element_box_at_position(ui, Nuclide::Nh286, p.x, p.y);
            let p = cell_pos(14,7);  Self::ui_element_box_at_position(ui, Nuclide::Fl289, p.x, p.y);
            let p = cell_pos(15,7);  Self::ui_element_box_at_position(ui, Nuclide::Mc290, p.x, p.y);
            let p = cell_pos(16,7);  Self::ui_element_box_at_position(ui, Nuclide::Lv293, p.x, p.y);
            let p = cell_pos(17,7);  Self::ui_element_box_at_position(ui, Nuclide::Ts294, p.x, p.y);
            let p = cell_pos(18,7);  Self::ui_element_box_at_position(ui, Nuclide::Og294, p.x, p.y);
        }

    }

    /// Simple contrast helper: choose black or white text based on fill.
    fn contrasting_text(fill: Color32) -> Color32 {
        let r = fill.r() as f32 / 255.0;
        let g = fill.g() as f32 / 255.0;
        let b = fill.b() as f32 / 255.0;
        let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        if luminance > 0.6 { Color32::BLACK } else { Color32::WHITE }
    }

    fn ui_element_box_at_position(ui: &mut egui::Ui, nuclide: Nuclide,
        x_pixel: f32,
        y_pixel: f32) {
        let size = Vec2::new(80.0, 80.0);
        let pos = Pos2::new(x_pixel, y_pixel);
        Self::draw_element_box_at(ui, nuclide, pos, size);
    }

    pub fn draw_element_box_at(
        ui: &mut egui::Ui,
        nuclide: Nuclide,
        pos: Pos2,
        size: Vec2,
    ) {
        let centre_x_pixels = pos.x;
        let centre_y_pixels = pos.y;
        let x_width_pixels = size.x;
        let y_width_pixels = size.y;
        let element_box: ElementBox = nuclide.clone().into();
        Self::put_widget_with_size_and_centre(ui,
            element_box,
            centre_x_pixels,
            centre_y_pixels,
            x_width_pixels,
            y_width_pixels
        );
    }

    pub fn symbol_from_z(z: u32) -> &'static str {
        static SYMBOLS: [&str; 119] = [
            "",
            "H","He","Li","Be","B","C","N","O","F","Ne",
            "Na","Mg","Al","Si","P","S","Cl","Ar",
            "K","Ca","Sc","Ti","V","Cr","Mn","Fe","Co","Ni","Cu","Zn",
            "Ga","Ge","As","Se","Br","Kr",
            "Rb","Sr","Y","Zr","Nb","Mo","Tc","Ru","Rh","Pd","Ag","Cd",
            "In","Sn","Sb","Te","I","Xe",
            "Cs","Ba","La","Ce","Pr","Nd","Pm","Sm","Eu","Gd","Tb","Dy","Ho","Er","Tm","Yb","Lu",
            "Hf","Ta","W","Re","Os","Ir","Pt","Au","Hg",
            "Tl","Pb","Bi","Po","At","Rn",
            "Fr","Ra","Ac","Th","Pa","U","Np","Pu","Am","Cm","Bk","Cf","Es","Fm","Md","No","Lr",
            "Rf","Db","Sg","Bh","Hs","Mt","Ds","Rg","Cn",
            "Nh","Fl","Mc","Lv","Ts","Og",
        ];
        if z <= 118 { SYMBOLS[z as usize] } else { "?" }
    }
    pub fn put_widget_with_size_and_centre(ui: &mut Ui, widget: impl Widget,
        centre_x_pixels: f32,
        centre_y_pixels: f32,
        x_width_pixels: f32,
        y_width_pixels: f32){

        let top_left_x: f32 = centre_x_pixels - 0.5 * x_width_pixels;
        let top_left_y: f32 = centre_y_pixels - 0.5 * y_width_pixels;
        let bottom_right_x: f32 = centre_x_pixels + 0.5 * x_width_pixels;
        let bottom_right_y: f32 = centre_y_pixels + 0.5 * y_width_pixels;

        let rect: Rect = Rect {
            min: Pos2 { x: top_left_x, y: top_left_y },
            max: Pos2 { x: bottom_right_x, y: bottom_right_y },
        };

        ui.put(rect, widget);
    }
}

pub struct ElementBox {
    pub nuclide: Nuclide,
    pub size: Vec2,
}

impl From<Nuclide> for ElementBox {
    fn from(nuclide: Nuclide) -> Self {
        let size = Vec2::new(80.0, 80.0);
        ElementBox { nuclide, size }
    }
}

impl Widget for ElementBox {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = self.size;
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        let color = DecaySimApp::element_color(self.nuclide);
        let (z, _a) = self.nuclide.get_z_a();

        let rounding = egui::CornerRadius::same(8);

        let painter: Painter = ui.painter().clone();
        let bg_stroke = Stroke::new(1.5, color.gamma_multiply(0.8));
        painter.rect(rect, rounding, color, bg_stroke, egui::epaint::StrokeKind::Middle);

        let text_color = DecaySimApp::contrasting_text(color);

        let padding = 6.0;
        let small_text_size = (desired_size.y * 0.16).clamp(10.0, 16.0);
        let symbol_text_size = (desired_size.y * 0.48).clamp(18.0, 40.0);

        let top_left = Pos2::new(rect.left() + padding, rect.top() + padding);

        {
            let galley = painter.layout_no_wrap(
                format!("{}", z),
                egui::FontId::proportional(small_text_size),
                text_color,
            );
            painter.galley(top_left, galley, text_color);
        }

        {
            let symbol = DecaySimApp::symbol_from_z(z);
            let galley = painter.layout_no_wrap(
                symbol.to_string(),
                egui::FontId::proportional(symbol_text_size),
                text_color,
            );
            let center = rect.center();
            let galley_pos = Pos2::new(
                center.x - galley.size().x / 2.0,
                center.y - galley.size().y / 2.0,
            );
            painter.galley(galley_pos, galley, text_color);
        }

        response
    }
}
