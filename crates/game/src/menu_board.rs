//! Menú raíz como tablero 3D (`docs/menu-3d.md`): una isla de ClanBomber flotando en la
//! oscuridad con una caja por opción, el bomber del jugador 1 como cursor y una bomba encendida a
//! su lado. Los cubos van por la escena 3D del renderer; el bomber, la bomba y los nombres son
//! sprites y texto 2D en la posición proyectada.
//!
//! Al elegir una caja, la mecha se consume, la caja estalla en trozos (cubos que giran) y la
//! cámara se zambulle en el destello hacia el submenú; al volver, los trozos recorren el mismo
//! camino al revés. Enter salta cada paso y cualquier otra tecla corta la secuencia.

use cb_render::{
    faces, Align, Burst, Cube, Fire, Light3d, ParticleKind, Renderer, Shadow, TextStyle, TextureId,
    View3d,
};
use winit::keyboard::KeyCode;

use crate::assets::Assets;

/// Casillas del tablero (con los muros del borde).
const W: usize = 15;
const D: usize = 6;
/// Fila de las cajas de opciones; la primera está en `BOX_X0` y las demás cada `BOX_STEP`.
const BOX_ROW: usize = 2;
const BOX_X0: usize = 2;
const BOX_STEP: usize = 2;
/// Casillas por segundo al caminar entre cajas.
const WALK_SPEED: f32 = 9.0;
/// Alto y margen de las cajas de opciones.
const BOX_H: f32 = 0.86;
const BOX_INSET: f32 = 0.07;

/// Segundos de mecha, del estallido con la zambullida, de la vuelta y del destello que se
/// desvanece sobre el submenú.
const FUSE: f32 = 0.5;
const BOOM: f32 = 0.6;
const RETURN: f32 = 0.65;
const FLASH_OUT: f32 = 0.35;
/// Gravedad de los trozos (casillas/s²) y el instante en que ya cayeron todos.
const GRAVITY: f32 = 18.0;
const SHARDS_LANDED: f32 = 1.2;

/// En qué punto de la secuencia de elegir una caja está el tablero.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Idle,
    /// La mecha se consume junto a la caja.
    Fuse {
        at: usize,
        t: f32,
    },
    /// La caja estalló hace `t` s; la cámara se zambulle en el destello.
    Boom {
        at: usize,
        t: f32,
    },
    /// En el submenú (u otra pantalla) que abrió la caja; `t` desde que se entró.
    Open {
        at: usize,
        t: f32,
    },
    /// De vuelta en la raíz: los trozos vuelven a su sitio y la cámara se aleja.
    Return {
        at: usize,
        t: f32,
    },
}

/// Qué hacer con una tecla pulsada en la raíz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// El tablero no la usa: va al menú como siempre.
    Pass,
    /// Se encendió la mecha (suena poner la bomba).
    Lit,
    /// El tablero la usó (p. ej. Enter con la mecha encendida hace estallar ya).
    Consumed,
    /// Se cortó la zambullida: hay que entrar ya en la caja (Enter al menú) y, si la tecla no
    /// era Enter, dársela después al menú.
    Finish,
}

/// Lo que pasó al avanzar el tablero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Estalló la caja.
    Exploded,
    /// Terminó la zambullida: entrar en la caja (Enter al menú).
    Enter,
}

/// Cómo se ve el tablero: tema, aspecto del bomber y efectos opcionales (opciones gráficas y
/// "Shaky Explosions").
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub theme: u32,
    pub skin: usize,
    pub fire_shader: bool,
    pub shockwaves: bool,
    pub shake: bool,
    /// Los nombres sobre las cajas (no detrás del letrero de un submenú).
    pub labels: bool,
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Esquina mínima de la caja `i`.
fn box_min(i: usize) -> [f32; 3] {
    [
        (BOX_X0 + BOX_STEP * i) as f32 + BOX_INSET,
        0.0,
        BOX_ROW as f32 + BOX_INSET,
    ]
}

/// Centro de la caja `i`.
fn box_center(i: usize) -> [f32; 3] {
    let m = box_min(i);
    let w = 1.0 - 2.0 * BOX_INSET;
    [m[0] + w / 2.0, BOX_H / 2.0, m[2] + w / 2.0]
}

