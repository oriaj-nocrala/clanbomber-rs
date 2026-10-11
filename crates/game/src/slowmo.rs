//! Cámara lenta en la muerte que decide la ronda. No toca la simulación: solo cambia a qué ritmo
//! le llegan los ticks (el acumulador recibe el tiempo real por `time_scale`), así que los ticks
//! son exactamente los mismos y el determinismo se mantiene. La interpolación entre ticks
//! (`interp.rs`) la deja fluida. Además, la cámara se acerca a quien cae.

use cb_core::World;
use cb_render::Camera;

/// Duración total en segundos reales: entra, se mantiene y sale.
const DURATION: f32 = 2.4;
const EASE_IN: f32 = 0.25;
const EASE_OUT: f32 = 0.8;
/// Velocidad mínima del juego (fracción del tiempo real).
const SLOWEST: f64 = 0.15;
const MAX_ZOOM: f32 = 0.45;

#[derive(Default)]
pub struct SlowMo {
    /// Segundos reales desde que empezó (`None`: no está activa).
    t: Option<f32>,
    focus: [f32; 2],
    /// Ya se disparó en esta ronda.
    done: bool,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Bandos con alguien vivo: cada bomber sin equipo cuenta solo y un equipo cuenta una vez.
fn sides_alive(w: &World) -> usize {
    let mut teams = [false; 5];
    let mut solo = 0;
    for b in w.bombers.iter().filter(|b| !b.dead) {
        match b.team {
            0 => solo += 1,
            t => teams[(t as usize).min(4)] = true,
        }
    }
    solo + teams.iter().filter(|&&t| t).count()
}

impl SlowMo {
    pub fn new() -> Self {
        SlowMo::default()
    }

    /// Mira el tick recién simulado (`was_dead`: quién estaba muerto antes). Si alguien acaba de
    /// morir y queda como mucho un bando, empieza la cámara lenta sobre él.
    pub fn after_tick(&mut self, w: &World, was_dead: &[bool]) {
        if self.done {
            return;
        }
        let fallen = w
            .bombers
            .iter()
            .zip(was_dead)
            .rfind(|(b, &was)| b.dead && !was)
            .map(|(b, _)| b);
        let Some(b) = fallen else {
            return;
        };
        if sides_alive(w) <= 1 {
            self.done = true;
            self.t = Some(0.0);
            self.focus = [60.0 + b.x + 20.0, 40.0 + b.y + 20.0];
        }
    }

    /// Nueva ronda: se puede volver a disparar.
    pub fn reset(&mut self) {
        *self = SlowMo::default();
    }

    pub fn active(&self) -> bool {
        self.t.is_some()
    }

    /// Avanza `dt` segundos reales.
    pub fn advance(&mut self, dt: f32) {
        if let Some(t) = &mut self.t {
            *t += dt;
            if *t >= DURATION {
                self.t = None;
            }
        }
    }

    /// 0 fuera de la cámara lenta, 1 en lo más lento.
    fn envelope(&self) -> f32 {
        self.t.map_or(0.0, |t| {
            smoothstep(0.0, EASE_IN, t) * (1.0 - smoothstep(DURATION - EASE_OUT, DURATION, t))
        })
    }

    /// Multiplicador del tiempo que reciben los ticks.
    pub fn time_scale(&self) -> f64 {
        1.0 - (1.0 - SLOWEST) * self.envelope() as f64
    }

    pub fn camera(&self) -> Camera {
        let e = self.envelope();
        Camera {
            zoom: 1.0 + MAX_ZOOM * e,
            focus: self.focus,
            drama: e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_in_holds_and_returns_to_normal() {
        let mut s = SlowMo::new();
        assert_eq!(s.time_scale(), 1.0);
        s.t = Some(0.0);
        s.advance(1.0);
        assert!((s.time_scale() - SLOWEST).abs() < 1e-6, "en lo más lento");
        assert!(s.camera().zoom > 1.4);
        s.advance(DURATION);
        assert_eq!(s.time_scale(), 1.0, "terminó");
        assert_eq!(s.camera().zoom, 1.0);
    }
}
