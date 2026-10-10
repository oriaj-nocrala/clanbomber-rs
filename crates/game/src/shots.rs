//! Guion para sacar las capturas del README (`--screenshots DIR`). Monta cada pantalla del juego,
//! la deja dibujarse un par de fotogramas y pide al renderizador que guarde el lienzo virtual
//! (800x600) en un PNG: no se captura el escritorio. Al final monta un GIF de una partida entre
//! IAs con `magick`.

use std::path::PathBuf;

use cb_core::{Config, ControllerKind, Phase, World};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

use crate::credits::Credits;
use crate::help::Help;
use crate::map_editor::MapEditor;
use crate::map_select::MapSelect;
use crate::player_setup::PlayerSetup;
use crate::{all_maps, maps_dir, user_maps_dir, App, Screen};

/// Semilla fija: las capturas salen igual cada vez.
const SEED: u64 = 7;
/// Fotogramas del GIF (a 10 por segundo) y ticks de simulación entre uno y otro.
const GIF_FRAMES: u32 = 110;
const GIF_TICKS_PER_FRAME: u32 = 6;

const SCENES: &[&str] = &[
    "menu",
    "new-game",
    "gameplay-ice",
    "gameplay-jungle",
    "scoreboard",
    "team-scoreboard",
    "player-setup",
    "map-select",
    "map-editor",
    "credits",
    "help",
    "gameplay-gif",
];

pub struct Shots {
    dir: PathBuf,
    scene: usize,
    /// Fotogramas desde que se montó la escena actual.
    frame: u32,
    /// Fotogramas del GIF ya guardados.
    gif_done: u32,
}

impl Shots {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(dir.join("frames"));
        Shots {
            dir,
            scene: 0,
            frame: 0,
            gif_done: 0,
        }
    }
}

/// Un puesto: (aspecto, controlador, equipo).
type Seat = (u8, ControllerKind, u8);

/// Configuración con esos jugadores activos y el resto apagados.
fn config_with(seats: &[Seat], theme: i32) -> Config {
    let mut c = Config {
        theme,
        ..Config::default()
    };
    for (i, b) in c.bombers.iter_mut().enumerate() {
        b.enabled = i < seats.len();
        if let Some(&(skin, controller, team)) = seats.get(i) {
            (b.skin, b.controller, b.team) = (skin, controller, team);
        }
    }
    c
}

const AI: ControllerKind = ControllerKind::Ai;

impl App {
    /// Avanza `ticks` la partida sin dibujar ni sonar.
    fn sim(&mut self, ticks: u32) {
        if let Some(w) = self.world.as_mut() {
            for _ in 0..ticks {
                w.step(&[]);
                w.take_events();
            }
            // Las partículas avanzan al mismo reloj que la partida (para el GIF).
            if let Some((_, r)) = self.gfx.as_mut() {
                let dt = ticks as f32 / cb_core::TICK_HZ as f32;
                self.sparks.update(r, w, dt);
                r.advance_particles(dt);
            }
        }
    }

    /// Simula hasta que se cumpla `done` (o `max` ticks).
    fn sim_until(&mut self, max: u32, done: impl Fn(&World) -> bool) {
        for _ in 0..max {
            if self.world.as_ref().is_some_and(&done) {
                break;
            }
            self.sim(1);
        }
    }

