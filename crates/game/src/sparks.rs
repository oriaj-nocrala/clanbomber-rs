//! Ráfagas de partículas de la partida. El núcleo no sabe nada de ellas: aquí se compara el
//! mundo con el del fotograma anterior (explosiones nuevas, cajas rotas, bombers muertos...) y se
//! piden ráfagas al renderizador, que las simula en la GPU. Puro adorno: no toca la simulación.

use cb_core::{ExtraKind, Phase, World, TILE_PIXELS};
use cb_render::{Burst, ParticleKind, Renderer};

use crate::assets::Assets;
use crate::dying::Deaths;
use crate::haze::Haze;
use crate::shards::{Shards, Source};
use crate::steps::Steps;
use crate::weather::Weather;

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
    /// Extras en el suelo del fotograma anterior: tipo, posición y si estaba ardiendo.
    extras: Vec<(ExtraKind, i32, i32, bool)>,
    /// Trozos con textura y extras recogidos (se dibujan en `draw`).
    pub shards: Shards,
    /// Aire caliente sobre el fuego y las mechas.
    pub haze: Haze,
    /// Bombers que acaban de morir (se queman y sube su silueta).
    pub deaths: Deaths,
    /// Clima del tema (nieve, hojas...).
    pub weather: Weather,
    steps: Steps,
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
    pub fn update(&mut self, r: &mut Renderer, a: &Assets, w: &World, players: &[Player], dt: f32) {
        self.shards.update(dt);
        self.haze.update(w, dt);
        self.deaths.update(w, dt);
        if r.effects().particles {
            self.weather.update(w.config.theme, dt);
        }
        if !matches!(w.phase, Phase::Playing) {
            self.steps.reset();
            self.tracking = false;
            self.shards.clear();
            return;
        }
        let textured = r.effects().sprite_fx;
        let extras: Vec<_> = w
            .extras()
            .filter(|e| !e.falling && !e.flight.active)
            .map(|e| (e.kind, e.x as i32, e.y as i32, e.destroyed))
            .collect();
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
                    if textured {
                        // La caja (ya agrietada: celda tema*4+3) salta en trozos de sí misma.
                        let origin = [
                            ORIGIN.0 + x as f32 * TILE_PIXELS as f32,
                            ORIGIN.1 + y as f32 * TILE_PIXELS as f32,
                        ];
                        let cell = theme as u32 * 4 + 3;
                        self.shards
                            .shatter(Source::MapTile, &a.maptiles, cell, origin, 4);
                        dust(r, x as i32, y as i32);
                    } else {
                        box_debris(r, x as i32, y as i32, BOX_COLORS[theme]);
                    }
                }
            }
            self.extra_fx(r, a, w, &extras, textured);
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
            self.steps.update(r, w);
            speed_sparks(r, w, players);
        }

        self.tracking = true;
        self.explosions = explosions;
        self.crumbling = crumbling;
        self.vanishing = vanishing;
        self.dead = dead;
        self.extras = extras;
    }

    /// Extras que se queman (se parten) o que alguien recoge (saltan brillando).
    fn extra_fx(
        &mut self,
        r: &mut Renderer,
        a: &Assets,
        w: &World,
        now: &[(ExtraKind, i32, i32, bool)],
        textured: bool,
    ) {
        if !textured {
            return;
        }
        for &(kind, x, y, burning) in now {
            let was = self.extras.iter().find(|e| (e.0, e.1, e.2) == (kind, x, y));
            if burning && was.is_some_and(|e| !e.3) {
                let origin = [ORIGIN.0 + x as f32, ORIGIN.1 + y as f32];
                self.shards
                    .shatter(Source::Extra, &a.extras, kind as u32, origin, 4);
                r.emit(&Burst {
                    area: [12.0, 12.0],
                    color: [0.08, 0.07, 0.07, 0.6],
                    speed: [10.0, 40.0],
                    life: [0.6, 1.1],
                    size: 6.0,
                    ..Burst::new(ParticleKind::Smoke, [origin[0] + 20.0, origin[1] + 20.0], 5)
                });
            }
        }
        for &(kind, x, y, burning) in &self.extras {
            if burning || now.iter().any(|e| (e.0, e.1, e.2) == (kind, x, y)) {
                continue;
            }
            // Si sigue existiendo (vuela o cae) no lo ha recogido nadie.
            if w.extras()
                .any(|e| e.kind == kind && (e.falling || e.flight.active))
            {
                continue;
            }
            let good = kind.disease().is_none();
            let pos = [ORIGIN.0 + x as f32, ORIGIN.1 + y as f32];
            self.shards.pop(kind as u32, pos, good);
            let color = if good {
                [2.6, 2.0, 0.7, 1.0]
            } else {
                [0.8, 2.4, 0.6, 1.0]
            };
            r.emit(&Burst {
                area: [4.0, 4.0],
                color,
                speed: [70.0, 110.0],
                life: [0.25, 0.45],
                size: 1.6,
                ..Burst::new(ParticleKind::Glow, [pos[0] + 20.0, pos[1] + 20.0], 18)
            });
        }
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
