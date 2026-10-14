//! Pantalla `Map Selection` (ClanBomber2: MapSelector.cpp): lista de mapas con vista previa,
//! marcas de activado y elección de tema. Se dibuja en el letrero de madera sobre el tablero
//! del menú.

use cb_core::{Config, Map, MapList};
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::draw_map_preview;
use crate::menu_sign::{self as sign, Front, Tone};
use crate::screen::Flow;

const THEMES: [&str; 6] = [
    "Ice",
    "Jungle",
    "Jungle Arena",
    "Strange Kitchen",
    "Green Crypt",
    "Melting Metal",
];
/// Filas visibles de la lista.
const VISIBLE: usize = 11;
/// Tamaño del letrero.
pub const PANEL: [f32; 2] = [780.0, 560.0];

pub struct MapSelect {
    maps: Vec<Map>,
    current: usize,
    top: usize,
    list_width: f32,
}

/// Primera fila visible para que la actual quede hacia el centro (`map_at_top` del original).
fn top_for(current: usize, count: usize) -> usize {
    let t = (current as i32 - 5).min(count as i32 - VISIBLE as i32);
    t.max(0) as usize
}

impl MapSelect {
    pub fn new(list: &MapList, c: &Config, a: &Assets) -> Option<Self> {
        let maps: Vec<Map> = list
            .paths
            .iter()
            .filter_map(|p| Map::load(p).ok())
            .collect();
        if maps.is_empty() {
            return None;
        }
        let current = (c.start_map.max(0) as usize).min(maps.len() - 1);
        let mut list_width = 230.0f32;
        for m in &maps {
            let w = a.font_small.width(&m.name);
            if w + 10.0 > list_width {
                list_width = w + 40.0;
            }
        }
        Some(MapSelect {
            current,
            top: top_for(current, maps.len()),
            maps,
            list_width,
        })
    }

    fn enabled(&self, i: usize, c: &Config) -> bool {
        !c.disabled_maps.contains(&self.maps[i].name)
    }

    fn go(&mut self, to: usize) {
        self.current = to;
        self.top = top_for(to, self.maps.len());
    }

