//! Pisadas: nubecitas de polvo bajo los pies al frenar o girar (más cuanto más rápido va el
//! bomber) y escarcha que salta al patinar sobre el hielo. Compara la posición de cada bomber
//! entre ticks; puro adorno, no toca la simulación.

use cb_core::{Tile, World, TILE_PIXELS};
use cb_render::{Burst, ParticleKind, Renderer};

use crate::draw::{speed_level, srgb_rgba};

/// Esquina del mapa en el lienzo virtual (offset 60, 40 del original).
const ORIGIN: (f32, f32) = (60.0, 40.0);
/// Color del polvo según el suelo de cada tema de `maptiles.png`: algo más claro que el suelo
/// tal como se ve con la luz ambiente (las partículas se dibujan después de la iluminación).
const DUST: [(u8, u8, u8); 6] = [
    (200, 204, 214),
    (150, 160, 96),
    (140, 150, 120),
    (160, 150, 110),
    (150, 84, 64),
    (140, 140, 146),
];
/// Ticks quieto para que cuente como frenazo: la IA para un tick en cada casilla al girar.
const BRAKE_TICKS: u64 = 3;
/// Escarcha: una chispa cada tantos píxeles patinados.
const FROST_EVERY: f32 = 3.0;

#[derive(Clone, Copy, Default)]
struct Foot {
    pos: (f32, f32),
    /// Último movimiento no nulo, normalizado (cero si no ha andado).
    heading: (f32, f32),
    /// Ticks seguidos sin moverse.
    still: u64,
    frost: f32,
}

#[derive(Default)]
pub struct Steps {
    tick: Option<u64>,
    feet: Vec<Foot>,
}

impl Steps {
    /// Olvida lo anterior (nueva ronda o fuera de la partida).
    pub fn reset(&mut self) {
        self.tick = None;
        self.feet.clear();
    }

    /// Mira lo que cambió desde el último tick visto (entre ticks no se mueve nada).
    pub fn update(&mut self, r: &mut Renderer, w: &World) {
        if self.tick == Some(w.tick) {
            return;
        }
        let fresh = self.tick.is_none() || self.feet.len() != w.bombers.len();
        let ticks = w.tick.saturating_sub(self.tick.unwrap_or(w.tick)).max(1);
        self.tick = Some(w.tick);
        if fresh {
            self.feet = w
                .bombers
                .iter()
                .map(|b| Foot {
                    pos: (b.x, b.y),
                    ..Foot::default()
                })
                .collect();
            return;
        }
        let theme = w.config.theme.clamp(0, 5) as usize;
        for (b, foot) in w.bombers.iter().zip(&mut self.feet) {
            let vel = (b.x - foot.pos.0, b.y - foot.pos.1);
            foot.pos = (b.x, b.y);
            let dist = (vel.0 * vel.0 + vel.1 * vel.1).sqrt();
            // Un salto grande no es andar (reaparición, teletransporte...).
            if b.dead || b.is_falling() || b.flight.active || dist > 20.0 {
                *foot = Foot {
                    pos: foot.pos,
                    ..Foot::default()
                };
                continue;
            }
            let feet = [ORIGIN.0 + b.x + 20.0, ORIGIN.1 + b.y + 32.0];
            let (tx, ty) = b.tile();
            if matches!(w.map.get(tx, ty), Tile::Ice) {
                if dist > 0.0 {
                    foot.frost += dist;
                    while foot.frost >= FROST_EVERY {
                        foot.frost -= FROST_EVERY;
                        frost(r, feet, vel, dist);
                    }
                }
                // Al salir del hielo no cuenta como giro ni frenazo.
                foot.heading = (0.0, 0.0);
                foot.still = 0;
                continue;
            }
            let level = speed_level(b);
            let had_heading = foot.heading != (0.0, 0.0);
            if dist <= 0.01 {
                let before = foot.still;
                foot.still += ticks;
                if had_heading && before < BRAKE_TICKS && foot.still >= BRAKE_TICKS {
                    // Frenazo: el polvo sigue hacia donde iba.
                    puff(r, feet, foot.heading, 3 + (level * 5.0) as u32, DUST[theme]);
                    foot.heading = (0.0, 0.0);
                }
                continue;
            }
            let heading = (vel.0 / dist, vel.1 / dist);
            let dot = heading.0 * foot.heading.0 + heading.1 * foot.heading.1;
            if had_heading && dot < 0.3 {
                // Giro (aunque haya parado un instante): el polvo sale por donde venía.
                puff(r, feet, foot.heading, 2 + (level * 5.0) as u32, DUST[theme]);
            }
            foot.heading = heading;
            foot.still = 0;
        }
    }
}

fn puff(r: &mut Renderer, pos: [f32; 2], dir: (f32, f32), count: u32, (cr, cg, cb): (u8, u8, u8)) {
    let t = TILE_PIXELS as f32;
    r.emit(&Burst {
        area: [t * 0.12, 2.0],
        color: srgb_rgba(cr, cg, cb, 170),
        speed: [6.0, 22.0],
        life: [0.3, 0.6],
        drift: [dir.0 * 22.0, dir.1 * 22.0 - 4.0],
        size: 3.5,
        ..Burst::new(ParticleKind::Smoke, pos, count)
    });
}

fn frost(r: &mut Renderer, pos: [f32; 2], vel: (f32, f32), dist: f32) {
    // Saltan hacia atrás y a los lados del patín, como hielo raspado.
    let back = (-vel.0 / dist, -vel.1 / dist);
    r.emit(&Burst {
        area: [4.0, 2.0],
        color: [1.6, 2.1, 2.8, 1.0],
        speed: [12.0, 40.0],
        life: [0.3, 0.6],
        drift: [back.0 * 30.0, back.1 * 30.0 - 10.0],
        size: 1.6,
        ..Burst::new(ParticleKind::Glow, pos, 2)
    });
}
