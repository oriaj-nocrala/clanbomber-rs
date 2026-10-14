//! Pantalla `Help Screen` (ClanBomber2: `show_tutorial`): dos páginas con los extras y las drogas,
//! en el letrero de madera sobre el tablero del menú.

use cb_render::{Align, Renderer};

use crate::assets::Assets;
use crate::menu_sign::{self as sign, Front, Tone};
use crate::screen::Flow;

/// Tamaño del letrero.
pub const PANEL: [f32; 2] = [620.0, 520.0];

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

    /// La página que se ve (0 o 1): cada una entra girando en el letrero.
    pub fn page(&self) -> usize {
        self.page
    }

    /// El contenido del letrero ya de frente (`menu_sign::Front`).
    pub fn draw(&self, r: &mut Renderer, a: &Assets, front: &Front) {
        let alpha = front.alpha;
        let [left, top, width, height] = sign::inner(front.rect);
        let (big, small) = (&a.font_big, &a.font_small);
        let (fh, sh) = (big.line_height().round(), small.line_height().round());
        let cx = left + width / 2.0;
        let (title, entries) = if self.page == 0 {
            ("ClanBomber Extras", EXTRAS)
        } else {
            ("ClanBomber Drugs", DRUGS)
        };
        sign::title(r, big, title, cx, top - 6.0, alpha);

        // Cada entrada: el extra, su nombre en dorado y la explicación debajo.
        let icon = fh * 1.5;
        let text_x = left + icon + 20.0;
        let mut y = top + fh * 1.6;
        for (sprite, name, lines) in entries {
            r.sprite_tinted(
                a.extras.tex,
                a.extras.cell(*sprite),
                [left + 4.0, y, icon, icon],
                [1.0, 1.0, 1.0, alpha],
            );
            let name = name.trim_end_matches(':');
            sign::label(
                r,
                big,
                name,
                text_x,
                y - 4.0,
                Align::Left,
                Tone::Selected,
                alpha,
            );
            for (k, line) in lines.iter().enumerate() {
                let ly = y + fh - 2.0 + k as f32 * sh;
                sign::label(r, small, line, text_x, ly, Align::Left, Tone::Normal, alpha);
            }
            y += fh + lines.len() as f32 * sh + 14.0;
        }

        let hint = format!("PRESS ANY KEY  ·  {}/2", self.page + 1);
        let hint_y = top + height - sh;
        sign::label(r, small, &hint, cx, hint_y, Align::Center, Tone::Dim, alpha);
    }
}
