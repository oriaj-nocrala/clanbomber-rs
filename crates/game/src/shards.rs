//! Trozos con textura y "pops" de extras recogidos. Son pocas piezas, así que se simulan aquí
//! (en la CPU) para que cada una lleve su trozo del sprite original y gire. Con una altura falsa
//! (`z`): saltan, rebotan en el suelo y se apagan. Puro adorno: no toca el mundo.

use cb_render::{Renderer, Sheet};

use crate::assets::Assets;

/// De qué hoja sale un trozo.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    MapTile,
    Extra,
}

struct Shard {
    source: Source,
    /// Región del sprite (px de la textura).
    src: [f32; 4],
    /// Tamaño en pantalla (px virtuales): con hojas HD no coincide con el de `src`.
    size: [f32; 2],
    /// Posición en el suelo (centro, px virtuales) y altura.
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    angle: f32,
    spin: f32,
    age: f32,
    life: f32,
    bounces: u8,
}

/// Extra recogido: el icono crece, sube y se apaga brillando.
struct Pop {
    cell: u32,
    x: f32,
    y: f32,
    age: f32,
    /// Dorado para los buenos, verdoso para las enfermedades.
    good: bool,
}

const GRAVITY: f32 = 900.0;
const POP_LIFE: f32 = 0.4;

#[derive(Default)]
pub struct Shards {
    shards: Vec<Shard>,
    pops: Vec<Pop>,
    seed: u32,
}

impl Shards {
    /// Aleatorio local (xorshift): no comparte estado con el `Rng` del mundo.
    fn rand(&mut self) -> f32 {
        let mut x = self.seed.wrapping_add(0x9e37_79b9) | 1;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn clear(&mut self) {
        self.shards.clear();
        self.pops.clear();
    }

    /// Parte la celda `cell` de `sheet` (dibujada en `origin`, esquina superior izquierda) en
    /// `n`x`n` trozos que saltan hacia fuera.
    pub fn shatter(&mut self, source: Source, sheet: &Sheet, cell: u32, origin: [f32; 2], n: u32) {
        let base = sheet.cell(cell);
        let (pw, ph) = (base[2] / n as f32, base[3] / n as f32);
        let (sw, sh) = (sheet.cell_w / n as f32, sheet.cell_h / n as f32);
        let (cx, cy) = (
            origin[0] + sheet.cell_w / 2.0,
            origin[1] + sheet.cell_h / 2.0,
        );
        for j in 0..n {
            for i in 0..n {
                let (x, y) = (
                    origin[0] + (i as f32 + 0.5) * sw,
                    origin[1] + (j as f32 + 0.5) * sh,
                );
                // Hacia fuera desde el centro, con algo de azar.
                let (dx, dy) = (x - cx, y - cy);
                let len = (dx * dx + dy * dy).sqrt().max(1.0);
                let speed = 30.0 + 90.0 * self.rand();
                let jitter = (self.rand() - 0.5) * 60.0;
                let life = 0.8 + 0.5 * self.rand();
                let spin = (self.rand() - 0.5) * 18.0;
                let vz = 120.0 + 160.0 * self.rand();
                self.shards.push(Shard {
                    source,
                    src: [base[0] + i as f32 * pw, base[1] + j as f32 * ph, pw, ph],
                    size: [sw, sh],
                    x,
                    y,
                    z: 0.0,
                    vx: dx / len * speed + jitter,
                    vy: dy / len * speed - jitter,
                    vz,
                    angle: 0.0,
                    spin,
                    age: 0.0,
                    life,
                    bounces: 0,
                });
            }
        }
    }

    pub fn pop(&mut self, cell: u32, pos: [f32; 2], good: bool) {
        self.pops.push(Pop {
            cell,
            x: pos[0],
            y: pos[1],
            age: 0.0,
            good,
        });
    }

    pub fn update(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        for s in &mut self.shards {
            s.age += dt;
            if s.bounces >= 2 {
                continue;
            }
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.vz -= GRAVITY * dt;
            s.z += s.vz * dt;
            s.angle += s.spin * dt;
            if s.z < 0.0 {
                // Rebote amortiguado; al segundo se queda en el suelo.
                s.z = 0.0;
                s.vz = -s.vz * 0.35;
                s.vx *= 0.5;
                s.vy *= 0.5;
                s.spin *= 0.4;
                s.bounces += 1;
            }
        }
        self.shards.retain(|s| s.age < s.life);
        self.pops.iter_mut().for_each(|p| p.age += dt);
        self.pops.retain(|p| p.age < POP_LIFE);
    }

    /// Los trozos, con su sombra en el suelo (antes de la iluminación: los alumbra).
    pub fn draw_shards(&self, r: &mut Renderer, a: &Assets) {
        for s in &self.shards {
            let fade = ((s.life - s.age) / 0.3).clamp(0.0, 1.0);
            // Sombra: más pequeña y tenue cuanto más alto está el trozo.
            let k = (1.0 - s.z / 80.0).clamp(0.3, 1.0);
            r.quad(
                s.x - s.size[0] * 0.4 * k,
                s.y + s.size[1] * 0.3,
                s.size[0] * 0.8 * k,
                2.0,
                [0.0, 0.0, 0.0, 0.35 * k * fade],
            );
            let sheet = match s.source {
                Source::MapTile => &a.maptiles,
                Source::Extra => &a.extras,
            };
            r.sprite_rotated(
                sheet.tex,
                s.src,
                [s.x, s.y - s.z],
                s.size,
                s.angle,
                [1.0, 1.0, 1.0, fade],
            );
        }
    }

    /// Los extras recogidos (después de la iluminación: brillan por sí mismos).
    pub fn draw_pops(&self, r: &mut Renderer, a: &Assets) {
        for p in &self.pops {
            let t = p.age / POP_LIFE;
            let scale = 1.0 + 0.6 * t;
            let size = 40.0 * scale;
            let (cx, cy) = (p.x + 20.0, p.y + 20.0 - 22.0 * t);
            let src = a.extras.cell(p.cell);
            let fade = (1.0 - t) * (1.0 - t);
            // Sobreexpuesto (> 1) y teñido del color del extra: el bloom lo hace resplandecer.
            let boost = 1.0 + 1.6 * (1.0 - t);
            let tint = if p.good {
                [boost, boost * 0.9, boost * 0.55, fade]
            } else {
                [boost * 0.6, boost, boost * 0.5, fade]
            };
            r.sprite_tinted(
                a.extras.tex,
                src,
                [cx - size / 2.0, cy - size / 2.0, size, size],
                tint,
            );
        }
    }
}
