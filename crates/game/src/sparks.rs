//! Ráfagas de partículas de la partida. El núcleo no sabe nada de ellas: aquí se compara el
//! mundo con el del fotograma anterior (explosiones nuevas, cajas rotas, bombers muertos...) y se
//! piden ráfagas al renderizador, que las simula en la GPU. Puro adorno: no toca la simulación.

use cb_core::{Phase, World, TILE_PIXELS};
use cb_render::{Burst, ParticleKind, Renderer};

use crate::draw::{skin_color, speed_level, srgb_rgba, Player};

/// Esquina del mapa en el lienzo virtual (offset 60, 40 del original).
const ORIGIN: (f32, f32) = (60.0, 40.0);

/// Color medio de la caja de cada tema de `maptiles.png` (celda `tema * 4 + 2`).
const BOX_COLORS: [(u8, u8, u8); 6] = [
    (171, 178, 182),
    (62, 112, 10),
    (129, 89, 62),
    (71, 94, 85),
    (48, 48, 48),
    (159, 148, 76),
];

/// Fuego en color lineal "HDR": por encima de 1, para que el bloom lo recoja.
const FIRE: [f32; 4] = [3.2, 1.6, 0.45, 1.0];

#[derive(Default)]
pub struct Sparks {
    /// Hay un fotograma anterior con el que comparar (se pierde fuera de la ronda).
    tracking: bool,
    explosions: Vec<(i32, i32, usize)>,
    crumbling: Vec<(usize, usize)>,
    vanishing: Vec<(usize, usize)>,
    dead: Vec<bool>,
}

fn tile_center(tx: i32, ty: i32) -> [f32; 2] {
    let t = TILE_PIXELS as f32;
    [
        ORIGIN.0 + tx as f32 * t + t / 2.0,
        ORIGIN.1 + ty as f32 * t + t / 2.0,
    ]
}

impl Sparks {
    pub fn new() -> Self {
        Sparks::default()
    }

    /// Lanza las ráfagas de lo que cambió desde la última llamada. `dt` son los segundos de
    /// juego transcurridos (0 en pausa: no sale nada nuevo).
    pub fn update(&mut self, r: &mut Renderer, w: &World, players: &[Player], dt: f32) {
        if !matches!(w.phase, Phase::Playing) {
            self.tracking = false;
            return;
        }
        let explosions: Vec<_> = w.explosions().map(|e| (e.tx, e.ty, e.owner)).collect();
        let crumbling: Vec<_> = w.map.crumbling.iter().map(|c| (c.x, c.y)).collect();
        let vanishing: Vec<_> = w.map.vanishing.iter().map(|v| (v.x, v.y)).collect();
        let dead: Vec<_> = w.bombers.iter().map(|b| b.dead).collect();

        if self.tracking && dt > 0.0 {
            for e in w.explosions() {
                if !self.explosions.contains(&(e.tx, e.ty, e.owner)) {
                    explosion(r, e.tx, e.ty, [e.left, e.right, e.up, e.down]);
                }
            }
            let theme = w.config.theme.clamp(0, 5) as usize;
            for &(x, y) in &self.crumbling {
                if !crumbling.contains(&(x, y)) {
                    box_debris(r, x as i32, y as i32, BOX_COLORS[theme]);
                }
            }
            for &(x, y) in &vanishing {
                if !self.vanishing.contains(&(x, y)) {
                    dust(r, x as i32, y as i32);
                }
            }
            for (i, b) in w.bombers.iter().enumerate() {
                if b.dead && !self.dead.get(i).copied().unwrap_or(true) {
                    let pos = [
                        ORIGIN.0 + b.get_x() as f32 + 20.0,
                        ORIGIN.1 + b.get_y() as f32 + 20.0,
                    ];
                    death(r, pos, w.config.kids_mode);
                }
            }
            fuses(r, w);
            speed_sparks(r, w, players);
        }

        self.tracking = true;
        self.explosions = explosions;
        self.crumbling = crumbling;
        self.vanishing = vanishing;
        self.dead = dead;
    }
}