/// Trozo `n` (0..8, un octante) de una caja a `t` s del estallido: desplazamiento desde su sitio
/// y giro. Vuelo balístico hasta tocar el suelo, donde se queda; como es una función del tiempo,
/// la vuelta es el mismo camino al revés.
fn shard(n: usize, t: f32) -> ([f32; 3], [f32; 3]) {
    let (i, j, k) = (n & 1, (n >> 1) & 1, (n >> 2) & 1);
    let h = |s: usize| hash(n * 7 + s, 13);
    let (ox, oz) = (i as f32 * 2.0 - 1.0, k as f32 * 2.0 - 1.0);
    let v = [
        ox * (1.6 + 1.6 * h(0)),
        4.0 + 2.5 * h(1) + 1.5 * j as f32,
        oz * (0.8 + 0.8 * h(2)),
    ];
    let y0 = j as f32 * BOX_H / 2.0;
    let land = (v[1] + (v[1] * v[1] + 2.0 * GRAVITY * y0).sqrt()) / GRAVITY;
    let t = t.clamp(0.0, land);
    let spin = [
        (h(3) - 0.5) * 22.0 * t,
        (h(4) - 0.5) * 14.0 * t,
        (h(5) - 0.5) * 22.0 * t,
    ];
    ([v[0] * t, v[1] * t - GRAVITY * t * t / 2.0, v[2] * t], spin)
}

/// Estado de la animación (lo que no es del menú: dónde está el bomber, el reloj).
pub struct Board {
    /// Columna (en casillas, fraccionaria) donde está el bomber ahora.
    x: f32,
    /// Fotograma de la caminata (1..9 como `Bomber::anim_count`); 0 = quieto.
    anim: f32,
    /// Mirando a la derecha (si no, a la izquierda) mientras camina.
    right: bool,
    time: f32,
    /// Ascuas y llamas pendientes de lanzar (fracción).
    ember_acc: f32,
    flame_acc: f32,
    stage: Stage,
    /// La caja acaba de estallar: falta lanzar sus chispas (en el próximo `update`).
    burst_pending: bool,
}

/// Columna del bomber junto a la caja `i` (a su izquierda).
fn stand_x(i: usize) -> f32 {
    (BOX_X0 + BOX_STEP * i) as f32 - 1.0 + 0.5
}

/// Cámara del tablero, que respira apenas con el tiempo: mira desde delante y arriba (`PITCH`)
/// a un punto algo detrás del centro, para que la isla quede en la mitad de abajo bajo el logo.
fn view(time: f32) -> View3d {
    const PITCH: f32 = 0.9;
    const DISTANCE: f32 = 25.5;
    let target = [W as f32 / 2.0, 0.0, 0.0];
    let d = DISTANCE * (1.0 + 0.008 * (time * 0.35).sin());
    View3d {
        eye: [
            target[0] + 0.3 * (time * 0.21).sin(),
            target[1] + d * PITCH.sin(),
            target[2] + d * PITCH.cos(),
        ],
        target,
        fov_y: 0.5,
        ambient: [0.2, 0.2, 0.27],
        sun_dir: [0.35, -1.0, -0.55],
        sun: [0.45, 0.42, 0.4],
    }
}

/// Esquinas del tablero, sobre los muros (centro de la llama).
fn torches() -> [[f32; 3]; 4] {
    let (x1, z1) = (W as f32 - 0.5, D as f32 - 0.5);
    [
        [0.5, 1.25, 0.5],
        [x1, 1.25, 0.5],
        [0.5, 1.25, z1],
        [x1, 1.25, z1],
    ]
}

