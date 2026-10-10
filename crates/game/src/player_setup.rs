//! Pantalla `Local Player Setup` (ClanBomber2: PlayerSetup.cpp): 8 puestos con aspecto, nombre,
//! equipo y controlador.

use cb_core::{Config, ControllerKind, NR_SKINS};
use cb_render::{Align, Renderer};
use winit::keyboard::KeyCode;

use crate::assets::Assets;
use crate::draw::{srgb_rgba, text_color};
use crate::screen::{EditResult, Flow, TextEdit};

/// Columnas navegables: aspecto, nombre, equipo y controlador.
const COLS: usize = 4;
/// Límites de las columnas en píxeles (`cg` del original).
const COL_EDGES: [f32; 5] = [17.0, 95.0, 340.0, 500.0, 720.0];
/// Controladores que se recorren con Enter (sin joysticks detectados: `6 + count`).
const CONTROLLERS: [ControllerKind; 6] = [
    ControllerKind::Ai,
    ControllerKind::AiMass,
    ControllerKind::Keymap(1),
    ControllerKind::Keymap(2),
    ControllerKind::Keymap(3),
    ControllerKind::RcMouse,
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
                        // `controller % (6 + joysticks)`: los seis fijos y luego cada mando.
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

    /// `time` en segundos, para el parpadeo del campo de nombre.
    pub fn draw(&self, r: &mut Renderer, a: &Assets, c: &Config, time: f32) {
        let col = text_color();
        let editing = self.edit.is_some();
        r.sprite(
            a.player_setup_bg,
            [0.0, 0.0, 800.0, 600.0],
            [0.0, 0.0, 800.0, 600.0],
        );
        r.text(&a.font_small, "SKIN", 55.0, 40.0, col, Align::Center);
        r.text(&a.font_small, "NAME", 130.0, 40.0, col, Align::Center);
        r.text(&a.font_small, "TEAM", 418.0, 40.0, col, Align::Center);
        r.text(&a.font_small, "Controller", 612.0, 40.0, col, Align::Center);
        r.text(
            &a.font_small,
            "SPACE ENABLES OR DISABLES A PLAYER, H TOGGLES HIGHLIGHTING",
            12.0,
            580.0,
            col,
            Align::Left,
        );

        let row_y = |i: usize| 70.0 + i as f32 * 63.0;
        r.quad(
            0.0,
            row_y(self.row),
            800.0,
            63.0,
            srgb_rgba(55, 110, 220, 70),
        );
        if !editing {
            let (x0, x1) = (COL_EDGES[self.col], COL_EDGES[self.col + 1]);
            r.quad(
                x0,
                row_y(self.row),
                x1 - x0,
                63.0,
                srgb_rgba(40, 40, 240, 120),
            );
        }

        for (i, b) in c.bombers.iter().enumerate() {
            let y = row_y(i);
            if b.highlight_maptile {
                r.quad(
                    34.0,
                    90.0 + i as f32 * 63.0,
                    40.0,
                    40.0,
                    srgb_rgba(0, 0, 0xFF, 50),
                );
            }
            // Pose 0 del aspecto, apoyada en la base de la fila.
            let skin = &a.skins[b.skin as usize % a.skins.len()];
            r.sheet_sprite(
                &skin.sheet,
                0,
                35.0,
                130.0 + i as f32 * 63.0 - skin.sheet.cell_h,
            );

            // El nombre que se está editando se enseña pequeño debajo del cuadro de edición.
            let font = if editing && self.row == i {
                &a.font_small
            } else {
                &a.font_big
            };
            r.text(
                font,
                &b.name,
                130.0,
                90.0 + i as f32 * 63.0,
                col,
                Align::Left,
            );

            r.sheet_sprite(&a.ps_teams, b.team as u32, 360.0, y + 2.0);
            match b.controller {
                ControllerKind::Ai => {
                    r.sheet_sprite(&a.ps_controls, 0, 550.0, y + 2.0);
                    r.text(
                        &a.font_small,
                        "DOK",
                        650.0,
                        102.0 + i as f32 * 63.0,
                        col,
                        Align::Right,
                    );
                }
                ControllerKind::AiMass => {
                    r.sheet_sprite(&a.ps_controls, 0, 550.0, y + 2.0);
                    r.text(
                        &a.font_small,
                        "MASS",
                        650.0,
                        102.0 + i as f32 * 63.0,
                        col,
                        Align::Right,
                    );
                }
                ControllerKind::Keymap(n) => {
                    r.sheet_sprite(&a.ps_controls, n as u32, 550.0, y + 2.0)
                }
                ControllerKind::RcMouse => {
                    r.text(
                        &a.font_small,
                        "RCMOUSE",
                        612.0,
                        110.0 + i as f32 * 63.0,
                        col,
                        Align::Center,
                    );
                }
                ControllerKind::Joystick(n) => {
                    r.sheet_sprite(&a.ps_controls, 4, 550.0, y + 2.0);
                    r.text(&a.font_small, &n.to_string(), 620.0, y, col, Align::Left);
                }
            }
            if !b.enabled {
                r.quad(0.0, y, 800.0, 63.0, srgb_rgba(0, 0, 0, 170));
            }
        }

        if let Some(edit) = &self.edit {
            // Cuadro de edición que parpadea (periodo de 0.5 s).
            let phase = time % 0.5;
            let alpha = if phase > 0.25 {
                (0.5 - phase) * 3.0
            } else {
                phase * 3.0
            };
            let y = row_y(self.row);
            r.quad(
                95.0,
                y,
                245.0,
                63.0,
                [0.03, 0.72, 0.22, alpha.clamp(0.0, 1.0)],
            );
            r.text(
                &a.font_big,
                &edit.text(),
                130.0,
                90.0 + self.row as f32 * 63.0,
                col,
                Align::Left,
            );
        }
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
        c.bombers[0].controller = ControllerKind::RcMouse;
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
        // Sin mandos, tras el ratón se vuelve directamente a la IA.
        c.bombers[0].controller = ControllerKind::RcMouse;
        s.key(KeyCode::Enter, None, &mut c, 0);
        assert_eq!(c.bombers[0].controller, ControllerKind::Ai);
    }
}
