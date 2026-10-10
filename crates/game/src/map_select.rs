//! Pantalla `Map Selection` (ClanBomber2: MapSelector.cpp): lista de mapas con vista previa,
//! marcas de activado y elección de tema.

use cb_core::{Config, Map, MapList};
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::{draw_map_preview, srgb_rgba, text_color};
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

    pub fn draw(&self, r: &mut Renderer, a: &Assets, c: &Config) {
        let col = text_color();
        let map = &self.maps[self.current];
        r.sprite(
            a.map_select_bg,
            [0.0, 0.0, 800.0, 600.0],
            [0.0, 0.0, 800.0, 600.0],
        );

        r.text(&a.font_big, &map.name, 510.0, 50.0, col, Align::Center);
        r.text(
            &a.font_small,
            "SPACE   TOGGLE CURRENT SELECTION",
            10.0,
            530.0,
            col,
            Align::Left,
        );
        r.text(
            &a.font_small,
            "A   SELECT ALL MAPS",
            10.0,
            550.0,
            col,
            Align::Left,
        );
        r.text(
            &a.font_small,
            "S   SELECT CURRENT MAP ONLY",
            10.0,
            570.0,
            col,
            Align::Left,
        );
        r.text(
            &a.font_small,
            &format!("by {}", map.author),
            510.0,
            90.0,
            col,
            Align::Center,
        );

        // Temas: el elegido va resaltado en rojo.
        r.quad(
            30.0 + c.theme as f32 * 120.0,
            423.0,
            120.0,
            30.0,
            srgb_rgba(255, 0, 0, 128),
        );
        for (i, name) in THEMES.iter().enumerate() {
            r.text(
                &a.font_small,
                name,
                90.0 + i as f32 * 120.0,
                428.0,
                col,
                Align::Center,
            );
        }

        draw_map_preview(
            r,
            a,
            map,
            c.theme.clamp(0, 5) as u32,
            350.0,
            125.0,
            0.5,
            true,
        );

        let max_text = format!("THIS MAP IS ONLY FOR UP TO {} PLAYERS", map.max_players);
        if (map.max_players as usize) < c.number_of_players() {
            r.sprite(
                a.not_available,
                [0.0, 0.0, 256.0, 256.0],
                [402.0, 128.0, 256.0, 256.0],
            );
            r.quad(200.0, 494.0, 400.0, 27.0, srgb_rgba(255, 0, 0, 128));
        }
        r.text(&a.font_small, &max_text, 400.0, 500.0, col, Align::Center);

        // Lista.
        let (list_top, row_h) = (116.0, 25.0);
        r.quad(
            15.0,
            list_top,
            self.list_width,
            391.0 - list_top,
            srgb_rgba(76, 76, 76, 128),
        );
        r.quad(
            15.0,
            list_top + (self.current - self.top) as f32 * row_h,
            self.list_width,
            row_h,
            srgb_rgba(25, 80, 200, 128),
        );
        for i in 0..VISIBLE.min(self.maps.len()) {
            let idx = i + self.top;
            let y = list_top + i as f32 * row_h;
            if self.enabled(idx, c) {
                r.quad(15.0, y, self.list_width, row_h, srgb_rgba(200, 200, 0, 100));
            }
            r.text(
                &a.font_small,
                &self.maps[idx].name,
                25.0,
                118.0 + i as f32 * row_h,
                col,
                Align::Left,
            );
        }
        if self.top > 0 {
            r.text(&a.font_small, "+", 128.0, 90.0, col, Align::Center);
        }
        if self.top + VISIBLE < self.maps.len() {
            r.text(&a.font_small, "-", 128.0, 391.0, col, Align::Center);
        }
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
