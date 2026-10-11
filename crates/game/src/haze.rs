//! De dónde sale el aire caliente (`Renderer::heat`): las llamas de cada explosión, las brasas
//! que quedan temblando y suben un rato después de apagarse, y las mechas a punto de estallar.
//! Puro adorno: no toca la simulación.

use cb_core::bomb::EXPLOSION_PERIOD;
use cb_core::{Phase, World, TILE_PIXELS};
use cb_render::Renderer;

/// Esquina del mapa en el lienzo virtual (offset 60, 40 del original).
const ORIGIN: (f32, f32) = (60.0, 40.0);
/// Segundos que el aire sigue caliente cuando el fuego ya se apagó.
const LINGER: f32 = 1.6;
/// Lo que suben las brasas por segundo (el aire caliente asciende).
const RISE: f32 = 16.0;
/// Las últimas `FUSE` s de mecha calientan el aire sobre la bomba.
const FUSE: f32 = 1.5;

/// Clave de una explosión (como en `sparks.rs`) y la forma de su calor.
type Blaze = ((i32, i32, usize), [[f32; 4]; 2]);

struct Ember {
    rects: [[f32; 4]; 2],
    age: f32,
}

#[derive(Default)]
pub struct Haze {
    live: Vec<Blaze>,
    embers: Vec<Ember>,
}

/// Calor de una cruz de explosión: una banda horizontal y otra vertical, algo más altas por
/// arriba porque el aire caliente sube.
fn cross(tx: i32, ty: i32, arms: [i32; 4]) -> [[f32; 4]; 2] {
    let t = TILE_PIXELS as f32;
    let (cx, cy) = (
        ORIGIN.0 + (tx as f32 + 0.5) * t,
        ORIGIN.1 + (ty as f32 + 0.5) * t,
    );
    let reach = arms.map(|n| (n as f32 + 0.5) * t - 6.0);
    let (x0, x1) = (cx - reach[0], cx + reach[1]);
    let (y0, y1) = (cy - reach[2] - 8.0, cy + reach[3]);
    [
        [x0, cy - 22.0, x1 - x0, 34.0],
        [cx - 14.0, y0, 28.0, y1 - y0],
    ]
}

impl Haze {
    /// Las explosiones que se apagaron desde la última llamada dejan brasas; `dt` son los
    /// segundos de juego transcurridos.
    pub fn update(&mut self, w: &World, dt: f32) {
        if !matches!(w.phase, Phase::Playing) {
            self.live.clear();
            self.embers.clear();
            return;
        }
        let live: Vec<Blaze> = w
            .explosions()
            .map(|e| {
                (
                    (e.tx, e.ty, e.owner),
                    cross(e.tx, e.ty, [e.left, e.right, e.up, e.down]),
                )
            })
            .collect();
        for (key, rects) in &self.live {
            if !live.iter().any(|(k, _)| k == key) {
                self.embers.push(Ember {
                    rects: *rects,
                    age: 0.0,
                });
            }
        }
        self.live = live;
        for e in &mut self.embers {
            e.age += dt;
        }
        self.embers.retain(|e| e.age < LINGER);
    }

    pub fn apply(&self, r: &mut Renderer, w: &World) {
        for e in w.explosions() {
            // Llega a tope enseguida y sigue así hasta que el fuego se apaga; las brasas
            // continúan desde ahí sin salto.
            let progress = (1.0 - e.period / EXPLOSION_PERIOD).clamp(0.0, 1.0);
            let k = (progress / 0.15).min(1.0);
            for rect in cross(e.tx, e.ty, [e.left, e.right, e.up, e.down]) {
                r.heat(rect, 10.0, k);
            }
        }
        for e in &self.embers {
            let f = e.age / LINGER;
            // Se apaga suave y el calor se eleva y se ensancha mientras se disipa.
            let k = (1.0 - f) * (1.0 - f);
            let lift = e.age * RISE;
            for [x, y, w, h] in e.rects {
                r.heat([x, y - lift, w, h], 10.0 + 14.0 * f, k);
            }
        }
        for b in w.bombs() {
            if b.exploding || b.falling || b.flight.active || b.countdown > FUSE {
                continue;
            }
            // Una columna fina sobre la mecha que se calienta según se acaba.
            let k = 0.15 + 0.45 * (1.0 - b.countdown / FUSE);
            let (x, y) = (ORIGIN.0 + b.x, ORIGIN.1 + b.y);
            r.heat([x + 14.0, y - 16.0, 12.0, 24.0], 7.0, k);
        }
    }
}
