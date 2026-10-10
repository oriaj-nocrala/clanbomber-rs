//! Pantalla `Map Editor` (ClanBomber2: MapEditor.cpp): lista de mapas, creación y borrado de
//! mapas propios y editor casilla a casilla.

use std::path::{Path, PathBuf};

use cb_core::{map_file_name, Config, MapDoc, MapList, MAP_HEIGHT, MAP_WIDTH};
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::{draw_map_preview, srgb_rgba, text_color};
use crate::screen::{EditResult, Flow, TextEdit};

/// Filas visibles de la lista de mapas.
const VISIBLE: usize = 16;
const MAX_TEXT: usize = 24;

/// Tecla → carácter que se escribe en la casilla (`MapEditor::edit_map`).
const TILE_KEYS: &[(KeyCode, char)] = &[
    (KeyCode::Space, '-'),
    (KeyCode::KeyG, ' '),
    (KeyCode::KeyW, '*'),
    (KeyCode::KeyB, '+'),
    (KeyCode::KeyR, 'R'),
    (KeyCode::KeyH, '<'),
    (KeyCode::KeyK, '>'),
    (KeyCode::KeyU, '^'),
    (KeyCode::KeyJ, 'v'),
    (KeyCode::KeyI, 'S'),
    (KeyCode::KeyO, 'o'),
];

const NUMBER_KEYS: [KeyCode; 8] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
];

enum Sub {
    Normal,
    Author(TextEdit),
    Help,
}

struct Editing {
    path: PathBuf,
    name: String,
    doc: MapDoc,
    cur: (i32, i32),
    /// Modo de texto (F2): tras escribir una casilla el cursor avanza solo.
    text_mode: bool,
    sub: Sub,
}

enum State {
    Select,
    NewName(TextEdit),
    Edit(Box<Editing>),
}

pub struct MapEditor {
    list: MapList,
    current: usize,
    top: usize,
    list_width: f32,
    state: State,
    shipped_dir: PathBuf,
    user_dir: PathBuf,
}

fn top_for(current: usize, count: usize) -> usize {
    ((current as i32 - 8).min(count as i32 - VISIBLE as i32)).max(0) as usize
}

impl MapEditor {
    pub fn new(shipped_dir: &Path, user_dir: &Path, a: &Assets) -> Option<Self> {
        let mut ed = MapEditor {
            list: MapList::scan_dirs(&[(shipped_dir, false), (user_dir, true)]).ok()?,
            current: 0,
            top: 0,
            list_width: 230.0,
            state: State::Select,
            shipped_dir: shipped_dir.to_owned(),
            user_dir: user_dir.to_owned(),
        };
        if ed.list.paths.is_empty() {
            return None;
        }
        ed.measure(a);
        Some(ed)
    }

