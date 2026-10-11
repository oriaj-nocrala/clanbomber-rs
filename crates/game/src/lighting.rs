//! Luces de la partida para la iluminación dinámica del renderizador: el mapa en penumbra y lo
//! que alumbra (explosiones, bombas que laten, bombers, extras). Muros y cajas tapan la luz.
//! Puro adorno: solo lee el mundo.

use cb_core::bomb::EXPLOSION_PERIOD;
use cb_core::{Tile, World, MAP_HEIGHT, MAP_WIDTH, TICK_HZ, TILE_PIXELS};
use cb_render::{Light, Lighting, Renderer};

use crate::draw::{skin_color, speed_level, Player};

/// Esquina del mapa en el lienzo virtual.
const ORIGIN: [f32; 2] = [60.0, 40.0];
/// Penumbra algo fría, para que las luces cálidas resalten.
const AMBIENT: [f32; 3] = [0.5, 0.53, 0.64];
const FIRE: [f32; 3] = [1.0, 0.62, 0.3];

/// Ruido barato y estable para el parpadeo de las llamas (sin tocar el `Rng` del mundo).
fn flicker(seed: u32, t: f32) -> f32 {
    let s = seed as f32 * 12.9898;
    0.88 + 0.07 * (t * 31.0 + s).sin() + 0.05 * (t * 53.0 + s * 1.7).sin()
}

/// Pide las luces del fotograma y aplica la iluminación a lo dibujado hasta ahora.
pub fn apply(r: &mut Renderer, w: &World, players: &[Player]) {
    if !r.effects().lighting {
        return;
    }
    let t = TILE_PIXELS as f32;
    let time = w.tick as f32 / TICK_HZ as f32;
    let center = |tx: i32, ty: i32| {
        [
            ORIGIN[0] + tx as f32 * t + t / 2.0,
            ORIGIN[1] + ty as f32 * t + t / 2.0,
        ]
    };

    // Primero lo que más importa, por si se llena el cupo de luces: el fuego.
    for e in w.explosions() {
        // `period` puede quedar algo por debajo de 0 en el último tick.
        let p = (1.0 - e.period / EXPLOSION_PERIOD).clamp(0.0, 1.0);
        let fade = (1.0 - p).powf(1.3);
        let seed = (e.tx * 31 + e.ty * 17) as u32;
        r.light(Light {
            pos: center(e.tx, e.ty),
            radius: (110.0 + 14.0 * e.power as f32).min(220.0),
            intensity: 1.7 * fade * flicker(seed, time),
            color: FIRE,
        });
        // Una luz cada dos casillas de cada brazo.
        let arms = [
            (e.left, -1, 0),
            (e.right, 1, 0),
            (e.up, 0, -1),
            (e.down, 0, 1),
        ];
        for (len, dx, dy) in arms {
            for i in (2..=len).step_by(2) {
                r.light(Light {
                    pos: center(e.tx + dx * i, e.ty + dy * i),
                    radius: 80.0,
                    intensity: 0.9 * fade * flicker(seed + i as u32, time),
                    color: FIRE,
                });
            }
        }
    }
    // Las bombas laten en rojo, más deprisa y más fuerte cuanto menos les queda.
    let fuse = (w.config.bomb_countdown as f32 / 10.0).max(0.1);
    for b in w.bombs().filter(|b| !b.exploding && !b.falling) {
        let urgency = (1.0 - b.countdown / fuse).clamp(0.0, 1.0);
        let beat = 0.5 + 0.5 * (time * (4.0 + 16.0 * urgency) * std::f32::consts::TAU).sin();
        r.light(Light {
            pos: [ORIGIN[0] + b.x + 20.0, ORIGIN[1] + b.y + 20.0],
            radius: 50.0 + 25.0 * urgency,
            intensity: 0.25 + 0.75 * urgency * beat,
            color: [1.0, 0.25, 0.08],
        });
    }
    // Cada bomber lleva un halo para que no se pierda en la penumbra; los rápidos, de su color.
    for (b, p) in w.bombers.iter().zip(players) {
        if b.dead || b.is_falling() || b.flight.active {
            continue;
        }
        let pos = [
            ORIGIN[0] + b.get_x() as f32 + 20.0,
            ORIGIN[1] + b.get_y() as f32 + 20.0,
        ];
        r.light(Light {
            pos,
            radius: 85.0,
            intensity: 0.42,
            color: [1.0, 0.95, 0.85],
        });
        let k = speed_level(b);
        if k > 0.0 {
            let c = skin_color(p.skin);
            r.light(Light {
                pos,
                radius: 60.0 + 50.0 * k,
                intensity: 0.7 * k,
                color: [c[0], c[1], c[2]],
            });
        }
    }
    // Los extras brillan un poco, para que se vean al salir de una caja.
    for (i, e) in w
        .extras()
        .filter(|e| !e.falling && !e.flight.active)
        .enumerate()
    {
        r.light(Light {
            pos: [ORIGIN[0] + e.x + 20.0, ORIGIN[1] + e.y + 20.0],
            radius: 36.0,
            intensity: 0.45 + 0.15 * (time * 3.0 + i as f32).sin(),
            color: [1.0, 0.95, 0.7],
        });
    }

    let mut occluders = [0u8; MAP_WIDTH * MAP_HEIGHT];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if matches!(w.map.get(x as i32, y as i32), Tile::Wall | Tile::Box) {
                occluders[y * MAP_WIDTH + x] = 255;
            }
        }
    }
    r.lighting(&Lighting {
        ambient: AMBIENT,
        origin: ORIGIN,
        cell: t,
        grid: (MAP_WIDTH as u32, MAP_HEIGHT as u32),
        occluders: &occluders,
    });
}
