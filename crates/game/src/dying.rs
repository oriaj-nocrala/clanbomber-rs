//! Transición de la muerte. En el original el bomber pasa de un fotograma a otro al sprite del
//! cadáver (con la sangre pintada). Aquí destella, se quema con un borde incandescente y el
//! cadáver aparece por los mismos píxeles; luego una silueta translúcida sube y se desvanece.
//! Puro adorno: la simulación ya tiene el cadáver desde el primer tick.

use cb_core::{Object, Phase, World};
use cb_render::Renderer;

use crate::assets::Assets;
use crate::draw::Player;

/// Desplazamiento de dibujo de los bombers (el de `draw.rs`).
const BOMBER_OFFSET_X: f32 = 60.0;
/// Destello blanco del impacto.
const FLASH: f32 = 0.06;
/// La quemadura empieza con el destello y dura hasta aquí.
const BURN_END: f32 = 0.5;
/// La silueta que sube.
const SOUL_START: f32 = 0.4;
const SOUL_END: f32 = 1.5;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Un bomber que acaba de morir: con qué aspecto y dónde estaba en su último tick vivo.
struct Dying {
    owner: usize,
    sprite: u32,
    x: f32,
    y: f32,
    age: f32,
}

#[derive(Default)]
pub struct Deaths {
    list: Vec<Dying>,
    /// Último aspecto y posición de cada bomber vivo (`None` si no lo estaba o caía a un hoyo).
    alive: Vec<Option<(u32, f32, f32)>>,
}

impl Deaths {
    /// Detecta las muertes nuevas y avanza `dt` segundos de juego.
    pub fn update(&mut self, w: &World, dt: f32) {
        if !matches!(w.phase, Phase::Playing) {
            self.list.clear();
            self.alive.clear();
            return;
        }
        self.alive.resize(w.bombers.len(), None);
        for (i, b) in w.bombers.iter().enumerate() {
            if !b.dead {
                // Quien cae a un hoyo ya tiene su animación: no se quema.
                let visible = !b.is_falling() && !b.flight.active;
                self.alive[i] = visible.then(|| (b.sprite_nr(), b.x, b.y));
            } else if let Some((sprite, x, y)) = self.alive[i].take() {
                self.list.push(Dying {
                    owner: i,
                    sprite,
                    x,
                    y,
                    age: 0.0,
                });
            }
        }
        for d in &mut self.list {
            d.age += dt;
        }
        self.list.retain(|d| d.age < SOUL_END);
    }

    /// El cadáver de `owner` todavía está apareciendo (no se dibuja tal cual).
    pub fn burning(&self, owner: usize) -> bool {
        self.list
            .iter()
            .any(|d| d.owner == owner && d.age < BURN_END)
    }

    /// Dónde dibujar: sobre el cadáver si lo hay (se arrastra), si no donde murió.
    fn place(d: &Dying, w: &World) -> (f32, f32) {
        w.objects
            .iter()
            .find_map(|o| match o {
                Object::Corpse(c) if c.owner == d.owner && !c.falling => Some((c.x, c.y)),
                _ => None,
            })
            .unwrap_or((d.x, d.y))
    }

    /// El bomber quemándose y su cadáver apareciendo (en la capa de los cadáveres).
    pub fn draw_burns(&self, r: &mut Renderer, a: &Assets, w: &World, players: &[Player]) {
        for d in self.list.iter().filter(|d| d.age < BURN_END) {
            let skin = &a.skins[players[d.owner].skin];
            let sheet = &skin.sheet;
            let (x, y) = Self::place(d, w);
            let dst = [
                x + BOMBER_OFFSET_X,
                y + skin.offset_y,
                sheet.cell_w,
                sheet.cell_h,
            ];
            let progress = smoothstep(FLASH * 0.5, BURN_END, d.age);
            // Arde desde el centro del cuerpo (algo por debajo: el pecho, no la cabeza).
            let center = [dst[0] + dst[2] / 2.0, dst[1] + dst[3] * 0.6];
            let radius = dst[2].max(dst[3]) * 0.6;
            for o in &w.objects {
                if let Object::Corpse(c) = o {
                    if c.owner == d.owner && !c.falling {
                        let src = sheet.cell(c.sprite_nr);
                        r.sprite_reveal(sheet.tex, src, dst, progress, center, radius, 1.0);
                    }
                }
            }
            // El golpe aplasta un poco el sprite (apoyado en los pies) mientras destella.
            let squash = 1.0 - smoothstep(0.0, FLASH * 1.5, d.age);
            let (sw, sh) = (
                dst[2] * (1.0 + 0.14 * squash),
                dst[3] * (1.0 - 0.1 * squash),
            );
            let body = [dst[0] - (sw - dst[2]) / 2.0, dst[1] + dst[3] - sh, sw, sh];
            let src = sheet.cell(d.sprite);
            r.sprite_burn(sheet.tex, src, body, progress, center, radius);
            if d.age < FLASH {
                let k = 1.0 - d.age / FLASH;
                r.sprite_silhouette(sheet.tex, src, body, [2.0, 2.0, 2.0, k]);
            }
        }
    }

    /// La silueta que sube (después de la iluminación: brilla sola).
    pub fn draw_souls(&self, r: &mut Renderer, a: &Assets, w: &World, players: &[Player]) {
        for d in self.list.iter().filter(|d| d.age > SOUL_START) {
            let t = (d.age - SOUL_START) / (SOUL_END - SOUL_START);
            let skin = &a.skins[players[d.owner].skin];
            let sheet = &skin.sheet;
            let (x, y) = Self::place(d, w);
            // Sube frenando, encoge y se mece.
            let rise = 55.0 * (1.0 - (1.0 - t) * (1.0 - t));
            let scale = 1.0 - 0.3 * t;
            let (sw, sh) = (sheet.cell_w * scale, sheet.cell_h * scale);
            let cx = x + BOMBER_OFFSET_X + sheet.cell_w / 2.0 + 3.0 * (d.age * 7.0).sin();
            let bottom = y + skin.offset_y + sheet.cell_h - rise;
            let alpha = 0.3 * smoothstep(0.0, 0.2, t) * (1.0 - t).powi(2);
            r.sprite_silhouette(
                sheet.tex,
                sheet.cell(d.sprite),
                [cx - sw / 2.0, bottom - sh, sw, sh],
                [1.0, 1.15, 1.45, alpha],
            );
        }
    }
}
