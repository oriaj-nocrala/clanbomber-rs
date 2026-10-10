//! Pantalla `Show Credits` (ClanBomber2: Credits.cpp): texto que se desplaza.

use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::text_color;
use crate::screen::Flow;

const SCROLL_SPEED: f32 = 400.0;
const NORMAL_SPEED: f32 = 40.0;
const TEXT_HEIGHT: f32 = 40.0;
const OFFSET_START: f32 = 50.0;

const LINES: &[&str] = &[
    "ClanBomber Credits",
    "",
    "Andreas Hundt",
    "Andreeshchev Eugeni",
    "Clanner",
    "Denis Oliver Kropp",
    "Ivar",
    "James Andrews",
    "Magnus Reftel",
    "Martin Pitt",
    "Rene Lopez",
    "mass",
    "non",
    "",
    "ClanBomber Uses",
    "DejaVu",
    "SDL3",
    "SDL3_image",
    "SDL3_mixer",
    "SDL3_ttf",
    "",
    "",
    "Thanks to",
    "Everyone supporting this game...",
    "... and playing it",
];

pub struct Credits {
    offset: f32,
    up: bool,
    down: bool,
}

impl Credits {
    pub fn new() -> Self {
        Credits {
            offset: OFFSET_START,
            up: false,
            down: false,
        }
    }

    fn end() -> f32 {
        -(LINES.len() as f32) * TEXT_HEIGHT + 100.0
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool) -> Flow {
        match code {
            KeyCode::Escape if pressed => return Flow::Back,
            KeyCode::ArrowUp => self.up = pressed,
            KeyCode::ArrowDown => self.down = pressed,
            _ => {}
        }
        Flow::Stay
    }

    /// Arriba/abajo aceleran el desplazamiento; el texto se detiene en los extremos.
    pub fn update(&mut self, dt: f32) {
        let speed = if self.up {
            -SCROLL_SPEED
        } else if self.down {
            SCROLL_SPEED
        } else {
            NORMAL_SPEED
        };
        // `yoffset -= speed * dt`: velocidad positiva = el texto sube.
        self.offset = (self.offset - speed * dt).clamp(Self::end(), OFFSET_START);
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets) {
        let col = text_color();
        r.sprite(
            a.credits_bg,
            [0.0, 0.0, 800.0, 600.0],
            [0.0, 0.0, 800.0, 600.0],
        );
        let y = self.offset;
        r.sprite(
            a.cb_logo,
            [0.0, 0.0, 146.0, 40.0],
            [10.0, y + 10.0, 146.0, 40.0],
        );
        r.sprite(
            a.cb_logo,
            [0.0, 0.0, 146.0, 40.0],
            [640.0, y + 10.0, 146.0, 40.0],
        );
        r.sprite(
            a.fl_logo,
            [0.0, 0.0, 600.0, 106.0],
            [
                100.0,
                y + LINES.len() as f32 * TEXT_HEIGHT + 130.0,
                600.0,
                106.0,
            ],
        );
        for (i, line) in LINES.iter().enumerate() {
            r.text(
                &a.font_big,
                line,
                400.0,
                y + i as f32 * 40.0 + 60.0,
                col,
                Align::Center,
            );
        }
        if y < -10.0 {
            r.text(&a.font_big, "+", 10.0, 9.0, col, Align::Left);
            r.text(&a.font_big, "+", 790.0, 9.0, col, Align::Right);
        }
        if y > Self::end() + 250.0 {
            r.text(&a.font_big, "-", 10.0, 560.0, col, Align::Left);
            r.text(&a.font_big, "-", 790.0, 560.0, col, Align::Right);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_up_by_itself_and_stops_at_both_ends() {
        let mut c = Credits::new();
        c.update(1.0);
        assert!((c.offset - (OFFSET_START - NORMAL_SPEED)).abs() < 0.01);
        c.down = true;
        for _ in 0..100 {
            c.update(0.5);
        }
        assert_eq!(c.offset, Credits::end());
        c.down = false;
        c.up = true;
        for _ in 0..100 {
            c.update(0.5);
        }
        assert_eq!(c.offset, OFFSET_START);
    }
}
