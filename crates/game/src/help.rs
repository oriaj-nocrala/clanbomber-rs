//! Pantalla `Help Screen` (ClanBomber2: `show_tutorial`): dos páginas con los extras y las drogas.

use cb_render::{Align, Renderer};

use crate::assets::Assets;
use crate::draw::{srgb_rgba, text_color};
use crate::screen::Flow;

/// (índice de sprite en extras2.png, título, líneas de explicación).
type Entry = (u32, &'static str, &'static [&'static str]);

const EXTRAS: &[Entry] = &[
    (0, "Bomb:", &["You can place an additional bomb"]),
    (
        1,
        "Power:",
        &["Explosions grow one field in each direction"],
    ),
    (2, "Skateboard:", &["Let's you move faster"]),
    (3, "Kick:", &["Kick bombs if you walk against one"]),
    (
        4,
        "Throw:",
        &[
            "Throw Bombs if you press the button twice",
            "without moving",
        ],
    ),
];

const DRUGS: &[Entry] = &[
    (5, "Joint:", &["Controller will be reversed"]),
    (6, "Viagra:", &["Autofire, this can be very dangerous!"]),
    (7, "Cocaine:", &["Let's you move very fast!! (too fast)"]),
];

pub struct Help {
    page: usize,
}

impl Help {
    pub fn new() -> Self {
        Help { page: 0 }
    }

    /// Cualquier tecla avanza de página y la segunda vuelve al menú.
    pub fn key(&mut self) -> Flow {
        self.page += 1;
        if self.page >= 2 {
            Flow::Back
        } else {
            Flow::Stay
        }
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets) {
        let col = text_color();
        r.fullscreen(a.title_bg);
        r.quad(0.0, 0.0, 800.0, 600.0, srgb_rgba(0, 0, 0, 128));
        let (title, entries) = if self.page == 0 {
            ("ClanBomber Extras", EXTRAS)
        } else {
            ("ClanBomber Drugs", DRUGS)
        };
        let mut y = 25.0;
        r.text(&a.font_big, title, 400.0, y, col, Align::Center);
        y += 80.0;
        for (sprite, name, lines) in entries {
            r.sheet_sprite(&a.extras, *sprite, 15.0, y - 5.0);
            r.text(&a.font_big, name, 70.0, y, col, Align::Left);
            for (k, line) in lines.iter().enumerate() {
                r.text(
                    &a.font_big,
                    line,
                    250.0,
                    y + k as f32 * 40.0,
                    col,
                    Align::Left,
                );
            }
            y += 50.0;
        }
        r.text(
            &a.font_big,
            "Press any key",
            400.0,
            520.0,
            col,
            Align::Center,
        );
    }
}
