//! Clima según el tema del mapa: nieve, hojas, esporas, ceniza, ascuas o motas de polvo. Son
//! pocas piezas, así que se simulan aquí (en la CPU) y se dibujan antes de la iluminación: la luz
//! de las explosiones y de los bombers las alumbra. Las ascuas brillan solas y van después. Las
//! que caen tienen una altura falsa: su sombra se acerca hasta que tocan el suelo y se deshacen.
//! Puro adorno: no toca el mundo.

use cb_render::Renderer;

use crate::draw::srgb_rgba;

/// Punto de `s` px; desde 3 px, con las esquinas recortadas (redondo en pixel art).
fn dot(r: &mut Renderer, [x, y]: [f32; 2], s: f32, color: [f32; 4]) {
    if s >= 3.0 {
        r.quad(x - s / 2.0, y - s / 2.0 + 1.0, s, s - 2.0, color);
        r.quad(x - s / 2.0 + 1.0, y - s / 2.0, s - 2.0, 1.0, color);
        r.quad(x - s / 2.0 + 1.0, y + s / 2.0 - 1.0, s - 2.0, 1.0, color);
    } else {
        r.quad(x - s / 2.0, y - s / 2.0, s, s, color);
    }
}

/// Rectángulo del mapa en el lienzo virtual (offset 60, 40 del original).
const MAP: [f32; 4] = [60.0, 40.0, 680.0, 520.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Snow,
    Leaves,
    Spores,
    Ash,
    Embers,
    Motes,
}

impl Kind {
    /// Por tema de `maptiles.png`: nieve, jungla, piedra con musgo, barriles, infierno y metal.
    fn of(theme: i32) -> Kind {
        match theme.clamp(0, 5) {
            0 => Kind::Snow,
            1 => Kind::Leaves,
            2 => Kind::Spores,
            3 => Kind::Ash,
            4 => Kind::Embers,
            _ => Kind::Motes,
        }
    }

    fn count(self) -> usize {
        match self {
            Kind::Snow => 230,
            Kind::Leaves => 30,
            Kind::Spores => 80,
            Kind::Ash => 90,
            Kind::Embers => 55,
            Kind::Motes => 110,
        }
    }

    /// Cae desde arriba (tiene altura y sombra).
    fn falls(self) -> bool {
        matches!(self, Kind::Snow | Kind::Leaves | Kind::Ash)
    }
}

struct Flake {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// Profundidad: 1 es lo más cercano (más grande, rápido y opaco).
    z: f32,
    age: f32,
    life: f32,
    /// Vaivén lateral: fase, frecuencia (rad/s) y amplitud (px).
    phase: f32,
    freq: f32,
    sway: f32,
    /// Giro (hojas) y volteo (el ancho de la hoja oscila como si diera vueltas).
    angle: f32,
    spin: f32,
    flip: f32,
    color: [f32; 4],
}

