//! Los submenús como letreros de madera sobre el tablero 3D (`docs/menu-3d.md`, fase 3). Al
//! cambiar de submenú, el letrero viejo gira como una carta hasta ponerse de canto y el nuevo
//! entra girando desde el otro lado; el texto aparece cuando ya está de frente.

use std::f32::consts::FRAC_PI_2;

use cb_render::{Align, Renderer, Shadow, TextStyle};

use crate::assets::Assets;
use crate::menu::Page;

/// Segundos que tarda el letrero en irse (hasta quedar de canto) y en llegar.
const OUT: f32 = 0.15;
const IN: f32 = 0.32;
/// Cuánto se achica lo que se aleja al girar (por píxel de distancia al eje).
const PERSPECTIVE: f32 = 0.0009;
/// Marco de `sign.png`: 64 texels en la textura (16 de 96, a 4x), dibujado a `BORDER` px.
const BORDER: f32 = 22.0;
const BORDER_TEXELS: f32 = 64.0;
/// Margen entre el marco y el texto, alto extra por fila y hueco para la bomba que señala.
const PAD: f32 = 18.0;
const ROW_GAP: f32 = 8.0;
const CURSOR: f32 = 44.0;
/// Ancho máximo del letrero.
const MAX_WIDTH: f32 = 780.0;
/// Opacidad del velo que oscurece el tablero detrás del letrero.
const DIM: f32 = 0.5;

pub struct Sign {
    page: Option<Page>,
    /// Segundos desde que se pidió la página actual.
    t: f32,
    /// Cuánto espera la página actual a que termine de irse la anterior.
    delay: f32,
    leaving: Option<(Page, f32)>,
    /// Sentido del giro: 1 al bajar a un submenú, -1 al subir.
    dir: f32,
}

impl Sign {
    pub fn new() -> Self {
        Sign {
            page: None,
            t: 0.0,
            delay: 0.0,
            leaving: None,
            dir: 1.0,
        }
    }

    /// `page`: el submenú abierto, o `None` en la raíz (el tablero).
    pub fn update(&mut self, dt: f32, page: Option<Page>) {
        let old = self.page.as_ref().map(|p| (p.id, p.depth));
        let new = page.as_ref().map(|p| (p.id, p.depth));
        if old.map(|o| o.0) != new.map(|n| n.0) {
            let depth = |d: Option<(_, usize)>| d.map_or(0, |d| d.1);
            self.dir = if depth(new) >= depth(old) { 1.0 } else { -1.0 };
            self.leaving = self.page.take().map(|p| (p, 0.0));
            self.delay = if self.leaving.is_some() { OUT } else { 0.0 };
            self.t = 0.0;
        }
        self.page = page;
        self.t += dt;
        if let Some((_, t)) = &mut self.leaving {
            *t += dt;
        }
        if self.leaving.as_ref().is_some_and(|(_, t)| *t >= OUT) {
            self.leaving = None;
        }
    }

    /// Deja `page` ya de frente, sin giro (para las capturas).
    pub fn settle(&mut self, page: Option<Page>) {
        self.page = page;
        self.leaving = None;
        self.delay = 0.0;
        self.t = IN;
    }

    /// Giro de la página actual: de ±90° (de canto) a 0 (de frente), con un leve rebote.
    fn incoming_angle(&self) -> Option<f32> {
        let u = ((self.t - self.delay) / IN).min(1.0);
        if u < 0.0 {
            return None;
        }
        let rest = 1.0 - u;
        let swing = rest * rest * rest - 0.12 * (u * std::f32::consts::PI).sin() * rest;
        Some(-self.dir * FRAC_PI_2 * swing)
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets, time: f32, skin: usize) {
        // El velo aparece con el letrero y se va con el último.
        let dim = match (&self.page, &self.leaving) {
            (Some(_), _) => DIM,
            (None, Some((_, t))) => DIM * (1.0 - t / OUT),
            (None, None) => 0.0,
        };
        if dim > 0.0 {
            r.quad(0.0, 0.0, 800.0, 600.0, [0.0, 0.0, 0.0, dim]);
        }
        if let Some((p, t)) = &self.leaving {
            draw_page(r, a, p, self.dir * FRAC_PI_2 * (t / OUT), time, skin);
        }
        if let (Some(p), Some(angle)) = (&self.page, self.incoming_angle()) {
            draw_page(r, a, p, angle, time, skin);
        }
    }
}