    /// Devuelve a dónde ir y si hay que guardar la configuración.
    pub fn key(&mut self, code: KeyCode, c: &mut Config) -> (Flow, bool) {
        match code {
            KeyCode::Escape | KeyCode::Backspace => {
                c.start_map = self.current as i32;
                return (Flow::Back, true);
            }
            KeyCode::Space => {
                let name = self.maps[self.current].name.clone();
                match c.disabled_maps.iter().position(|n| *n == name) {
                    Some(i) => {
                        c.disabled_maps.remove(i);
                    }
                    None => c.disabled_maps.push(name),
                }
                return (Flow::Stay, true);
            }
            // A: todos los mapas activos.
            KeyCode::KeyA => {
                c.disabled_maps.clear();
                return (Flow::Stay, true);
            }
            // S: solo el actual.
            KeyCode::KeyS => {
                c.disabled_maps = self
                    .maps
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != self.current)
                    .map(|(_, m)| m.name.clone())
                    .collect();
                return (Flow::Stay, true);
            }
            KeyCode::ArrowUp if self.current > 0 => self.go(self.current - 1),
            KeyCode::ArrowDown if self.current + 1 < self.maps.len() => self.go(self.current + 1),
            KeyCode::ArrowLeft if c.theme > 0 => {
                c.theme -= 1;
                return (Flow::Stay, true);
            }
            KeyCode::ArrowRight if c.theme < 5 => {
                c.theme += 1;
                return (Flow::Stay, true);
            }
            _ => {}
        }
        (Flow::Stay, false)
    }

    /// El contenido del letrero ya de frente (`menu_sign::Front`): la lista a la izquierda y la
    /// vista previa del mapa a la derecha.
    pub fn draw(&self, r: &mut Renderer, a: &Assets, c: &Config, front: &Front, time: f32) {
        let alpha = front.alpha;
        let [left, top, width, height] = sign::inner(front.rect);
        let (big, small) = (&a.font_big, &a.font_small);
        let (fh, sh) = (big.line_height().round(), small.line_height().round());
        let right = left + width;
        sign::title(
            r,
            big,
            "Map Selection",
            left + width / 2.0,
            top - 6.0,
            alpha,
        );

        // Lista: la fila actual brilla; los mapas activos llevan una chispa delante.
        let list_top = top + fh * 1.5;
        let row_h = sh + 6.0;
        let more_x = left + self.list_width / 2.0;
        if self.top > 0 {
            sign::label(
                r,
                small,
                "...",
                more_x,
                list_top - row_h * 0.8,
                Align::Center,
                Tone::Dim,
                alpha,
            );
        }
        for i in 0..VISIBLE.min(self.maps.len()) {
            let idx = i + self.top;
            let y = list_top + i as f32 * row_h;
            let enabled = self.enabled(idx, c);
            let tone = match (idx == self.current, enabled) {
                (true, _) => {
                    sign::highlight(r, a, [left, y, self.list_width, row_h], 1.0, alpha);
                    Tone::Selected
                }
                (false, true) => Tone::Normal,
                (false, false) => Tone::Dim,
            };
            if enabled {
                let pulse = 0.85 + 0.15 * (time * 5.0 + idx as f32).sin();
                r.sprite_whole(
                    a.glow,
                    [left - 4.0, y + row_h / 2.0 - 12.0, 24.0, 24.0],
                    [1.4 * pulse, 0.9 * pulse, 0.25, alpha],
                );
            }
            let name = &self.maps[idx].name;
            sign::label(
                r,
                small,
                name,
                left + 24.0,
                y + 3.0,
                Align::Left,
                tone,
                alpha,
            );
        }
        if self.top + VISIBLE < self.maps.len() {
            let y = list_top + VISIBLE as f32 * row_h - 6.0;
            sign::label(r, small, "...", more_x, y, Align::Center, Tone::Dim, alpha);
        }

        // A la derecha: nombre, autor y vista previa del mapa con el tema elegido.
        let map = &self.maps[self.current];
        let factor = 0.5;
        let (pw, ph) = (17.0 * 40.0 * factor, 13.0 * 40.0 * factor);
        let px = right - pw;
        let cx = px + pw / 2.0;
        sign::label(
            r,
            big,
            &map.name,
            cx,
            top + fh * 1.3,
            Align::Center,
            Tone::Selected,
            alpha,
        );
        let by = format!("by {}", map.author);
        let author_y = top + fh * 2.3;
        sign::label(
            r,
            small,
            &by,
            cx,
            author_y,
            Align::Center,
            Tone::Normal,
            alpha,
        );
        let py = author_y + sh + 10.0;
        // Un hueco oscuro, como si el mapa estuviera hundido en la madera.
        r.quad(
            px - 4.0,
            py - 4.0,
            pw + 8.0,
            ph + 8.0,
            [0.0, 0.0, 0.0, 0.6 * alpha],
        );
        let theme = c.theme.clamp(0, 5) as u32;
        draw_map_preview(r, a, map, theme, px, py, factor, true);

        let too_few = (map.max_players as usize) < c.number_of_players();
        if too_few {
            let s = ph.min(pw);
            r.sprite_whole(
                a.not_available,
                [cx - s / 2.0, py, s, s],
                [1.0, 1.0, 1.0, alpha],
            );
        }
        // Encima de la cruz, para que se lea.
        if !self.enabled(self.current, c) {
            r.quad(px, py, pw, ph, [0.0, 0.0, 0.0, 0.55 * alpha]);
            let y = py + (ph - fh) / 2.0;
            sign::label(
                r,
                big,
                "Disabled",
                cx,
                y,
                Align::Center,
                Tone::Normal,
                alpha,
            );
        }
        let max_text = format!("For up to {} players", map.max_players);
        let tone = if too_few {
            Tone::Color([1.0, 0.25, 0.15])
        } else {
            Tone::Normal
        };
        let players_y = py + ph + 8.0;
        sign::label(
            r,
            small,
            &max_text,
            cx,
            players_y,
            Align::Center,
            tone,
            alpha,
        );

        // Tema: ←/→ lo cambian, como los valores de los submenús.
        let theme_y = players_y + sh + 10.0;
        let theme_name = format!("Theme  « {} »", THEMES[theme as usize]);
        let (mid, tone) = (left + width / 2.0, Tone::Selected);
        sign::label(
            r,
            big,
            &theme_name,
            mid,
            theme_y,
            Align::Center,
            tone,
            alpha,
        );

        let hint = "SPACE TOGGLES THE MAP · A ENABLES ALL · S ONLY THIS ONE";
        let hint_y = top + height - sh;
        sign::label(
            r,
            small,
            hint,
            left + width / 2.0,
            hint_y,
            Align::Center,
            Tone::Dim,
            alpha,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_scrolls_to_keep_the_selection_near_the_middle() {
        assert_eq!(top_for(0, 33), 0);
        assert_eq!(top_for(5, 33), 0);
        assert_eq!(top_for(10, 33), 5);
        assert_eq!(top_for(32, 33), 22, "no pasa del final");
        assert_eq!(top_for(2, 5), 0, "menos mapas que filas");
    }
}
