//! Pantalla `Show Credits` (ClanBomber2: Credits.cpp): texto que se desplaza.

use cb_core::{CorpsePart, Rng};
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

/// Lo que ocurrió en un fotograma de los créditos.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Tick {
    /// El texto acaba de llegar a un extremo (`Credits_stop`).
    pub stopped: bool,
    /// Trozos nuevos lanzados (cada uno suena a salpicadura).
    pub splashes: u32,
}

pub struct Credits {
    offset: f32,
    up: bool,
    down: bool,
    stopped: bool,
    /// Sangre: trozos de cadáver que caen cuando se empuja el texto más allá de los extremos.
    parts: Vec<CorpsePart>,
    blood_timer: f32,
    rng: Rng,
}

impl Credits {
    pub fn new(seed: u64) -> Self {
        Credits {
            parts: Vec::new(),
            blood_timer: 0.0,
            rng: Rng::new(seed),
            offset: OFFSET_START,
            up: false,
            down: false,
            // Empieza en el extremo superior sin que suene el tope.
            stopped: true,
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
    /// Avanza `secs` segundos de desplazamiento normal (para las capturas de pantalla).
    pub fn advance(&mut self, secs: f32) {
        for _ in 0..(secs * 60.0) as usize {
            self.update(1.0 / 60.0, true);
        }
    }

    /// `kids_mode` apaga la sangre.
    pub fn update(&mut self, dt: f32, kids_mode: bool) -> Tick {
        let speed = if self.up {
            -SCROLL_SPEED
        } else if self.down {
            SCROLL_SPEED
        } else {
            NORMAL_SPEED
        };
        // `yoffset -= speed * dt`: velocidad positiva = el texto sube.
        let wanted = self.offset - speed * dt;
        let mut tick = Tick::default();

        // Empujar más allá de un extremo hace llover trozos (uno cada 0.04 s).
        let past_top = wanted > OFFSET_START;
        let past_bottom = wanted < Self::end() && speed != NORMAL_SPEED;
        if (past_top || past_bottom) && !kids_mode {
            self.blood_timer += dt;
            while self.blood_timer > 0.04 {
                self.blood_timer -= 0.04;
                let x = self.rng.below(800) as i32 - 60;
                let mut part = CorpsePart::new(x, -40, &mut self.rng);
                let dest_x = self.rng.below(800) as i32 - 60;
                // Arriba caen hasta el borde inferior; abajo, a cualquier punto de la pantalla.
                let dest_y = if past_top {
                    540
                } else {
                    self.rng.below(600) as i32 - 60
                };
                part.fly_to((dest_x, dest_y));
                self.parts.push(part);
                tick.splashes += 1;
            }
        }
        for p in &mut self.parts {
            p.act(dt, None);
        }
        self.parts.retain(|p| !p.delete_me);

        self.offset = wanted.clamp(Self::end(), OFFSET_START);
        let at_end = self.offset <= Self::end() || self.offset >= OFFSET_START;
        tick.stopped = at_end && !self.stopped;
        self.stopped = at_end;
        tick
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets) {
        let col = text_color();
        r.fullscreen(a.credits_bg);
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
        // La sangre va por encima de todo.
        for p in &self.parts {
            let sheet = &a.corpse_parts;
            if p.flight.active {
                let sf = p.flight.scale();
                let (x, y) = (
                    (p.x + 60.0 + 20.0 - sf * 20.0).trunc(),
                    (p.y + 40.0 + 40.0 - sf * 40.0).trunc(),
                );
                r.sprite_tinted(
                    sheet.tex,
                    sheet.cell(p.sprite_nr),
                    [x, y, 40.0 * sf, 40.0 * sf],
                    [1.0; 4],
                );
            } else {
                let (x, y) = ((p.x + 60.0).trunc(), (p.y + 40.0).trunc());
                r.sprite_tinted(
                    sheet.tex,
                    sheet.cell(p.sprite_nr),
                    [x, y, 40.0, 40.0],
                    [1.0, 1.0, 1.0, p.opacity()],
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolls_up_by_itself_and_stops_at_both_ends() {
        let mut c = Credits::new(1);
        c.update(1.0, false);
        assert!((c.offset - (OFFSET_START - NORMAL_SPEED)).abs() < 0.01);
        c.down = true;
        for _ in 0..100 {
            c.update(0.5, false);
        }
        assert_eq!(c.offset, Credits::end());
        c.down = false;
        c.up = true;
        for _ in 0..100 {
            c.update(0.5, false);
        }
        assert_eq!(c.offset, OFFSET_START);
    }

    #[test]
    fn pushing_past_the_top_rains_blood_unless_kids_mode() {
        let mut c = Credits::new(5);
        c.up = true;
        let mut splashes = 0;
        for _ in 0..30 {
            splashes += c.update(1.0 / 60.0, false).splashes;
        }
        assert!(
            splashes >= 10,
            "unos 12 trozos en medio segundo: {splashes}"
        );
        assert!(!c.parts.is_empty());

        let mut kids = Credits::new(5);
        kids.up = true;
        for _ in 0..30 {
            assert_eq!(kids.update(1.0 / 60.0, true).splashes, 0);
        }
        assert!(kids.parts.is_empty());
    }

    #[test]
    fn normal_scrolling_does_not_make_blood() {
        let mut c = Credits::new(5);
        for _ in 0..600 {
            assert_eq!(c.update(1.0 / 60.0, false).splashes, 0);
        }
    }

    #[test]
    fn blood_falls_lands_and_disappears() {
        let mut c = Credits::new(7);
        c.up = true;
        for _ in 0..10 {
            c.update(1.0 / 60.0, false);
        }
        c.up = false;
        for _ in 0..(6.0 * 60.0) as usize {
            c.update(1.0 / 60.0, false);
        }
        assert!(c.parts.is_empty());
    }
}