/// Un letrero girado `angle` radianes sobre su eje vertical.
fn draw_page(r: &mut Renderer, a: &Assets, p: &Page, angle: f32, time: f32, skin: usize) {
    // Ancho del texto con una fuente: el cursor, la etiqueta más larga y su valor con « ».
    let content = |font: &cb_render::Font| {
        let arrows = font.width("« ") + font.width(" »");
        let value_w = p
            .rows
            .iter()
            .filter_map(|(_, v)| v.as_ref())
            .map(|v| font.width(v) + arrows)
            .fold(0.0, f32::max);
        let label_w = p
            .rows
            .iter()
            .map(|(l, _)| font.width(l))
            .fold(0.0, f32::max);
        let gap = if value_w > 0.0 { 30.0 } else { 0.0 };
        (CURSOR + label_w + gap + value_w).max(font.width(&p.title) + 20.0)
    };
    // Si no cabe con la grande (etiquetas largas como las de Bomb Timing), todo con la chica.
    let max_inner = MAX_WIDTH - 2.0 * (BORDER + PAD);
    let font = if content(&a.font_big) <= max_inner {
        &a.font_big
    } else {
        &a.font_small
    };
    let fh = font.line_height().round();
    let inner = content(font);
    let w = (inner + 2.0 * (BORDER + PAD)).min(MAX_WIDTH);
    let row_h = fh + ROW_GAP;
    let h = fh * 1.6 + p.rows.len() as f32 * row_h + 2.0 * (BORDER + PAD);
    let (cx, cy) = (400.0, 300.0);
    let (x0, y0) = (cx - w / 2.0, cy - h / 2.0);

    // Perspectiva del giro: el lado que se aleja se achica hacia el centro.
    let (sin, cos) = angle.sin_cos();
    let warp = |x: f32, y: f32| {
        let dx = x - cx;
        let k = 1.0 / (1.0 + dx * sin * PERSPECTIVE);
        [cx + dx * cos * k, cy + (y - cy) * k]
    };
    let shade = 1.0 - 0.55 * sin.abs();
    let tint = [shade, shade, shade, 1.0];
    let (tw, _) = r.texture_size(a.sign);
    let xs = [x0, x0 + BORDER, x0 + w - BORDER, x0 + w];
    let ys = [y0, y0 + BORDER, y0 + h - BORDER, y0 + h];
    let ts = [0.0, BORDER_TEXELS, tw - BORDER_TEXELS, tw];
    for j in 0..3 {
        for i in 0..3 {
            let src = [ts[i], ts[j], ts[i + 1] - ts[i], ts[j + 1] - ts[j]];
            let corners = [
                warp(xs[i], ys[j]),
                warp(xs[i + 1], ys[j]),
                warp(xs[i], ys[j + 1]),
                warp(xs[i + 1], ys[j + 1]),
            ];
            r.sprite_corners(a.sign, src, corners, tint);
        }
    }

    // El texto, solo casi de frente (sin deformar: aparece al final del giro).
    let alpha = 1.0 - angle.abs() / 0.12;
    if alpha <= 0.0 {
        return;
    }
    let gold = [1.0, 0.85, 0.4, alpha];
    let cream = [0.93, 0.88, 0.78, alpha];
    let outline = |c: f32| TextStyle {
        outline: Some((c, [0.12, 0.06, 0.02, alpha])),
        shadow: TextStyle::DEFAULT.shadow,
    };
    let mut y = y0 + BORDER + PAD * 0.7;
    r.text_styled(font, &p.title, cx, y, gold, Align::Center, outline(2.0));
    y += fh * 1.6;
    let (left, right) = (x0 + BORDER + PAD, x0 + w - BORDER - PAD);
    for (row, (label, value)) in p.rows.iter().enumerate() {
        let selected = row == p.selection;
        if selected {
            // Un resplandor cálido detrás de la fila y la bomba encendida como cursor.
            r.sprite_whole(
                a.glow,
                [left - 10.0, y - 10.0, right - left + 20.0, fh + 20.0],
                [1.4, 0.55, 0.12, 0.45 * alpha],
            );
            let frame = (time * 8.0) as u32 % 4;
            let beat = 1.0 + 0.06 * (time * 9.0).sin();
            let size = fh * 1.8 * beat;
            r.sprite_tinted(
                a.bombs.tex,
                a.bombs.cell(skin as u32 * 4 + frame),
                [
                    left + 16.0 - size / 2.0,
                    y + fh / 2.0 - size * 0.6,
                    size,
                    size,
                ],
                [1.0, 1.0, 1.0, alpha],
            );
        }
        let color = if selected { gold } else { cream };
        let style = if selected {
            TextStyle {
                outline: Some((2.0, [0.3, 0.06, 0.0, alpha])),
                shadow: Some(Shadow {
                    offset: [0.0, 0.0],
                    blur: 8.0,
                    color: [1.0, 0.35, 0.0, 0.8 * alpha],
                }),
            }
        } else {
            outline(1.5)
        };
        r.text_styled(font, label, left + CURSOR, y, color, Align::Left, style);
        if let Some(v) = value {
            let shown = if selected {
                format!("« {v} »")
            } else {
                v.clone()
            };
            r.text_styled(font, &shown, right, y, color, Align::Right, style);
        }
        y += row_h;
    }
}
