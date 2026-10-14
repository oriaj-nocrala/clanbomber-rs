//! Pantalla `Local Player Setup` (ClanBomber2: PlayerSetup.cpp): 8 puestos con aspecto, nombre,
//! equipo y controlador. Se dibuja en el letrero de madera sobre el tablero del menú.

use cb_core::{Config, ControllerKind, NR_SKINS};
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::{srgb_rgba, TEAM_COLORS, TEAM_NAMES};
use crate::menu_sign::{self as sign, Front, Tone};
use crate::screen::{EditResult, Flow, TextEdit};

/// Columnas navegables: aspecto, nombre, equipo y controlador.
const COLS: usize = 4;
/// Límites de las columnas desde el borde izquierdo de lo que hay dentro del letrero.
const COL_EDGES: [f32; 5] = [20.0, 90.0, 340.0, 490.0, 690.0];
/// Alto de cada fila de jugador.
const ROW_H: f32 = 50.0;
/// Tamaño del letrero.
pub const PANEL: [f32; 2] = [780.0, 590.0];
/// Controladores que se recorren con Enter (sin joysticks detectados: `6 + count`).
const CONTROLLERS: [ControllerKind; 4] = [
    ControllerKind::Ai,
    ControllerKind::Keymap(1),
    ControllerKind::Keymap(2),
    ControllerKind::Keymap(3),
];
const MAX_NAME: usize = 14;

pub struct PlayerSetup {
    row: usize,
    col: usize,
    edit: Option<TextEdit>,
}

impl PlayerSetup {
    pub fn new() -> Self {
        PlayerSetup {
            row: 0,
            col: 0,
            edit: None,
        }
    }

