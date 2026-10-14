//! Intro (ClanBomber2: `run_intro`): negro, el logo de Fischlustig aparece poco a poco y la
//! leyenda se escribe letra a letra con retardos al azar.

use cb_core::Rng;
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::audio::Sfx;
use crate::draw::text_color;
use crate::screen::Flow;

const BLACK_TIME: f32 = 2.0;
/// El alfa sube 130 de 255 por segundo.
const FADE_SPEED: f32 = 130.0;
const LEGEND: &str = "A WORLD DOMINATION PROJECT";

pub struct Intro {
    time: f32,
    alpha: f32,
    /// Letras de la leyenda ya escritas.
    typed: usize,
    /// Segundos hasta la siguiente letra.
    wait: f32,
    rng: Rng,
    finished: bool,
    /// Sonido pendiente de reproducir (lo recoge quien llama).
    pub sound: Option<Sfx>,
    fade_started: bool,
}

impl Intro {
    pub fn new(seed: u64) -> Self {
        Intro {
            time: 0.0,
            alpha: 0.0,
            typed: 0,
            wait: 0.0,
            rng: Rng::new(seed),
            finished: false,
            sound: None,
            fade_started: false,
        }
    }

    /// Esc, Enter o espacio se saltan la intro.
    pub fn key(&mut self, code: KeyCode) -> Flow {
        if matches!(code, KeyCode::Escape | KeyCode::Enter | KeyCode::Space) {
            self.finished = true;
        }
        self.flow()
    }

    fn flow(&self) -> Flow {
        if self.finished {
            Flow::Back
        } else {
            Flow::Stay
        }
    }

    /// Avanza `dt` segundos; devuelve `Back` cuando la intro termina.
    pub fn update(&mut self, dt: f32) -> Flow {
        self.time += dt;
        if self.time < BLACK_TIME {
            return self.flow();
        }
        if !self.fade_started {
            // `Intro_winlevel` suena al empezar a aparecer el logo.
            self.fade_started = true;
            self.sound = Some(Sfx::WinLevel);
        }
        if self.alpha < 255.0 {
            self.alpha = (self.alpha + dt * FADE_SPEED).min(255.0);
            return self.flow();
        }
        // Cada letra tarda entre 80 y 180 ms (`fast_random() % 100000 + 80000` µs).
        self.wait -= dt;
        while self.wait <= 0.0 && !self.finished {
            if self.typed >= LEGEND.chars().count() {
                self.finished = true;
                self.sound = Some(Sfx::MenuBack);
                break;
            }
            if LEGEND.chars().nth(self.typed) != Some(' ') {
                self.sound = Some(Sfx::Typewriter);
            }
            self.typed += 1;
            self.wait += (self.rng.below(100_000) + 80_000) as f32 / 1_000_000.0;
        }
        self.flow()
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets) {
        if self.time < BLACK_TIME {
            return;
        }
        let alpha = (self.alpha / 255.0).clamp(0.0, 1.0);
        r.sprite_whole(
            a.fl_logo,
            [100.0, 250.0, 600.0, 106.0],
            [1.0, 1.0, 1.0, alpha],
        );
        if self.typed > 0 {
            // El texto se centra con el ancho de la leyenda completa, no de lo escrito.
            let full = a.font_small.width(LEGEND);
            let shown: String = LEGEND.chars().take(self.typed).collect();
            r.text(
                &a.font_small,
                &shown,
                400.0 - full / 2.0,
                360.0,
                text_color(),
                Align::Left,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_end(skip_after: Option<f32>) -> f32 {
        let mut i = Intro::new(3);
        let mut t = 0.0;
        loop {
            if i.update(1.0 / 60.0) == Flow::Back {
                return t;
            }
            t += 1.0 / 60.0;
            if skip_after.is_some_and(|s| t > s) {
                i.key(KeyCode::Escape);
            }
            assert!(t < 60.0, "no termina");
        }
    }

    #[test]
    fn plays_black_fade_then_types_the_legend() {
        // 2 s de negro + ~2 s de fundido + 26 letras de 80-180 ms.
        let t = run_to_end(None);
        assert!((6.0..=11.0).contains(&t), "t={t}");
    }

    #[test]
    fn escape_skips_it() {
        assert!(run_to_end(Some(0.5)) < 1.0);
    }

    #[test]
    fn nothing_is_typed_before_the_logo_is_fully_visible() {
        let mut i = Intro::new(1);
        for _ in 0..(3.0 * 60.0) as usize {
            i.update(1.0 / 60.0);
        }
        assert!(i.alpha < 255.0 || i.typed == 0 || i.time > BLACK_TIME);
        assert_eq!(
            i.typed, 0,
            "a los 3 s aún está en el fundido (2 s negro + ~2 s)"
        );
    }
}
