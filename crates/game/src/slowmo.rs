//! Cámara lenta en la muerte que decide la ronda. No toca la simulación: solo cambia a qué ritmo
//! le llegan los ticks (el acumulador recibe el tiempo real por `time_scale`), así que los ticks
//! son exactamente los mismos y el determinismo se mantiene. La interpolación entre ticks
//! (`interp.rs`) la deja fluida.
//!
//! La cámara encuadra lo que cuenta la jugada: quien cae, la bomba que lo mató (el centro de la
//! explosión que lo alcanzó) y quien la puso. Con poco zoom si están lejos. La penumbra deja un
//! foco de luz sobre cada uno.

use cb_core::{Explosion, World, TILE_PIXELS};
use cb_render::{Camera, MAX_SPOTS};

/// Duración total en segundos reales: entra, se mantiene y sale.
const DURATION: f32 = 2.4;
const EASE_IN: f32 = 0.25;
const EASE_OUT: f32 = 0.8;
/// Velocidad mínima del juego (fracción del tiempo real).
const SLOWEST: f64 = 0.15;
/// Zoom máximo (cuando todo cabe en poco sitio).
const MAX_ZOOM: f32 = 1.45;
/// Margen alrededor de lo encuadrado (px virtuales).
const MARGIN: f32 = 70.0;
/// Esquina del mapa en el lienzo virtual (offset 60, 40 del original).
const ORIGIN: (f32, f32) = (60.0, 40.0);
const SCREEN: (f32, f32) = (800.0, 600.0);

/// Un bomber justo antes de un tick.
#[derive(Clone, Copy, Debug)]
pub struct Before {
    pub dead: bool,
    pub x: f32,
    pub y: f32,
    pub kills: i32,
}

impl Before {
    pub fn of(w: &World) -> Vec<Before> {
        w.bombers
            .iter()
            .map(|b| Before {
                dead: b.dead,
                x: b.x,
                y: b.y,
                kills: b.kills,
            })
            .collect()
    }
}

/// Quién mató a quién, para la etiqueta sobre el asesino.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub victim: usize,
    /// `None` si no se sabe; igual a `victim` si fue suicidio.
    pub killer: Option<usize>,
    /// Dónde cayó la víctima (centro, px virtuales).
    pub victim_pos: [f32; 2],
    /// 0..1 según la cámara lenta.
    pub strength: f32,
}

#[derive(Default)]
pub struct SlowMo {
    /// Segundos reales desde que empezó (`None`: no está activa).
    t: Option<f32>,
    /// Lo que se quiere ver: centro y tamaño del encuadre (px virtuales).
    center: [f32; 2],
    size: [f32; 2],
    victim: usize,
    victim_pos: [f32; 2],
    /// Centro de la explosión que lo mató.
    bomb: Option<[f32; 2]>,
    killer: Option<usize>,
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

/// Centro en pantalla de un objeto del mapa en (x, y) (esquina de su casilla, px del mapa).
fn screen_center(x: f32, y: f32) -> [f32; 2] {
    let half = TILE_PIXELS as f32 / 2.0;
    [ORIGIN.0 + x + half, ORIGIN.1 + y + half]
}

/// La explosión cubre la casilla (su cruz llega hasta ahí).
fn covers(e: &Explosion, (tx, ty): (i32, i32)) -> bool {
    (ty == e.ty && tx >= e.tx - e.left && tx <= e.tx + e.right)
        || (tx == e.tx && ty >= e.ty - e.up && ty <= e.ty + e.down)
}

impl SlowMo {
    pub fn new() -> Self {
        SlowMo::default()
    }