    fn setup_scene(&mut self, name: &str) {
        self.scripted = false;
        match name {
            "menu" => self.screen = Screen::Menu,
            "new-game" => {
                self.screen = Screen::Menu;
                self.menu.key(KeyCode::Enter);
            }
            "gameplay-ice" | "gameplay-jungle" => {
                let (theme, map) = if name == "gameplay-ice" {
                    (0, "Arena")
                } else {
                    (2, "Football")
                };
                let cfg = config_with(&[(0, AI, 0), (1, AI, 0), (2, AI, 0), (3, AI, 0)], theme);
                self.launch(cfg, SEED + theme as u64, Some(map));
                self.scripted = true;
                // Hasta que haya fuego, extras en el suelo y alguna caja rota.
                self.sim_until(60 * 90, |w| {
                    w.time_left() < 70.0 && w.explosions().count() > 0 && w.extras().count() >= 2
                });
            }
            "scoreboard" | "team-scoreboard" => {
                let teams = name == "team-scoreboard";
                let t = |a, b, c, d| if teams { (a, b, c, d) } else { (0, 0, 0, 0) };
                let (t0, t1, t2, t3) = t(1, 1, 2, 2);
                let cfg = config_with(&[(0, AI, t0), (1, AI, t1), (2, AI, t2), (3, AI, t3)], 3);
                self.launch(cfg, SEED + 20, Some("Arena"));
                self.scripted = true;
                if let Some(w) = self.world.as_mut() {
                    for (b, pts) in w.bombers.iter_mut().zip([3, 4, 1, 2]) {
                        b.points = pts;
                    }
                }
                self.sim_until(60 * 60 * 5, |w| matches!(w.phase, Phase::Scores { .. }));
                self.ui.settle();
            }
            "player-setup" => {
                let mut cfg = config_with(
                    &[
                        (0, ControllerKind::Keymap(1), 1),
                        (1, ControllerKind::Keymap(2), 1),
                        (3, AI, 2),
                        (6, AI, 2),
                        (2, AI, 0),
                    ],
                    0,
                );
                for (b, n) in cfg
                    .bombers
                    .iter_mut()
                    .zip(["Alice", "Bob", "Dok", "Mel", "Spider"])
                {
                    b.name = n.into();
                }
                cfg.bombers[2].highlight_maptile = true;
                self.config = cfg;
                let mut ps = PlayerSetup::new();
                ps.key(KeyCode::ArrowDown, None, &mut self.config, 0);
                ps.key(KeyCode::ArrowDown, None, &mut self.config, 0);
                ps.key(KeyCode::ArrowRight, None, &mut self.config, 0);
                ps.key(KeyCode::ArrowRight, None, &mut self.config, 0);
                self.screen = Screen::PlayerSetup(ps);
            }
            "map-select" => {
                self.config = config_with(&[(0, AI, 0), (1, AI, 0)], 4);
                if let (Some(list), Some(a)) = (all_maps(), self.assets.as_ref()) {
                    self.config.start_map = list
                        .paths
                        .iter()
                        .position(|p| p.file_stem().is_some_and(|s| s == "FireWheels"))
                        .unwrap_or(0) as i32;
                    self.config.disabled_maps = vec!["Arena".into(), "Boiling_Egg".into()];
                    if let Some(ms) = MapSelect::new(&list, &self.config, a) {
                        self.screen = Screen::MapSelect(ms);
                    }
                }
            }
            "map-editor" => {
                self.config = config_with(&[(0, AI, 0), (1, AI, 0)], 1);
                if let Some(a) = self.assets.as_ref() {
                    if let Some(mut ed) = MapEditor::new(&maps_dir(), &user_maps_dir(), a) {
                        ed.edit_named("Overkill");
                        self.screen = Screen::MapEditor(Box::new(ed));
                    }
                }
            }
            "credits" => {
                let mut c = Credits::new(SEED);
                c.advance(4.0);
                self.screen = Screen::Credits(c);
            }
            "help" => self.screen = Screen::Help(Help::new()),
            "gameplay-gif" => {
                let cfg = config_with(&[(0, AI, 0), (1, AI, 0), (2, AI, 0), (3, AI, 0)], 0);
                self.launch(cfg, SEED + 5, Some("FireWheels"));
                self.scripted = true;
                // Dejar que aterricen y empiece la acción.
                self.sim(60 * 6);
            }
            _ => {}
        }
    }

    /// Un paso del guion; se llama al empezar cada fotograma, antes de dibujar.
    pub(crate) fn shots_step(&mut self, el: &ActiveEventLoop) {
        let Some(mut sh) = self.shots.take() else {
            return;
        };
        if self.assets.is_none() {
            self.shots = Some(sh);
            return;
        }
        let Some(&name) = SCENES.get(sh.scene) else {
            el.exit();
            return;
        };

        if name == "gameplay-gif" {
            if sh.frame == 0 {
                self.setup_scene(name);
            } else if sh.gif_done < GIF_FRAMES {
                self.sim(GIF_TICKS_PER_FRAME);
                let path = sh
                    .dir
                    .join("frames")
                    .join(format!("frame-{:03}.png", sh.gif_done));
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.request_capture(path);
                }
                sh.gif_done += 1;
            } else {
                build_gif(&sh.dir);
                sh.scene += 1;
                sh.frame = 0;
                self.shots = Some(sh);
                return;
            }
            sh.frame += 1;
            self.shots = Some(sh);
            return;
        }

        match sh.frame {
            0 => self.setup_scene(name),
            // Dos fotogramas para que la pantalla se asiente y se dibuje.
            3 => {
                let path = sh.dir.join(format!("{name}.png"));
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.request_capture(path);
                }
            }
            4 => {
                sh.scene += 1;
                sh.frame = 0;
                self.shots = Some(sh);
                return;
            }
            _ => {}
        }
        sh.frame += 1;
        self.shots = Some(sh);
    }
}

/// Une los fotogramas en `gameplay.gif` (480x360, 10 fps, paleta reducida) y los borra.
fn build_gif(dir: &std::path::Path) {
    let frames = dir.join("frames");
    let out = dir.join("gameplay.gif");
    let status = std::process::Command::new("magick")
        .args(["-delay", "10", "-loop", "0"])
        .arg(frames.join("frame-*.png"))
        .args(["-resize", "480x360", "-layers", "Optimize", "-colors", "96"])
        .arg(&out)
        .status();
    match status {
        Ok(s) if s.success() => {
            let _ = std::fs::remove_dir_all(&frames);
            eprintln!("GIF listo: {out:?}");
        }
        Ok(s) => eprintln!("magick falló ({s}); los fotogramas quedan en {frames:?}"),
        Err(e) => {
            eprintln!("no se pudo ejecutar magick ({e}); los fotogramas quedan en {frames:?}")
        }
    }
}