/// Ruido determinista por casilla en [0, 1).
fn hash(x: usize, z: usize) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (z as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

/// Parpadeo de fuego en [0.75, 1.1] con su propia fase.
fn flicker(time: f32, seed: f32) -> f32 {
    0.92 + 0.1 * (time * 11.0 + seed * 7.0).sin() + 0.08 * (time * 23.0 + seed * 3.0).sin()
}

/// Regiones de textura de los muros, las cajas y la tierra de la isla de un tema.
struct BlockFaces {
    tex: TextureId,
    wall_top: [f32; 4],
    wall_side: [f32; 4],
    box_top: [f32; 4],
    box_side: [f32; 4],
    earth: [f32; 4],
    earth_tint: [f32; 3],
}

impl BlockFaces {
    /// Las de `maptiles_cubes.png` si está; si no, las casillas de la partida (el lado igual que
    /// arriba y la tierra como el muro oscurecido).
    fn of(a: &Assets, theme: u32) -> BlockFaces {
        match &a.cubes {
            Some(c) => {
                let cell = |n: u32| c.cell(theme * 5 + n);
                BlockFaces {
                    tex: c.tex,
                    wall_top: cell(0),
                    wall_side: cell(1),
                    box_top: cell(2),
                    box_side: cell(3),
                    earth: cell(4),
                    earth_tint: [0.8; 3],
                }
            }
            None => {
                let t = &a.maptiles_3d;
                let cell = |n: u32| t.cell(theme * 4 + n);
                BlockFaces {
                    tex: t.tex,
                    wall_top: cell(1),
                    wall_side: cell(1),
                    box_top: cell(2),
                    box_side: cell(2),
                    earth: cell(1),
                    earth_tint: [0.42, 0.33, 0.27],
                }
            }
        }
    }
}

impl Board {
    pub fn new(selection: usize) -> Self {
        Board {
            x: stand_x(selection),
            anim: 0.0,
            right: true,
            time: 0.0,
            ember_acc: 0.0,
            flame_acc: 0.0,
            stage: Stage::Idle,
            burst_pending: false,
        }
    }

    fn explode(&mut self, at: usize) {
        self.stage = Stage::Boom { at, t: 0.0 };
        self.burst_pending = true;
    }

    /// Una tecla en la raíz, antes de dársela al menú.
    pub fn key(&mut self, code: KeyCode, selection: usize) -> KeyAction {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        match self.stage {
            Stage::Idle | Stage::Return { .. } if enter => {
                self.stage = Stage::Fuse {
                    at: selection,
                    t: 0.0,
                };
                KeyAction::Lit
            }
            Stage::Fuse { at, .. } if enter => {
                self.explode(at);
                KeyAction::Consumed
            }
            Stage::Fuse { .. } => {
                self.stage = Stage::Idle;
                KeyAction::Pass
            }
            Stage::Boom { at, .. } => {
                self.stage = Stage::Open { at, t: 0.0 };
                KeyAction::Finish
            }
            _ => KeyAction::Pass,
        }
    }

    /// Cuánto se zambulló la cámara hacia la caja que estalló (0 = nada, 1 = dentro).
    fn dive(&self) -> Option<(usize, f32)> {
        match self.stage {
            // Tras el destello, el submenú se ve sobre el tablero desde la cámara de siempre.
            Stage::Boom { at, t } => Some((at, smoothstep((t - 0.12) / (BOOM - 0.12)))),
            _ => None,
        }
    }

    /// Cámara con la zambullida.
    fn camera(&self) -> View3d {
        let mut v = view(self.time);
        let Some((at, k)) = self.dive() else {
            return v;
        };
        let c = box_center(at);
        let eye = [c[0], c[1] + 1.5, c[2] + 1.4];
        for i in 0..3 {
            v.eye[i] += (eye[i] - v.eye[i]) * k;
            v.target[i] += (c[i] - v.target[i]) * k;
        }
        v
    }

    /// Blanco del destello sobre el tablero (al final de la zambullida y al volver).
    fn flash(&self) -> f32 {
        match self.stage {
            Stage::Boom { t, .. } => smoothstep((t - 0.3) / (BOOM - 0.3)),
            _ => 0.0,
        }
    }

    /// El destello que se desvanece sobre el submenú recién abierto (se dibuja después de él).
    pub fn draw_flash(&self, r: &mut Renderer) {
        if let Stage::Open { t, .. } = self.stage {
            let a = 1.0 - smoothstep(t / FLASH_OUT);
            if a > 0.0 {
                r.quad(0.0, 0.0, 800.0, 600.0, [1.0, 0.95, 0.85, a]);
            }
        }
    }

    /// Avanza `dt` segundos: el bomber camina hacia la caja seleccionada, salen ascuas y sigue
    /// la secuencia de elegir. `at_root`: si el menú está en la raíz (al volver a ella, la caja
    /// abierta se rearma).
    pub fn update(
        &mut self,
        r: &mut Renderer,
        dt: f32,
        selection: usize,
        at_root: bool,
    ) -> Option<Event> {
        self.time += dt;
        let mut event = None;
        self.stage = match self.stage {
            Stage::Fuse { at, t } if t + dt >= FUSE => {
                self.burst_pending = true;
                Stage::Boom { at, t: 0.0 }
            }
            Stage::Fuse { at, t } => Stage::Fuse { at, t: t + dt },
            Stage::Boom { at, t } if t + dt >= BOOM => {
                event = Some(Event::Enter);
                Stage::Open { at, t: 0.0 }
            }
            Stage::Boom { at, t } => Stage::Boom { at, t: t + dt },
            Stage::Open { at, .. } if at_root => Stage::Return { at, t: 0.0 },
            Stage::Open { at, t } => Stage::Open { at, t: t + dt },
            Stage::Return { t, .. } if t + dt >= RETURN => Stage::Idle,
            Stage::Return { at, t } => Stage::Return { at, t: t + dt },
            Stage::Idle => Stage::Idle,
        };
        if std::mem::take(&mut self.burst_pending) {
            event = event.or(Some(Event::Exploded));
            self.emit_blast(r);
        }
        let goal = stand_x(selection);
        let dx = goal - self.x;
        if dx.abs() > 1e-3 {
            self.right = dx > 0.0;
            self.x += dx.signum() * (WALK_SPEED * dt).min(dx.abs());
            // Como `Bomber::act`: los fotogramas 1..8 en bucle.
            self.anim += dt * 20.0;
            if self.anim >= 9.0 || self.anim < 1.0 {
                self.anim = 1.0;
            }
        } else {
            self.x = goal;
            self.anim = 0.0;
        }

        let v = self.camera();
        // Llamas de las antorchas: destellos que suben.
        self.flame_acc += dt * 40.0;
        let flames = self.flame_acc.floor();
        self.flame_acc -= flames;
        for t in torches() {
            let Some(p) = v.project(t) else { continue };
            let s = v.scale_at(t) / 40.0;
            r.emit(&Burst {
                area: [5.0 * s, 2.0 * s],
                color: [3.0, 1.1, 0.25, 1.0],
                speed: [5.0, 25.0],
                life: [0.25, 0.55],
                drift: [0.0, -70.0 * s],
                size: 3.0 * s,
                ..Burst::new(ParticleKind::Glow, [p[0], p[1]], flames as u32)
            });
        }
        // Ascuas sueltas que suben desde la isla.
        self.ember_acc += dt * 14.0;
        let n = self.ember_acc.floor();
        self.ember_acc -= n;
        if n > 0.0 {
            r.emit(&Burst {
                area: [380.0, 120.0],
                color: [3.2, 1.0, 0.25, 1.0],
                speed: [5.0, 20.0],
                life: [1.5, 3.5],
                drift: [0.0, -45.0],
                size: 1.6,
                ..Burst::new(ParticleKind::Spark, [400.0, 420.0], n as u32)
            });
        }
        // Chispas de la mecha.
        if let Some(p) = v.project(self.bomb_fuse()).filter(|_| self.bomb_visible()) {
            r.emit(&Burst {
                color: [4.0, 2.0, 0.6, 1.0],
                speed: [30.0, 90.0],
                life: [0.1, 0.3],
                drift: [0.0, -30.0],
                size: 1.4,
                ..Burst::new(
                    ParticleKind::Spark,
                    [p[0], p[1]],
                    if matches!(self.stage, Stage::Fuse { .. }) {
                        7
                    } else {
                        2
                    },
                )
            });
        }
        r.advance_particles(dt);
        event
    }

    /// Chispas, humo y astillas de la caja que estalla.
    fn emit_blast(&self, r: &mut Renderer) {
        let Stage::Boom { at, .. } = self.stage else {
            return;
        };
        let v = self.camera();
        let c = box_center(at);
        let (Some(p), s) = (v.project(c), v.scale_at(c) / 40.0) else {
            return;
        };
        let pos = [p[0], p[1]];
        r.emit(&Burst {
            color: [4.0, 2.2, 0.7, 1.0],
            speed: [80.0 * s, 320.0 * s],
            life: [0.3, 0.9],
            size: 2.2 * s,
            ..Burst::new(ParticleKind::Spark, pos, 90)
        });
        r.emit(&Burst {
            area: [10.0 * s, 8.0 * s],
            color: [0.35, 0.3, 0.28, 0.7],
            speed: [20.0 * s, 70.0 * s],
            life: [0.6, 1.4],
            drift: [0.0, -30.0 * s],
            size: 9.0 * s,
            ..Burst::new(ParticleKind::Smoke, pos, 14)
        });
        r.emit(&Burst {
            color: [0.45, 0.3, 0.18, 1.0],
            speed: [60.0 * s, 220.0 * s],
            life: [0.4, 0.9],
            size: 2.5 * s,
            ..Burst::new(ParticleKind::Debris, pos, 24)
        });
    }

    /// Dónde está la bomba (en el suelo, entre el bomber y la caja).
    fn bomb_pos(&self) -> [f32; 3] {
        [self.x + 0.55, 0.0, BOX_ROW as f32 + 0.85]
    }

    /// La punta de la mecha.
    fn bomb_fuse(&self) -> [f32; 3] {
        let b = self.bomb_pos();
        [b[0] + 0.12, 0.62, b[2]]
    }

    pub fn draw(
        &self,
        r: &mut Renderer,
        a: &Assets,
        labels: &[&str],
        selection: usize,
        look: Look,
    ) {
        let (theme, skin) = (look.theme, look.skin);
        let v = self.camera();
        // El logo en llamas arriba, más chico que en el menú viejo.
        // Al zambullirse, el logo se apaga.
        let shown = 1.0 - self.dive().map_or(0.0, |(_, k)| k);
        r.sprite_whole(
            a.title_bg,
            [170.0, -40.0, 460.0, 345.0],
            [shown, shown, shown, 1.0],
        );

        r.set_view3d(v);
        for (i, t) in torches().into_iter().enumerate() {
            let f = flicker(self.time, i as f32);
            r.light3d(Light3d {
                pos: t,
                radius: 6.0,
                color: [2.2 * f, 1.0 * f, 0.35 * f],
            });
        }
        let f = flicker(self.time, 9.0);
        let fuse = self.bomb_fuse();
        if self.bomb_visible() {
            r.light3d(Light3d {
                pos: [fuse[0], 0.9, fuse[2] - 0.2],
                radius: 5.0,
                color: [4.5 * f, 2.6 * f, 0.9 * f],
            });
        }
        if let Stage::Boom { at, t } = self.stage {
            // El fogonazo alumbra todo el tablero y se apaga enseguida.
            let k = (1.0 - t / 0.45).max(0.0).powi(2);
            let c = box_center(at);
            r.light3d(Light3d {
                pos: [c[0], 1.2, c[2] + 0.3],
                radius: 7.0,
                color: [9.0 * k, 5.5 * k, 2.0 * k],
            });
        }

        let faces_of = BlockFaces::of(a, theme);
        let floor = &a.maptiles_3d;
        let cube =
            |tex, min: [f32; 3], size: [f32; 3], top, side, tint: [f32; 3], ao, faces| Cube {
                tex,
                min,
                size,
                top,
                side,
                tint: [tint[0], tint[1], tint[2], 1.0],
                ao,
                faces,
                spin: [0.0; 3],
            };
        let f = &faces_of;
        let edge = |x: usize, z: usize| x == 0 || z == 0 || x == W - 1 || z == D - 1;
        for z in 0..D {
            for x in 0..W {
                let (fx, fz) = (x as f32, z as f32);
                // La tierra de la isla: columnas de distinta hondura en el borde.
                if edge(x, z) {
                    let depth = 0.5 + 0.9 * hash(x, z);
                    r.cube(&cube(
                        f.tex,
                        [fx, -depth, fz],
                        [1.0, depth, 1.0],
                        f.earth,
                        f.earth,
                        f.earth_tint,
                        0.85,
                        faces::SIDES,
                    ));
                    r.cube(&cube(
                        f.tex,
                        [fx, 0.0, fz],
                        [1.0, 1.0, 1.0],
                        f.wall_top,
                        f.wall_side,
                        [1.0; 3],
                        0.4,
                        faces::ALL,
                    ));
                } else {
                    let shade = 0.9 + 0.1 * hash(z, x);
                    let cell = floor.cell(theme * 4);
                    r.cube(&cube(
                        floor.tex,
                        [fx, 0.0, fz],
                        [1.0, 0.0, 1.0],
                        cell,
                        cell,
                        [shade; 3],
                        0.0,
                        faces::TOP,
                    ));
                }
            }
        }
        let w = 1.0 - 2.0 * BOX_INSET;
        let broken = self.broken();
        for i in 0..labels.len() {
            let lit = if i == selection { 1.25 } else { 0.8 };
            let m = box_min(i);
            if let Some((_, t)) = broken.filter(|&(at, _)| at == i) {
                // En trozos: un octante de la caja por trozo, con su parte de cada textura.
                let quarter = |r: [f32; 4], qx: usize, qy: usize| {
                    [
                        r[0] + qx as f32 * r[2] / 2.0,
                        r[1] + qy as f32 * r[3] / 2.0,
                        r[2] / 2.0,
                        r[3] / 2.0,
                    ]
                };
                for n in 0..8 {
                    let (bi, bj, bk) = (n & 1, (n >> 1) & 1, (n >> 2) & 1);
                    let (d, spin) = shard(n, t);
                    let half = [w / 2.0, BOX_H / 2.0, w / 2.0];
                    let min = [
                        m[0] + bi as f32 * half[0] + d[0],
                        m[1] + bj as f32 * half[1] + d[1],
                        m[2] + bk as f32 * half[2] + d[2],
                    ];
                    r.cube(&Cube {
                        spin,
                        ..cube(
                            f.tex,
                            min,
                            half,
                            quarter(f.box_top, bi, bk),
                            quarter(f.box_side, bi, 1 - bj),
                            [lit; 3],
                            0.2,
                            faces::ALL | faces::BOTTOM,
                        )
                    });
                }
                continue;
            }
            r.cube(&cube(
                f.tex,
                m,
                [w, BOX_H, w],
                f.box_top,
                f.box_side,
                [lit; 3],
                0.35,
                faces::ALL,
            ));
        }

        self.draw_torch_glow(r, a, &v);
        self.draw_bomber(r, a, &v, skin);
        self.draw_blast(r, a, &v, look);
        if look.labels {
            self.draw_labels(r, a, &v, labels, selection);
        }
        r.particle_layer();
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        r.text(
            &a.font_small,
            &version,
            790.0,
            574.0,
            [1.0, 0.55, 0.2, 0.8],
            Align::Right,
        );
        let white = self.flash();
        if white > 0.0 {
            r.quad(0.0, 0.0, 800.0, 600.0, [1.0, 0.95, 0.85, white]);
        }
    }

    /// La caja en trozos, si hay una: cuál y a qué tiempo de su vuelo se dibujan los trozos.
    fn broken(&self) -> Option<(usize, f32)> {
        match self.stage {
            Stage::Boom { at, t } => Some((at, t)),
            Stage::Open { at, .. } => Some((at, SHARDS_LANDED)),
            Stage::Return { at, t } => Some((at, SHARDS_LANDED * (1.0 - smoothstep(t / RETURN)))),
            _ => None,
        }
    }

    /// La bomba está junto al bomber salvo desde que estalla hasta que la caja se rearma.
    fn bomb_visible(&self) -> bool {
        matches!(self.stage, Stage::Idle | Stage::Fuse { .. })
    }

    /// Fuego (por shader o con los sprites de siempre), onda expansiva y temblor del estallido.
    fn draw_blast(&self, r: &mut Renderer, a: &Assets, v: &View3d, fx: Look) {
        let Stage::Boom { at, t } = self.stage else {
            return;
        };
        let c = box_center(at);
        let (Some(p), tile) = (v.project(c), v.scale_at(c)) else {
            return;
        };
        let progress = (t / 0.55).min(1.0);
        if fx.shake {
            r.set_shake((1.0 - t / 0.3).max(0.0));
        }
        if fx.shockwaves {
            r.shockwave([p[0], p[1]], (t / 0.5).min(1.0), 1.2);
        }
        if progress >= 1.0 {
            return;
        }
        if fx.fire_shader {
            r.fire(&Fire {
                center: [p[0], p[1]],
                arms: [0; 4],
                tile: tile * 2.2,
                power: 8.0,
                progress,
                seed: at as u32 * 977 + 13,
            });
        } else {
            // Centro de `explosion2.png` (EXPLODE_X = 6), 3 fotogramas.
            let cell = 6 + 7 * ((progress * 3.0) as u32).min(2);
            let size = tile * 2.2;
            r.sprite(
                a.explosion.tex,
                a.explosion.cell(cell),
                [p[0] - size / 2.0, p[1] - size / 2.0, size, size],
            );
        }
    }

    /// La llama de cada antorcha: halos suaves superpuestos que bailan (con el bloom brillan).
    fn draw_torch_glow(&self, r: &mut Renderer, a: &Assets, v: &View3d) {
        for (i, t) in torches().into_iter().enumerate() {
            let (Some(p), s) = (v.project(t), v.scale_at(t) / 40.0) else {
                continue;
            };
            let f = flicker(self.time, i as f32);
            for (k, (w, h, c)) in [
                (60.0, 60.0, [1.0, 0.35, 0.06, 0.35]),
                (18.0, 30.0, [2.2, 0.7, 0.12, 0.9]),
                (10.0, 18.0, [3.5, 1.8, 0.5, 1.0]),
                (5.0, 9.0, [4.0, 3.4, 2.0, 1.0]),
            ]
            .into_iter()
            .enumerate()
            {
                let (w, h) = (w * s * f, h * s * f);
                let sway = 1.2 * s * (self.time * 6.0 + i as f32 + k as f32).sin();
                r.sprite_whole(
                    a.glow,
                    [p[0] + sway - w / 2.0, p[1] - h * 0.75, w, h],
                    [c[0] * f, c[1] * f, c[2] * f, c[3]],
                );
            }
        }
    }

    /// El bomber del jugador 1 y su bomba, de frente a la cámara.
    fn draw_bomber(&self, r: &mut Renderer, a: &Assets, v: &View3d, skin: usize) {
        let z = BOX_ROW as f32 + 0.5;
        let feet = [self.x, 0.0, z + 0.3];
        let (Some(p), s) = (v.project(feet), v.scale_at(feet) / 40.0) else {
            return;
        };
        let skin = skin.min(a.skins.len() - 1);
        let sheet = &a.skins[skin].sheet;
        // Apoyado algo por delante del centro de la casilla (`feet`), como en la partida.
        let (w, h) = (sheet.cell_w * s, sheet.cell_h * s);
        // Dirección del original: DOWN=0, LEFT=1, RIGHT=3.
        let dir = match (self.anim > 0.0, self.right) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 3,
        };
        let cell = dir * 10 + self.anim as u32;
        r.sprite(
            sheet.tex,
            sheet.cell(cell),
            [p[0] - w / 2.0, p[1] - h, w, h],
        );

        let b = self.bomb_pos();
        let (Some(bp), bs) = (v.project(b), v.scale_at(b) / 40.0) else {
            return;
        };
        if !self.bomb_visible() {
            return;
        }
        // Late como las bombas de la partida (`draw.rs`); con la mecha encendida, cada vez más
        // rápido y fuerte, y destella.
        let nervous = match self.stage {
            Stage::Fuse { t, .. } => t / FUSE,
            _ => 0.0,
        };
        let beat = (self.time * std::f32::consts::TAU * (1.6 + 8.0 * nervous)).sin();
        let amp = 0.05 + 0.12 * nervous;
        let (sx, sy) = (1.0 + amp * beat, 1.0 - amp * beat);
        let (bw, bh) = (
            a.bombs.cell_w * bs * 0.8 * sx,
            a.bombs.cell_h * bs * 0.8 * sy,
        );
        let frame = (self.time * 8.0) as u32 % 4;
        let src = a.bombs.cell(skin as u32 * 4 + frame);
        let dst = [bp[0] - bw / 2.0, bp[1] - bh * 0.92, bw, bh];
        r.sprite(a.bombs.tex, src, dst);
        if nervous > 0.0 {
            let flash = nervous * (0.5 + 0.5 * beat).powi(4) * 0.8;
            r.sprite_silhouette(a.bombs.tex, src, dst, [2.0, 2.0, 2.0, flash]);
        }
    }

    /// El nombre de cada opción sobre su caja, derecho y de frente.
    fn draw_labels(
        &self,
        r: &mut Renderer,
        a: &Assets,
        v: &View3d,
        labels: &[&str],
        selection: usize,
    ) {
        let step = v.scale_at([W as f32 / 2.0, 1.0, BOX_ROW as f32]) * BOX_STEP as f32;
        let broken = self.broken().map(|(at, _)| at);
        let alpha = 1.0 - self.dive().map_or(0.0, |(_, k)| k);
        if alpha <= 0.0 {
            return;
        }
        for (i, label) in labels.iter().enumerate() {
            if broken == Some(i) {
                continue;
            }
            let top = [
                (BOX_X0 + BOX_STEP * i) as f32 + 0.5,
                1.0,
                BOX_ROW as f32 + 0.5,
            ];
            let Some(p) = v.project(top) else { continue };
            let selected = i == selection;
            let font = if selected { &a.font_big } else { &a.font_small };
            // Si no cabe entre dos cajas, en dos líneas.
            let lines: Vec<&str> = match label.split_once(' ') {
                Some((l1, l2)) if font.width(label) > step * 0.95 => vec![l1, l2],
                _ => vec![label],
            };
            let lh = font.line_height() * 0.9;
            let y0 = p[1] - 8.0 - lh * lines.len() as f32;
            let (color, style) = if selected {
                (
                    [1.0, 0.85, 0.4, 1.0],
                    TextStyle {
                        outline: Some((2.0, [0.35, 0.05, 0.0, 1.0])),
                        shadow: Some(Shadow {
                            offset: [0.0, 0.0],
                            blur: 9.0,
                            color: [1.0, 0.35, 0.0, 0.9],
                        }),
                    },
                )
            } else {
                (
                    [0.82, 0.82, 0.8, 0.9],
                    TextStyle {
                        outline: Some((1.5, [0.0, 0.0, 0.0, 0.9])),
                        shadow: TextStyle::DEFAULT.shadow,
                    },
                )
            };
            // El alfa del texto también apaga su contorno y su sombra.
            let color = [color[0], color[1], color[2], color[3] * alpha];
            for (k, line) in lines.iter().enumerate() {
                r.text_styled(
                    font,
                    line,
                    p[0],
                    y0 + lh * k as f32,
                    color,
                    Align::Center,
                    style,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_lights_then_explodes_then_finishes() {
        let mut b = Board::new(0);
        assert_eq!(b.key(KeyCode::Enter, 2), KeyAction::Lit);
        assert_eq!(b.stage, Stage::Fuse { at: 2, t: 0.0 });
        // Enter otra vez no espera la mecha.
        assert_eq!(b.key(KeyCode::Enter, 2), KeyAction::Consumed);
        assert!(matches!(b.stage, Stage::Boom { at: 2, .. }));
        // Durante la zambullida cualquier tecla entra ya.
        assert_eq!(b.key(KeyCode::ArrowLeft, 2), KeyAction::Finish);
        assert!(matches!(b.stage, Stage::Open { at: 2, .. }));
    }

    #[test]
    fn moving_cancels_the_fuse() {
        let mut b = Board::new(0);
        b.key(KeyCode::Enter, 0);
        assert_eq!(b.key(KeyCode::ArrowRight, 0), KeyAction::Pass);
        assert_eq!(b.stage, Stage::Idle);
    }

    #[test]
    fn shards_start_in_place_and_stay_on_the_floor() {
        for n in 0..8 {
            let (d, spin) = shard(n, 0.0);
            assert_eq!((d, spin), ([0.0; 3], [0.0; 3]));
            let (landed, _) = shard(n, SHARDS_LANDED);
            assert_eq!(shard(n, SHARDS_LANDED + 5.0).0, landed, "ya cayó");
            // Termina con su base en el suelo (y = 0).
            let base = (n >> 1 & 1) as f32 * BOX_H / 2.0 + landed[1];
            assert!(base.abs() < 1e-3, "trozo {n}: {base}");
        }
    }
}
