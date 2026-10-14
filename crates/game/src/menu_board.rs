//! Menú raíz como tablero 3D (`docs/menu-3d.md`): una isla de ClanBomber flotando en la
//! oscuridad con una caja por opción, el bomber del jugador 1 como cursor y una bomba encendida a
//! su lado. Los cubos van por la escena 3D del renderer; el bomber, la bomba y los nombres son
//! sprites y texto 2D en la posición proyectada.

use cb_render::{
    faces, Align, Burst, Cube, Light3d, ParticleKind, Renderer, Shadow, TextStyle, View3d,
};

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

impl Board {
    pub fn new(selection: usize) -> Self {
        Board {
            x: stand_x(selection),
            anim: 0.0,
            right: true,
            time: 0.0,
            ember_acc: 0.0,
            flame_acc: 0.0,
        }
    }

    /// Avanza `dt` segundos: el bomber camina hacia la caja seleccionada y salen ascuas.
    pub fn update(&mut self, r: &mut Renderer, dt: f32, selection: usize) {
        self.time += dt;
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

        let v = view(self.time);
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
        if let Some(p) = v.project(self.bomb_fuse()) {
            r.emit(&Burst {
                color: [4.0, 2.0, 0.6, 1.0],
                speed: [30.0, 90.0],
                life: [0.1, 0.3],
                drift: [0.0, -30.0],
                size: 1.4,
                ..Burst::new(ParticleKind::Spark, [p[0], p[1]], 2)
            });
        }
        r.advance_particles(dt);
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
        theme: u32,
        skin: usize,
    ) {
        let v = view(self.time);
        // El logo en llamas arriba, más chico que en el menú viejo.
        r.sprite_whole(a.title_bg, [170.0, -40.0, 460.0, 345.0], [1.0; 4]);

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
        r.light3d(Light3d {
            pos: [fuse[0], 0.9, fuse[2] - 0.2],
            radius: 5.0,
            color: [4.5 * f, 2.6 * f, 0.9 * f],
        });

        let tiles = &a.maptiles;
        let cell = |n: u32| tiles.cell(theme * 4 + n);
        let cube = |min: [f32; 3], size: [f32; 3], top, side, tint: [f32; 3], ao, faces| Cube {
            tex: tiles.tex,
            min,
            size,
            top,
            side,
            tint: [tint[0], tint[1], tint[2], 1.0],
            ao,
            faces,
        };
        let edge = |x: usize, z: usize| x == 0 || z == 0 || x == W - 1 || z == D - 1;
        for z in 0..D {
            for x in 0..W {
                let (fx, fz) = (x as f32, z as f32);
                // La tierra de la isla: columnas de distinta hondura en el borde.
                if edge(x, z) {
                    let depth = 0.5 + 0.9 * hash(x, z);
                    r.cube(&cube(
                        [fx, -depth, fz],
                        [1.0, depth, 1.0],
                        cell(1),
                        cell(1),
                        [0.42, 0.33, 0.27],
                        0.85,
                        faces::SIDES,
                    ));
                    r.cube(&cube(
                        [fx, 0.0, fz],
                        [1.0, 1.0, 1.0],
                        cell(1),
                        cell(1),
                        [1.0; 3],
                        0.4,
                        faces::ALL,
                    ));
                } else {
                    let shade = 0.9 + 0.1 * hash(z, x);
                    r.cube(&cube(
                        [fx, 0.0, fz],
                        [1.0, 0.0, 1.0],
                        cell(0),
                        cell(0),
                        [shade; 3],
                        0.0,
                        faces::TOP,
                    ));
                }
            }
        }
        let inset = 0.07;
        for i in 0..labels.len() {
            let x = (BOX_X0 + BOX_STEP * i) as f32;
            let lit = if i == selection { 1.25 } else { 0.8 };
            r.cube(&cube(
                [x + inset, 0.0, BOX_ROW as f32 + inset],
                [1.0 - 2.0 * inset, 0.86, 1.0 - 2.0 * inset],
                cell(2),
                cell(2),
                [lit; 3],
                0.35,
                faces::ALL,
            ));
        }

        self.draw_torch_glow(r, a, &v);
        self.draw_bomber(r, a, &v, skin);
        self.draw_labels(r, a, &v, labels, selection);
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
        // Late como las bombas de la partida (`draw.rs`), sin acabarse nunca.
        let beat = (self.time * std::f32::consts::TAU * 1.6).sin();
        let (sx, sy) = (1.0 + 0.05 * beat, 1.0 - 0.05 * beat);
        let (bw, bh) = (
            a.bombs.cell_w * bs * 0.8 * sx,
            a.bombs.cell_h * bs * 0.8 * sy,
        );
        let frame = (self.time * 8.0) as u32 % 4;
        r.sprite(
            a.bombs.tex,
            a.bombs.cell(skin as u32 * 4 + frame),
            [bp[0] - bw / 2.0, bp[1] - bh * 0.92, bw, bh],
        );
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
        for (i, label) in labels.iter().enumerate() {
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