    /// Mira el tick recién simulado. `before` es cómo estaba cada bomber antes del tick (al
    /// morir, el original lo manda a (0, 0)). Si alguien acaba de morir y queda como mucho un
    /// bando, empieza la cámara lenta.
    pub fn after_tick(&mut self, w: &World, before: &[Before]) {
        if self.done {
            return;
        }
        let fallen = w
            .bombers
            .iter()
            .zip(before)
            .enumerate()
            .rfind(|(_, (b, was))| b.dead && !was.dead)
            .map(|(i, (_, was))| (i, was.x, was.y));
        let Some((victim, x, y)) = fallen else {
            return;
        };
        if sides_alive(w) > 1 {
            return;
        }
        self.done = true;
        self.t = Some(0.0);
        self.victim = victim;
        self.victim_pos = screen_center(x, y);
        let t = TILE_PIXELS as f32;
        let tile = (((x + t / 2.0) / t) as i32, ((y + t / 2.0) / t) as i32);
        // Quien lo mató es a quien el juego le acaba de sumar la muerte (`kill_bombers`); si
        // nadie, se mató solo. La bomba: la explosión de ese asesino que le alcanzó.
        let scorer = w
            .bombers
            .iter()
            .zip(before)
            .position(|(b, was)| b.kills > was.kills);
        let mut blasts = w.explosions().filter(|e| covers(e, tile));
        let blast = match scorer {
            Some(k) => blasts.find(|e| e.owner == k),
            None => blasts.next(),
        };
        self.bomb = blast.map(|e| screen_center(e.tx as f32 * t, e.ty as f32 * t));
        self.killer = scorer.or(blast.map(|e| e.owner));
        // Encuadre: la víctima, la bomba y el asesino si sigue vivo.
        let mut points = vec![self.victim_pos];
        points.extend(self.bomb);
        if let Some(k) = self.killer.filter(|&k| k != victim) {
            if let Some(b) = w.bombers.get(k).filter(|b| !b.dead) {
                points.push(screen_center(b.x, b.y));
            }
        }
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &points {
            for i in 0..2 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        self.center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        self.size = [hi[0] - lo[0] + 2.0 * MARGIN, hi[1] - lo[1] + 2.0 * MARGIN];
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

    /// Zoom que deja ver todo el encuadre (como mucho `MAX_ZOOM`, como poco nada).
    fn fit_zoom(&self) -> f32 {
        (SCREEN.0 / self.size[0])
            .min(SCREEN.1 / self.size[1])
            .clamp(1.0, MAX_ZOOM)
    }

    /// Cámara de este fotograma. La composición hace zoom dejando quieto `focus`; para que el
    /// encuadre quede a la vista se elige el `focus` que pone su centro lo más al medio posible
    /// sin salirse de la pantalla. `w` da dónde está ahora el asesino (su foco lo sigue).
    pub fn camera(&self, w: &World) -> Camera {
        let e = self.envelope();
        let zoom = 1.0 + (self.fit_zoom() - 1.0) * e;
        let mut cam = Camera {
            zoom,
            drama: e,
            ..Camera::default()
        };
        if zoom > 1.001 {
            // Lo visible con zoom `z` y foco `f` empieza en `f * (1 - 1/z)` y mide `pantalla / z`.
            let k = 1.0 - 1.0 / zoom;
            for (i, size) in [SCREEN.0, SCREEN.1].into_iter().enumerate() {
                let view = size / zoom;
                let start = (self.center[i] - view / 2.0).clamp(0.0, size - view);
                cam.focus[i] = start / k;
            }
        }
        let mut spots = vec![[self.victim_pos[0], self.victim_pos[1], 46.0, 1.0]];
        if let Some(b) = self.bomb {
            spots.push([b[0], b[1], 52.0, 1.0]);
        }
        if let Some(k) = self.killer.filter(|&k| k != self.victim) {
            if let Some(b) = w.bombers.get(k).filter(|b| !b.dead) {
                let c = screen_center(b.x, b.y);
                spots.push([c[0], c[1] - 6.0, 44.0, 1.0]);
            }
        }
        for (dst, src) in cam.spots.iter_mut().zip(spots.iter().take(MAX_SPOTS)) {
            *dst = *src;
        }
        cam
    }

    /// Quién mató a quién mientras dura (para la etiqueta).
    pub fn highlight(&self) -> Option<Highlight> {
        self.t.map(|_| Highlight {
            victim: self.victim,
            killer: self.killer,
            victim_pos: self.victim_pos,
            strength: self.envelope(),
        })
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
        s.size = [200.0, 150.0];
        s.advance(1.0);
        assert!((s.time_scale() - SLOWEST).abs() < 1e-6, "en lo más lento");
        s.advance(DURATION);
        assert_eq!(s.time_scale(), 1.0, "terminó");
    }

    /// Con el zoom aplicado, el centro del encuadre debe quedar dentro de lo visible.
    fn visible(cam: &Camera, p: [f32; 2]) -> bool {
        let screen = [SCREEN.0, SCREEN.1];
        (0..2).all(|i| {
            let start = cam.focus[i] * (1.0 - 1.0 / cam.zoom);
            let end = start + screen[i] / cam.zoom;
            p[i] >= start && p[i] <= end
        })
    }

    #[test]
    fn frames_what_matters_even_in_a_corner() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps");
        let mut maps: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "map"))
            .collect();
        maps.sort();
        let w = World::with_seed(cb_core::Map::load(&maps[0]).unwrap(), 1);
        let mut s = SlowMo::new();
        for (center, size) in [
            ([90.0, 70.0], [150.0, 150.0]),
            ([720.0, 540.0], [180.0, 140.0]),
            ([400.0, 300.0], [300.0, 200.0]),
        ] {
            s.t = Some(1.0);
            s.center = center;
            s.size = size;
            let cam = s.camera(&w);
            assert!(cam.zoom > 1.2, "algo de zoom");
            for dx in [-1.0, 1.0] {
                for dy in [-1.0, 1.0] {
                    let corner = [
                        (center[0] + dx * (size[0] / 2.0 - MARGIN)).clamp(0.0, 800.0),
                        (center[1] + dy * (size[1] / 2.0 - MARGIN)).clamp(0.0, 600.0),
                    ];
                    assert!(visible(&cam, corner), "{corner:?} fuera de {cam:?}");
                }
            }
        }
        // Si están muy lejos, sin zoom: se ve todo el mapa.
        s.size = [820.0, 620.0];
        assert_eq!(s.camera(&w).zoom, 1.0);
    }
}