    /// ¿Se está escribiendo un nombre?
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }

    /// Devuelve a dónde ir y si hay que guardar la configuración.
    /// `joysticks`: mandos conectados; se suman a los controladores que se recorren con Enter.
    pub fn key(
        &mut self,
        code: KeyCode,
        typed: Option<&str>,
        c: &mut Config,
        joysticks: usize,
    ) -> (Flow, bool) {
        if let Some(edit) = self.edit.as_mut() {
            match edit.key(code, typed, MAX_NAME) {
                EditResult::Editing => {}
                EditResult::Cancelled => self.edit = None,
                EditResult::Accepted(name) => {
                    // `BomberConfig::set_name` ignora los nombres vacíos.
                    if !name.is_empty() {
                        c.bombers[self.row].name = name;
                    }
                    self.edit = None;
                    return (Flow::Stay, true);
                }
            }
            return (Flow::Stay, false);
        }

        let n = c.bombers.len();
        match code {
            KeyCode::Escape | KeyCode::Backspace => return (Flow::Back, false),
            KeyCode::ArrowDown => self.row = (self.row + 1).min(n - 1),
            KeyCode::ArrowUp => self.row = self.row.saturating_sub(1),
            KeyCode::ArrowRight => self.col = (self.col + 1).min(COLS - 1),
            KeyCode::ArrowLeft => self.col = self.col.saturating_sub(1),
            KeyCode::Space => {
                let b = &mut c.bombers[self.row];
                b.enabled = !b.enabled;
                return (Flow::Stay, true);
            }
            KeyCode::KeyH => {
                let b = &mut c.bombers[self.row];
                b.highlight_maptile = !b.highlight_maptile;
                return (Flow::Stay, true);
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let b = &mut c.bombers[self.row];
                match self.col {
                    0 => b.skin = (b.skin + 1) % NR_SKINS,
                    1 => self.edit = Some(TextEdit::new(&b.name)),
                    2 => b.team = (b.team + 1) % 5,
                    _ => {
                        // `controller % (4 + joysticks)`: los cuatro fijos y luego cada mando.
                        let all: Vec<ControllerKind> = CONTROLLERS
                            .iter()
                            .copied()
                            .chain((1..=joysticks).map(|n| ControllerKind::Joystick(n as u8)))
                            .collect();
                        let i = all.iter().position(|k| *k == b.controller).unwrap_or(0);
                        b.controller = all[(i + 1) % all.len()];
                    }
                }
                return (Flow::Stay, true);
            }
            _ => {}
        }
        (Flow::Stay, false)
    }

    /// El contenido del letrero ya de frente (`menu_sign::Front`); `time` en segundos, para el
    /// parpadeo del campo de nombre y la bomba que hace de cursor.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        r: &mut Renderer,
        a: &Assets,
        c: &Config,
        front: &Front,
        time: f32,
        skin: usize,
    ) {
        let alpha = front.alpha;
        let [left, top, width, _] = sign::inner(front.rect);
        let (big, small) = (&a.font_big, &a.font_small);
        let fh = big.line_height().round();
        sign::title(
            r,
            big,
            "Local Player Setup",
            left + width / 2.0,
            top - 6.0,
            alpha,
        );

        // Columnas: aspecto, nombre, equipo y controlador (límites desde el borde izquierdo).
        let edges = COL_EDGES.map(|x| left + x);
        let center = |i: usize| (edges[i] + edges[i + 1]) / 2.0;
        let head_y = top + fh * 1.3;
        for (i, h) in ["Skin", "Name", "Team", "Controller"].iter().enumerate() {
            let tone = if i == self.col && self.edit.is_none() {
                Tone::Selected
            } else {
                Tone::Dim
            };
            sign::label(r, small, h, center(i), head_y, Align::Center, tone, alpha);
        }

        let row_y = |i: usize| head_y + small.line_height() + 6.0 + i as f32 * ROW_H;
        let text_y = |i: usize| row_y(i) + (ROW_H - fh) / 2.0;
        let (x0, x1) = (edges[0], edges[COLS]);
        for (i, b) in c.bombers.iter().enumerate() {
            let y = row_y(i);
            let selected = i == self.row;
            if selected {
                sign::highlight(r, a, [x0, y + 2.0, x1 - x0, ROW_H - 4.0], 0.8, alpha);
                if self.edit.is_none() {
                    let (cx0, cx1) = (edges[self.col], edges[self.col + 1]);
                    sign::highlight(r, a, [cx0, y + 2.0, cx1 - cx0, ROW_H - 4.0], 1.2, alpha);
                }
            }
            let tone = |selected_cell: bool| match (b.enabled, selected && selected_cell) {
                (false, _) => Tone::Dim,
                (true, true) => Tone::Selected,
                (true, false) => Tone::Normal,
            };
            let on_col = |col: usize| self.edit.is_none() && self.col == col;

            // Aspecto: la pose 0, apoyada en la base de la fila; la casilla resaltada detrás.
            let sk = &a.skins[b.skin as usize % a.skins.len()];
            let h = ROW_H - 6.0;
            let w = h * sk.sheet.cell_w / sk.sheet.cell_h;
            if b.highlight_maptile {
                r.quad(
                    center(0) - h / 2.0,
                    y + 3.0,
                    h,
                    h,
                    [0.0, 0.05, 1.0, 0.3 * alpha],
                );
            }
            r.sprite_tinted(
                sk.sheet.tex,
                sk.sheet.cell(0),
                [center(0) - w / 2.0, y + 3.0, w, h],
                [1.0, 1.0, 1.0, alpha],
            );

            // Nombre: el que se edita va en un cuadro verde que late, con su cursor.
            let name_x = edges[1] + 10.0;
            if selected && self.edit.is_some() {
                let pulse = 0.5 + 0.5 * (time * std::f32::consts::TAU * 2.0).sin();
                r.quad(
                    edges[1],
                    y + 3.0,
                    edges[2] - edges[1],
                    ROW_H - 6.0,
                    [0.03, 0.45, 0.15, (0.35 + 0.35 * pulse) * alpha],
                );
            }
            let name = match &self.edit {
                Some(edit) if selected => {
                    let caret = if time % 0.8 < 0.4 { "_" } else { " " };
                    format!("{}{caret}", edit.text())
                }
                _ => b.name.clone(),
            };
            sign::label(
                r,
                big,
                &name,
                name_x,
                text_y(i),
                Align::Left,
                tone(on_col(1)),
                alpha,
            );

            // Equipo: su nombre en su color, con un brillo detrás.
            let team_tone = match b.team {
                0 => tone(on_col(2)),
                t if b.enabled && !(selected && on_col(2)) => {
                    let (cr, cg, cb) = TEAM_COLORS[t as usize - 1];
                    let [lr, lg, lb, _] = srgb_rgba(cr, cg, cb, 255);
                    Tone::Color([lr, lg, lb])
                }
                _ => tone(on_col(2)),
            };
            let team = match b.team {
                0 => "None",
                t => TEAM_NAMES[t as usize - 1].trim_end_matches(" Team"),
            };
            sign::label(
                r,
                big,
                team,
                center(2),
                text_y(i),
                Align::Center,
                team_tone,
                alpha,
            );

            let controller = match b.controller {
                ControllerKind::Ai => "Computer".to_owned(),
                ControllerKind::Keymap(n) => format!("Keyboard {n}"),
                ControllerKind::Joystick(n) => format!("Gamepad {n}"),
            };
            let ctl_tone = tone(on_col(3));
            sign::label(
                r,
                big,
                &controller,
                center(3),
                text_y(i),
                Align::Center,
                ctl_tone,
                alpha,
            );

            // Los puestos apagados, en sombra.
            if !b.enabled {
                let (bx, bw) = (x0 - 4.0, x1 - x0 + 8.0);
                r.quad(bx, y + 2.0, bw, ROW_H - 4.0, [0.0, 0.0, 0.0, 0.6 * alpha]);
            }
            if selected && self.edit.is_none() {
                let cy = text_y(i) + fh * 0.4;
                sign::bomb_cursor(r, a, x0 - 12.0, cy, fh * 0.8, time, skin, alpha);
            }
        }

        let hint = if self.edit.is_some() {
            "ENTER ACCEPTS THE NAME, ESC CANCELS"
        } else {
            "ENTER CHANGES · SPACE ENABLES A PLAYER · H HIGHLIGHTS ITS TILE"
        };
        let hint_y = row_y(c.bombers.len()) + 4.0;
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
    fn navigation_is_clamped_and_enter_cycles_values() {
        let mut c = Config::default();
        let mut s = PlayerSetup::new();
        s.key(KeyCode::ArrowUp, None, &mut c, 0);
        s.key(KeyCode::ArrowLeft, None, &mut c, 0);
        assert_eq!((s.row, s.col), (0, 0));
        assert_eq!(s.key(KeyCode::Enter, None, &mut c, 0), (Flow::Stay, true));
        assert_eq!(c.bombers[0].skin, 1);
        s.key(KeyCode::ArrowRight, None, &mut c, 0);
        s.key(KeyCode::ArrowRight, None, &mut c, 0);
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(c.bombers[0].team, 1);
        s.key(KeyCode::ArrowRight, None, &mut c, 0);
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(
            c.bombers[0].controller,
            ControllerKind::Keymap(2),
            "pasa del teclado 1 al 2"
        );
    }

    #[test]
    fn skin_and_team_wrap_around() {
        let mut c = Config::default();
        c.bombers[0].skin = NR_SKINS - 1;
        let mut s = PlayerSetup::new();
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(c.bombers[0].skin, 0);
    }

    #[test]
    fn space_toggles_and_escape_goes_back() {
        let mut c = Config::default();
        let mut s = PlayerSetup::new();
        assert!(c.bombers[0].enabled);
        s.key(KeyCode::Space, None, &mut c, 0);
        assert!(!c.bombers[0].enabled);
        assert_eq!(s.key(KeyCode::Escape, None, &mut c, 0).0, Flow::Back);
    }

    #[test]
    fn renaming_a_player() {
        let mut c = Config::default();
        let mut s = PlayerSetup::new();
        s.key(KeyCode::ArrowRight, None, &mut c, 0); // columna del nombre
        s.key(KeyCode::Enter, None, &mut c, 0); // empieza a editar
        for _ in 0..3 {
            s.key(KeyCode::Backspace, None, &mut c, 0); // borra "Are"
        }
        s.key(KeyCode::KeyZ, Some("Zoé"), &mut c, 0);
        assert_eq!(s.key(KeyCode::Enter, None, &mut c, 0), (Flow::Stay, true));
        assert_eq!(c.bombers[0].name, "Zoé");
    }

    #[test]
    fn empty_names_are_rejected_and_escape_cancels_the_edit() {
        let mut c = Config::default();
        let mut s = PlayerSetup::new();
        s.key(KeyCode::ArrowRight, None, &mut c, 0);
        s.key(KeyCode::Enter, None, &mut c, 0);
        for _ in 0..5 {
            s.key(KeyCode::Backspace, None, &mut c, 0);
        }
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(c.bombers[0].name, "Are", "un nombre vacío no se guarda");
        s.key(KeyCode::Enter, None, &mut c, 0);
        s.key(KeyCode::KeyQ, Some("q"), &mut c, 0);
        s.key(KeyCode::Escape, None, &mut c, 0);
        assert_eq!(c.bombers[0].name, "Are");
        assert_eq!(s.key(KeyCode::Escape, None, &mut c, 0).0, Flow::Back);
    }

    #[test]
    fn connected_joysticks_join_the_controller_cycle() {
        let mut c = Config::default();
        c.bombers[0].controller = ControllerKind::Keymap(3);
        let mut s = PlayerSetup::new();
        for _ in 0..3 {
            s.key(KeyCode::ArrowRight, None, &mut c, 2);
        }
        s.key(KeyCode::Enter, None, &mut c, 2);
        assert_eq!(c.bombers[0].controller, ControllerKind::Joystick(1));
        s.key(KeyCode::Enter, None, &mut c, 2);
        assert_eq!(c.bombers[0].controller, ControllerKind::Joystick(2));
        s.key(KeyCode::Enter, None, &mut c, 2);
        assert_eq!(
            c.bombers[0].controller,
            ControllerKind::Ai,
            "vuelta al principio"
        );
        // Sin mandos, tras el último teclado se vuelve directamente a la IA.
        c.bombers[0].controller = ControllerKind::Keymap(3);
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(c.bombers[0].controller, ControllerKind::Ai);
    }
}