#[derive(Default)]
pub struct Weather {
    kind: Option<Kind>,
    flakes: Vec<Flake>,
    seed: u32,
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Weather {
    /// Aleatorio local (xorshift): no comparte estado con el `Rng` del mundo.
    fn rand(&mut self) -> f32 {
        let mut x = self.seed.wrapping_add(0x9e37_79b9) | 1;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rand()
    }

    /// Una pieza nueva en un sitio al azar del mapa. Con `prewarm` empieza a media vida, para
    /// que la ronda no arranque con el cielo vacío.
    fn spawn(&mut self, kind: Kind, prewarm: bool) -> Flake {
        let z = self.rand();
        let (vx, vy, life, freq, sway) = match kind {
            Kind::Snow => (
                8.0 + 10.0 * z,
                16.0 + 34.0 * z,
                self.range(4.0, 9.0),
                self.range(0.8, 1.7),
                4.0 + 9.0 * z,
            ),
            Kind::Leaves => (
                16.0 + 14.0 * z,
                10.0 + 14.0 * z,
                self.range(5.0, 10.0),
                self.range(0.5, 0.9),
                10.0 + 8.0 * z,
            ),
            Kind::Ash => (
                20.0 + 16.0 * z,
                7.0 + 10.0 * z,
                self.range(4.0, 8.0),
                self.range(0.7, 1.4),
                5.0 + 4.0 * z,
            ),
            Kind::Spores => (
                self.range(-5.0, 5.0),
                self.range(-6.0, 1.0),
                self.range(4.0, 8.0),
                self.range(0.4, 0.9),
                3.0 + 3.0 * z,
            ),
            Kind::Embers => (
                self.range(-6.0, 12.0),
                -(22.0 + 34.0 * z),
                self.range(1.8, 4.5),
                self.range(1.5, 3.2),
                5.0 + 6.0 * z,
            ),
            Kind::Motes => (
                self.range(-4.0, 4.0),
                self.range(-3.0, 3.0),
                self.range(5.0, 10.0),
                self.range(0.3, 0.7),
                2.0 + 3.0 * z,
            ),
        };
        let color = match kind {
            // Por encima de 1: con la luz ambiente (~0.5) queda blanco y junto al fuego brilla.
            Kind::Snow => [2.0, 2.05, 2.2, 0.7 + 0.3 * z],
            Kind::Leaves => {
                // Hojas secas: contrastan con el césped. Más brillantes de la cuenta porque la
                // luz ambiente las oscurece.
                let palette = [
                    (236, 180, 48),
                    (220, 116, 34),
                    (190, 200, 70),
                    (176, 96, 40),
                ];
                let (r, g, b) = palette[(self.rand() * 3.99) as usize];
                let c = srgb_rgba(r, g, b, 255);
                [c[0] * 1.8, c[1] * 1.8, c[2] * 1.8, 1.0]
            }
            Kind::Spores => [1.5, 2.1, 0.8, 0.55 + 0.35 * z],
            Kind::Ash => {
                let v = self.range(0.5, 1.1);
                [v, v, v * 0.92, 0.6 + 0.35 * z]
            }
            // Fuego en color lineal "HDR": por encima de 1, para que el bloom lo recoja.
            Kind::Embers => [3.4, 1.2, 0.3, 1.0],
            Kind::Motes => [2.6, 2.5, 2.3, 0.45 + 0.4 * z],
        };
        let life_left = if prewarm { self.rand() } else { 0.0 };
        Flake {
            x: self.range(MAP[0], MAP[0] + MAP[2]),
            y: self.range(MAP[1], MAP[1] + MAP[3]),
            vx,
            vy,
            z,
            age: life * life_left,
            life,
            phase: self.range(0.0, std::f32::consts::TAU),
            freq,
            sway,
            angle: self.range(0.0, std::f32::consts::TAU),
            spin: self.range(-2.5, 2.5),
            flip: self.range(2.0, 4.5),
            color,
        }
    }

    /// Avanza `dt` segundos. Cambia de clima si cambió el tema.
    pub fn update(&mut self, theme: i32, dt: f32) {
        let kind = Kind::of(theme);
        if self.kind != Some(kind) {
            self.kind = Some(kind);
            self.flakes = (0..kind.count()).map(|_| self.spawn(kind, true)).collect();
        }
        if dt <= 0.0 {
            return;
        }
        for i in 0..self.flakes.len() {
            let f = &mut self.flakes[i];
            f.age += dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.angle += f.spin * dt;
            let out = f.x < MAP[0] - 20.0
                || f.x > MAP[0] + MAP[2] + 20.0
                || f.y < MAP[1] - 20.0
                || f.y > MAP[1] + MAP[3] + 20.0;
            if f.age >= f.life || out {
                self.flakes[i] = self.spawn(kind, false);
            }
        }
    }

    /// Posición con el vaivén, opacidad (aparece, se deshace y se recorta en el borde del mapa)
    /// y altura sobre el suelo.
    fn place(&self, f: &Flake) -> ([f32; 2], f32, f32) {
        let x = f.x + f.sway * (f.phase + f.age * f.freq).sin();
        let y = f.y;
        let fade = smoothstep(0.0, 0.7, f.age) * (1.0 - smoothstep(f.life - 1.0, f.life, f.age));
        let edge = smoothstep(MAP[0], MAP[0] + 10.0, x)
            * (1.0 - smoothstep(MAP[0] + MAP[2] - 10.0, MAP[0] + MAP[2], x))
            * smoothstep(MAP[1], MAP[1] + 10.0, y)
            * (1.0 - smoothstep(MAP[1] + MAP[3] - 10.0, MAP[1] + MAP[3], y));
        let height = if self.kind.is_some_and(Kind::falls) {
            (1.0 - f.age / f.life) * (12.0 + 40.0 * f.z)
        } else {
            0.0
        };
        ([x, y], fade * edge, height)
    }

    /// Lo que la luz alumbra (antes de la iluminación).
    pub fn draw_lit(&self, r: &mut Renderer) {
        let Some(kind) = self.kind.filter(|&k| k != Kind::Embers) else {
            return;
        };
        for f in &self.flakes {
            let ([x, y], a, h) = self.place(f);
            if a <= 0.0 {
                continue;
            }
            // Sombra hacia abajo a la derecha, más lejos cuanto más alto.
            let shadow = [x + h * 0.4, y + h * 0.7];
            match kind {
                Kind::Leaves => {
                    let w = (9.0 + 4.0 * f.z) * (f.phase + f.age * f.flip).cos().abs().max(0.2);
                    let size = [w, 5.0 + 2.0 * f.z];
                    r.quad_rotated(shadow, size, f.angle, [0.0, 0.0, 0.0, 0.3 * a]);
                    let c = f.color;
                    r.quad_rotated([x, y], size, f.angle, [c[0], c[1], c[2], c[3] * a]);
                }
                _ => {
                    let s = match kind {
                        Kind::Snow => (2.0 + 2.0 * f.z).round(),
                        Kind::Motes => (1.0 + f.z).round(),
                        _ => (2.0 + 1.5 * f.z).round(),
                    };
                    let mut c = f.color;
                    if matches!(kind, Kind::Motes | Kind::Spores) {
                        // Destellan al girar en el aire.
                        c[3] *= 0.75 + 0.25 * (f.phase * 3.0 + f.age * f.freq * 4.0).sin();
                    }
                    if kind.falls() {
                        dot(r, shadow, s, [0.0, 0.0, 0.0, 0.3 * a]);
                    }
                    dot(r, [x, y], s, [c[0], c[1], c[2], c[3] * a]);
                }
            }
        }
    }

    /// Lo que brilla por sí mismo (después de la iluminación): las ascuas.
    pub fn draw_glow(&self, r: &mut Renderer) {
        if self.kind != Some(Kind::Embers) {
            return;
        }
        for f in &self.flakes {
            let ([x, y], a, _) = self.place(f);
            if a <= 0.0 {
                continue;
            }
            // Parpadean y se enfrían (más rojas) al final.
            let t = f.age / f.life;
            let flicker = 0.75 + 0.25 * (f.phase * 7.0 + f.age * 19.0).sin();
            let s = if f.z > 0.35 { 2.0 } else { 1.0 };
            let c = f.color;
            r.quad(
                x - s / 2.0,
                y - s / 2.0,
                s,
                s,
                [
                    c[0] * flicker,
                    c[1] * flicker * (1.0 - 0.6 * t),
                    c[2] * (1.0 - t),
                    a,
                ],
            );
        }
    }
}