/// Bola de fuego: chispas y humo en el centro y chispas que corren por cada brazo.
fn explosion(r: &mut Renderer, tx: i32, ty: i32, [left, right, up, down]: [i32; 4]) {
    let c = tile_center(tx, ty);
    r.emit(&Burst {
        area: [8.0, 8.0],
        color: FIRE,
        speed: [60.0, 260.0],
        life: [0.25, 0.7],
        size: 2.0,
        ..Burst::new(ParticleKind::Spark, c, 70)
    });
    r.emit(&Burst {
        area: [10.0, 10.0],
        color: [0.10, 0.09, 0.08, 0.55],
        speed: [10.0, 40.0],
        life: [0.8, 1.6],
        size: 9.0,
        ..Burst::new(ParticleKind::Smoke, c, 10)
    });
    let arms = [(left, -1, 0), (right, 1, 0), (up, 0, -1), (down, 0, 1)];
    for (len, dx, dy) in arms {
        for i in 1..=len {
            let p = tile_center(tx + dx * i, ty + dy * i);
            r.emit(&Burst {
                area: [14.0, 14.0],
                color: FIRE,
                speed: [10.0, 70.0],
                life: [0.2, 0.5],
                drift: [dx as f32 * 90.0, dy as f32 * 90.0],
                size: 1.6,
                ..Burst::new(ParticleKind::Spark, p, 14)
            });
        }
    }
}

/// Cascotes del color de la caja y una nube de polvo.
fn box_debris(r: &mut Renderer, tx: i32, ty: i32, (cr, cg, cb): (u8, u8, u8)) {
    let c = tile_center(tx, ty);
    r.emit(&Burst {
        area: [14.0, 14.0],
        color: srgb_rgba(cr, cg, cb, 255),
        speed: [40.0, 180.0],
        life: [0.4, 0.9],
        size: 2.5,
        ..Burst::new(ParticleKind::Debris, c, 28)
    });
    dust(r, tx, ty);
}

fn dust(r: &mut Renderer, tx: i32, ty: i32) {
    r.emit(&Burst {
        area: [12.0, 12.0],
        color: [0.25, 0.23, 0.2, 0.4],
        speed: [5.0, 30.0],
        life: [0.6, 1.2],
        size: 7.0,
        ..Burst::new(ParticleKind::Smoke, tile_center(tx, ty), 6)
    });
}

/// Salpicadura de sangre (o confeti en el modo niños) al morir un bomber.
fn death(r: &mut Renderer, pos: [f32; 2], kids: bool) {
    let (kind, color) = if kids {
        (ParticleKind::Confetti, [1.0; 4])
    } else {
        (ParticleKind::Debris, srgb_rgba(150, 0, 0, 255))
    };
    r.emit(&Burst {
        area: [6.0, 6.0],
        color,
        speed: [40.0, 220.0],
        life: [0.5, 1.3],
        size: 1.6,
        ..Burst::new(kind, pos, 60)
    });
}

/// Los bombers muy rápidos sueltan destellos de su color al correr.
fn speed_sparks(r: &mut Renderer, w: &World, players: &[Player]) {
    for (b, p) in w.bombers.iter().zip(players) {
        let k = speed_level(b);
        if k < 0.3 || b.dead || b.is_falling() || b.flight.active {
            continue;
        }
        let c = skin_color(p.skin);
        let boost = 1.5 + 2.5 * k;
        r.emit(&Burst {
            area: [8.0, 10.0],
            color: [c[0] * boost, c[1] * boost, c[2] * boost, 1.0],
            speed: [5.0, 25.0 + 40.0 * k],
            life: [0.15, 0.25 + 0.3 * k],
            size: 1.2 + 1.3 * k,
            ..Burst::new(
                ParticleKind::Glow,
                [
                    ORIGIN.0 + b.get_x() as f32 + 20.0,
                    ORIGIN.1 + b.get_y() as f32 + 22.0,
                ],
                1 + (k * 3.0) as u32,
            )
        });
    }
}

/// Las bombas a punto de estallar echan chispas por arriba, cada vez más.
fn fuses(r: &mut Renderer, w: &World) {
    for b in w.bombs() {
        if b.exploding || b.falling || b.flight.active || b.countdown > 1.0 {
            continue;
        }
        let count = if b.countdown < 0.4 { 2 } else { 1 };
        r.emit(&Burst {
            area: [3.0, 2.0],
            color: FIRE,
            speed: [20.0, 70.0],
            life: [0.15, 0.35],
            drift: [0.0, -40.0],
            size: 1.2,
            ..Burst::new(
                ParticleKind::Spark,
                [ORIGIN.0 + b.x + 20.0, ORIGIN.1 + b.y + 8.0],
                count,
            )
        });
    }
}