    fn names(&self) -> Vec<String> {
        self.list
            .paths
            .iter()
            .map(|p| {
                p.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect()
    }

    fn measure(&mut self, a: &Assets) {
        self.list_width = 230.0;
        for n in self.names() {
            let w = a.font_small.width(&n);
            if w + 10.0 > self.list_width {
                self.list_width = w + 40.0;
            }
        }
    }

    fn rescan(&mut self, a: &Assets) {
        if let Ok(l) = MapList::scan_dirs(&[(&self.shipped_dir, false), (&self.user_dir, true)]) {
            self.list = l;
        }
        self.measure(a);
    }

    fn select(&mut self, to: usize) {
        self.current = to;
        self.top = top_for(to, self.list.paths.len());
    }

    /// Abre en el editor el mapa de ese nombre (para las capturas de pantalla).
    pub fn edit_named(&mut self, name: &str) {
        if let Some(i) = self.names().iter().position(|n| n == name) {
            self.select(i);
            self.start_editing(i);
        }
    }

    fn start_editing(&mut self, index: usize) {
        let path = self.list.paths[index].clone();
        if let Ok(doc) = MapDoc::load(&path) {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.state = State::Edit(Box::new(Editing {
                path,
                name,
                doc,
                cur: (0, 0),
                text_mode: false,
                sub: Sub::Normal,
            }));
        }
    }

    /// ¿Se está escribiendo un nombre o un autor (y no editando casillas)?
    pub fn is_typing(&self) -> bool {
        match &self.state {
            State::NewName(_) => true,
            State::Edit(ed) => matches!(ed.sub, Sub::Author(_) | Sub::Help),
            State::Select => false,
        }
    }

    pub fn key(&mut self, code: KeyCode, typed: Option<&str>, a: &Assets, _c: &Config) -> Flow {
        match &mut self.state {
            State::Select => return self.key_select(code, a),
            State::NewName(edit) => match edit.key(code, typed, MAX_TEXT) {
                EditResult::Editing => {}
                EditResult::Cancelled => self.state = State::Select,
                EditResult::Accepted(name) => {
                    let created = map_file_name(&name).map(|f| self.user_dir.join(f));
                    match created {
                        Some(path) => {
                            // Un mapa que ya existe no se pisa: se abre tal cual.
                            if !path.exists() && MapDoc::template().save(&path).is_err() {
                                self.state = State::Select;
                                return Flow::Stay;
                            }
                            self.rescan(a);
                            if let Some(i) = self.list.paths.iter().position(|p| *p == path) {
                                self.select(i);
                                if self.list.writable[i] {
                                    self.start_editing(i);
                                    return Flow::Stay;
                                }
                            }
                            self.state = State::Select;
                        }
                        None => self.state = State::Select,
                    }
                }
            },
            State::Edit(ed) => {
                if ed.key(code, typed) {
                    self.state = State::Select;
                }
            }
        }
        Flow::Stay
    }

    fn key_select(&mut self, code: KeyCode, a: &Assets) -> Flow {
        let count = self.list.paths.len();
        match code {
            KeyCode::Escape | KeyCode::Backspace => return Flow::Back,
            KeyCode::Enter | KeyCode::NumpadEnter if self.list.writable[self.current] => {
                self.start_editing(self.current);
            }
            KeyCode::KeyN => self.state = State::NewName(TextEdit::new("")),
            KeyCode::KeyD if self.list.writable[self.current] => {
                if std::fs::remove_file(&self.list.paths[self.current]).is_ok() {
                    self.rescan(a);
                    let last = self.list.paths.len().saturating_sub(1);
                    self.select(self.current.min(last));
                }
            }
            KeyCode::ArrowUp if self.current > 0 => self.select(self.current - 1),
            KeyCode::ArrowDown if self.current + 1 < count => self.select(self.current + 1),
            _ => {}
        }
        Flow::Stay
    }

    pub fn draw(&self, r: &mut Renderer, a: &Assets, c: &Config) {
        match &self.state {
            State::Edit(ed) => ed.draw(r, a, c),
            State::Select => self.draw_select(r, a),
            State::NewName(edit) => {
                self.draw_select(r, a);
                let col = text_color();
                r.quad(200.0, 300.0, 400.0, 100.0, srgb_rgba(0, 0, 0, 128));
                r.text(
                    &a.font_big,
                    &format!("Name: {}", edit.text()),
                    230.0,
                    330.0,
                    col,
                    Align::Left,
                );
                r.text(
                    &a.font_small,
                    "PRESS ESC TO ABORT",
                    400.0,
                    380.0,
                    col,
                    Align::Center,
                );
            }
        }
    }

    fn draw_select(&self, r: &mut Renderer, a: &Assets) {
        let col = text_color();
        r.sprite(
            a.editor_bg,
            [0.0, 0.0, 800.0, 600.0],
            [0.0, 0.0, 800.0, 600.0],
        );
        let (list_top, row_h) = (116.0, 25.0);
        r.quad(
            45.0,
            list_top,
            self.list_width,
            400.0,
            srgb_rgba(75, 75, 75, 128),
        );
        r.quad(
            45.0,
            list_top + (self.current - self.top) as f32 * row_h,
            self.list_width,
            row_h,
            srgb_rgba(25, 100, 200, 128),
        );
        let names = self.names();
        for i in 0..VISIBLE.min(names.len()) {
            let idx = i + self.top;
            let y = list_top + i as f32 * row_h;
            // Los de solo lectura van en rojo.
            if !self.list.writable[idx] {
                r.quad(45.0, y, self.list_width, row_h, srgb_rgba(255, 25, 25, 100));
            }
            r.text(
                &a.font_small,
                &names[idx],
                60.0,
                118.0 + i as f32 * row_h,
                col,
                Align::Left,
            );
        }
        if self.top > 0 {
            r.text(&a.font_big, "+", 165.0, 85.0, col, Align::Center);
        }
        if self.top + VISIBLE < names.len() {
            r.text(&a.font_big, "-", 165.0, 516.0, col, Align::Center);
        }
        for (text, y) in [
            ("Select a map to edit and press Enter", 150.0),
            ("Maps marked red are readonly", 190.0),
            ("Press N to create a new map", 250.0),
            ("Press D to delete a map", 290.0),
        ] {
            r.text(&a.font_big, text, 520.0, y, col, Align::Center);
        }
    }
}

impl Editing {
    /// Mantiene el cursor dentro del mapa; en modo de texto salta de línea (`clip_cursor`).
    fn clip(&mut self) {
        let (w, h) = (MAP_WIDTH as i32, MAP_HEIGHT as i32);
        if self.text_mode {
            if self.cur.0 < 0 {
                self.cur = (w - 1, self.cur.1 - 1);
            }
            if self.cur.0 > w - 1 {
                self.cur = (0, self.cur.1 + 1);
            }
        } else {
            self.cur.0 = self.cur.0.clamp(0, w - 1);
        }
        self.cur.1 = self.cur.1.clamp(0, h - 1);
        self.cur.0 = self.cur.0.clamp(0, w - 1);
    }

    fn set_tile(&mut self, c: char) {
        self.doc.set(self.cur.0 as usize, self.cur.1 as usize, c);
        if self.text_mode {
            self.cur.0 += 1;
        }
    }

    /// Devuelve `true` al salir del editor (Esc), después de guardar.
    fn key(&mut self, code: KeyCode, typed: Option<&str>) -> bool {
        match &mut self.sub {
            Sub::Help => {
                self.sub = Sub::Normal;
                return false;
            }
            Sub::Author(edit) => {
                match edit.key(code, typed, MAX_TEXT) {
                    EditResult::Editing => {}
                    EditResult::Cancelled => self.sub = Sub::Normal,
                    EditResult::Accepted(author) => {
                        if !author.is_empty() {
                            self.doc.author = author;
                        }
                        self.sub = Sub::Normal;
                    }
                }
                return false;
            }
            Sub::Normal => {}
        }

        match code {
            KeyCode::Escape => {
                let _ = self.doc.save(&self.path);
                return true;
            }
            KeyCode::F1 => self.sub = Sub::Help,
            KeyCode::F2 => self.text_mode = !self.text_mode,
            KeyCode::Backspace => {
                self.cur.0 -= 1;
                self.clip();
                self.doc.set(self.cur.0 as usize, self.cur.1 as usize, '-');
            }
            KeyCode::PageUp => self.doc.set_max_players(self.doc.max_players as i32 + 1),
            KeyCode::PageDown => self.doc.set_max_players(self.doc.max_players as i32 - 1),
            KeyCode::KeyA => self.sub = Sub::Author(TextEdit::new(&self.doc.author)),
            KeyCode::ArrowLeft => self.cur.0 -= 1,
            KeyCode::ArrowRight => self.cur.0 += 1,
            KeyCode::ArrowUp => self.cur.1 -= 1,
            KeyCode::ArrowDown => self.cur.1 += 1,
            _ => {
                if let Some(&(_, c)) = TILE_KEYS.iter().find(|(k, _)| *k == code) {
                    self.set_tile(c);
                } else if let Some(n) = NUMBER_KEYS.iter().position(|k| *k == code) {
                    self.doc
                        .set_bomber_pos(self.cur.0 as usize, self.cur.1 as usize, n as u8);
                    if self.text_mode {
                        self.cur.0 += 1;
                    }
                }
            }
        }
        self.clip();
        false
    }

    fn draw(&self, r: &mut Renderer, a: &Assets, c: &Config) {
        let col = text_color();
        let theme = c.theme.clamp(0, 5) as u32;
        r.sprite(
            a.editor_bg,
            [0.0, 0.0, 800.0, 600.0],
            [0.0, 0.0, 800.0, 600.0],
        );

        if let Sub::Help = self.sub {
            self.draw_help(r, a, theme);
            return;
        }

        let map = self.doc.to_map();
        draw_map_preview(r, a, &map, theme, 60.0, 40.0, 1.0, false);
        // Casillas `R` marcadas con su signo de interrogación.
        for &(x, y) in &map.random_boxes {
            r.sheet_sprite(&a.addons, 5, 60.0 + x as f32 * 40.0, 40.0 + y as f32 * 40.0);
        }
        // Salidas de los jugadores activos en el mapa.
        for i in 0..self.doc.max_players {
            let (x, y) = self.doc.bomber_pos(i).unwrap_or((0, 0));
            let (px, py) = (60.0 + x as f32 * 40.0, 40.0 + y as f32 * 40.0);
            r.sheet_sprite(&a.addons, 6, px, py);
            r.text(
                &a.font_small,
                &(i + 1).to_string(),
                px + 20.0,
                py + 12.0,
                col,
                Align::Center,
            );
        }
        r.quad(
            60.0 + self.cur.0 as f32 * 40.0,
            40.0 + self.cur.1 as f32 * 40.0,
            40.0,
            40.0,
            srgb_rgba(150, 125, 25, 150),
        );

        r.text(&a.font_big, &self.name, 780.0, 3.0, col, Align::Right);
        r.text(
            &a.font_small,
            "PRESS F1 FOR HELP",
            20.0,
            3.0,
            col,
            Align::Left,
        );
        let mode = if self.text_mode {
            "TEXT EDITOR MODE"
        } else {
            "NORMAL EDITOR MODE"
        };
        r.text(&a.font_small, mode, 20.0, 580.0, col, Align::Left);
        let players = format!("NUMBER OF PLAYERS   {}", self.doc.max_players);
        r.text(&a.font_small, &players, 780.0, 580.0, col, Align::Right);

        if let Sub::Author(edit) = &self.sub {
            r.quad(150.0, 300.0, 400.0, 100.0, srgb_rgba(0, 0, 0, 200));
            r.text(
                &a.font_big,
                &format!("Author: {}", edit.text()),
                180.0,
                300.0,
                col,
                Align::Left,
            );
            r.text(
                &a.font_small,
                "PRESS ESC TO ABORT",
                400.0,
                380.0,
                col,
                Align::Center,
            );
        }
    }

    /// `show_help`: qué tecla pone cada cosa.
    fn draw_help(&self, r: &mut Renderer, a: &Assets, theme: u32) {
        let col = text_color();
        let tile = |r: &mut Renderer, x: f32, y: f32, base: u32, addon: Option<u32>| {
            r.sheet_sprite(&a.maptiles, theme * 4 + base, x, y);
            if let Some(ad) = addon {
                r.sheet_sprite(&a.addons, ad, x, y);
            }
        };
        let line = |r: &mut Renderer, text: &str, x: f32, y: f32| {
            r.text(&a.font_small, text, x, y, col, Align::Left);
        };
        for (y, base, addon, text) in [
            (70.0, 0, None, "PRESS G FOR A GROUND TILE"),
            (110.0, 1, None, "PRESS W FOR A WALL"),
            (150.0, 2, None, "PRESS B FOR A BOX"),
            (190.0, 2, Some(5), "PRESS R FOR A RANDOM TILE"),
            (230.0, 0, Some(7), "PRESS O FOR A BOMB TRAP"),
            (270.0, 0, Some(0), "PRESS I FOR AN ICE TILE"),
            (320.0, 0, Some(6), "PRESS NUMBER KEYS FOR BOMBER POSITIONS"),
        ] {
            tile(r, 40.0, y, base, addon);
            line(r, text, 110.0, y + 8.0);
        }
        line(r, "THE FOLLOWING KEYS PRODUCE ARROWS", 40.0, 398.0);
        // (x, y de la casilla, addon de flecha, letra y dónde se escribe)
        for (x, y, addon, letter, lx, ly) in [
            (150.0, 460.0, 3, "U", 170.0, 428.0),
            (110.0, 500.0, 2, "J", 98.0, 510.0),
            (150.0, 500.0, 1, "K", 170.0, 550.0),
            (190.0, 500.0, 4, "L", 242.0, 510.0),
        ] {
            tile(r, x, y, 0, Some(addon));
            r.text(&a.font_small, letter, lx, ly, col, Align::Center);
        }
        line(r, "PRESS SPACE BAR FOR A HOLE", 380.0, 448.0);
        line(r, "PAGE UP AND DOWN SET NR OF PLAYERS", 380.0, 468.0);
        line(r, "PRESS A FOR CHANGING THE AUTHOR", 380.0, 488.0);
        line(r, "PRESSING F2 SWITCHES EDITOR MODE", 380.0, 508.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editing() -> Editing {
        Editing {
            path: std::env::temp_dir().join(format!("cb-editor-test-{}.map", std::process::id())),
            name: "t".into(),
            doc: MapDoc::template(),
            cur: (5, 5),
            text_mode: false,
            sub: Sub::Normal,
        }
    }

    #[test]
    fn keys_write_tiles_and_arrows() {
        let mut e = editing();
        e.key(KeyCode::KeyW, None);
        e.key(KeyCode::ArrowRight, None);
        e.key(KeyCode::KeyI, None);
        e.key(KeyCode::ArrowRight, None);
        e.key(KeyCode::KeyK, None);
        assert_eq!(
            (e.doc.data[5][5], e.doc.data[6][5], e.doc.data[7][5]),
            ('*', 'S', '>')
        );
        assert_eq!(e.cur, (7, 5), "en modo normal el cursor no avanza solo");
    }

    #[test]
    fn text_mode_advances_and_wraps_to_the_next_line() {
        let mut e = editing();
        e.key(KeyCode::F2, None);
        e.cur = (MAP_WIDTH as i32 - 1, 3);
        e.key(KeyCode::KeyB, None);
        assert_eq!(e.doc.data[MAP_WIDTH - 1][3], '+');
        assert_eq!(e.cur, (0, 4), "salta a la línea siguiente");
        e.key(KeyCode::Backspace, None);
        assert_eq!(e.cur, (MAP_WIDTH as i32 - 1, 3));
        assert_eq!(e.doc.data[MAP_WIDTH - 1][3], '-', "borrar deja un hueco");
    }

    #[test]
    fn number_keys_move_start_positions_and_page_keys_set_players() {
        let mut e = editing();
        e.key(KeyCode::Digit1, None);
        assert_eq!(e.doc.bomber_pos(0), Some((5, 5)));
        e.key(KeyCode::PageUp, None);
        assert_eq!(e.doc.max_players, 5);
        e.key(KeyCode::PageDown, None);
        e.key(KeyCode::PageDown, None);
        e.key(KeyCode::PageDown, None);
        e.key(KeyCode::PageDown, None);
        assert_eq!(e.doc.max_players, 2, "no baja de 2");
    }

    #[test]
    fn author_editing_and_escape_saves_the_file() {
        let mut e = editing();
        e.key(KeyCode::KeyA, None);
        for _ in 0..20 {
            e.key(KeyCode::Backspace, None);
        }
        e.key(KeyCode::KeyZ, Some("Zoé"));
        e.key(KeyCode::Enter, None);
        assert_eq!(e.doc.author, "Zoé");
        assert!(e.key(KeyCode::Escape, None), "Esc sale");
        assert_eq!(MapDoc::load(&e.path).unwrap().author, "Zoé");
        let _ = std::fs::remove_file(&e.path);
    }

    #[test]
    fn cursor_stays_inside_the_map() {
        let mut e = editing();
        for _ in 0..40 {
            e.key(KeyCode::ArrowRight, None);
            e.key(KeyCode::ArrowDown, None);
        }
        assert_eq!(e.cur, (MAP_WIDTH as i32 - 1, MAP_HEIGHT as i32 - 1));
        for _ in 0..40 {
            e.key(KeyCode::ArrowLeft, None);
            e.key(KeyCode::ArrowUp, None);
        }
        assert_eq!(e.cur, (0, 0));
    }

    #[test]
    fn help_page_returns_on_any_key() {
        let mut e = editing();
        e.key(KeyCode::F1, None);
        assert!(matches!(e.sub, Sub::Help));
        e.key(KeyCode::KeyQ, None);
        assert!(matches!(e.sub, Sub::Normal));
        assert_eq!(e.doc.data[5][5], ' ', "esa tecla no escribió nada");
    }

    #[test]
    fn list_scroll_matches_the_original_window() {
        assert_eq!(top_for(0, 33), 0);
        assert_eq!(top_for(8, 33), 0);
        assert_eq!(top_for(20, 33), 12);
        assert_eq!(top_for(32, 33), 17);
    }
}
