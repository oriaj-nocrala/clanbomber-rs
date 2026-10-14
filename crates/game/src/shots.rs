//! Guion para sacar las capturas del README (`--screenshots DIR`). Monta cada pantalla del juego,
//! la deja dibujarse un par de fotogramas y pide al renderizador que guarde el lienzo virtual
//! (800x600) en un PNG: no se captura el escritorio. Al final monta con `magick` un GIF de una
//! partida entre IAs y otro de la cámara lenta en la muerte que decide una ronda.

use std::path::PathBuf;

use cb_core::{Config, ControllerKind, Phase, World};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

use crate::credits::Credits;
use crate::help::Help;
use crate::map_editor::MapEditor;
use crate::map_select::MapSelect;
use crate::player_setup::PlayerSetup;
use crate::slowmo;
use crate::{all_maps, maps_dir, user_maps_dir, App, Screen};

/// Semilla fija: las capturas salen igual cada vez.
const SEED: u64 = 7;
/// Fotogramas del GIF (a 10 por segundo) y ticks de simulación entre uno y otro.
const GIF_FRAMES: u32 = 110;
const GIF_TICKS_PER_FRAME: u32 = 6;
/// El GIF de la cámara lenta empieza estos ticks antes de la muerte.
const SLOWMO_LEAD: u32 = 60 * 2;

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
    "slow-motion-gif",
];

pub struct Shots {
    dir: PathBuf,
    scene: usize,
    /// Fotogramas desde que se montó la escena actual.
    frame: u32,
    /// Fotogramas del GIF ya guardados.
    gif_done: u32,
    /// Ticks pendientes (fracción) en la cámara lenta.
    acc: f32,
    /// Ticks que faltan para la muerte del GIF de cámara lenta.
    lead: u32,
}

impl Shots {
    pub fn new(dir: PathBuf) -> Self {
        Shots {
            dir,
            scene: 0,
            frame: 0,
            gif_done: 0,
            acc: 0.0,
            lead: 0,
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
                let was = slowmo::Before::of(w);
                w.step(&[]);
                self.slowmo.after_tick(w, &was);
                w.take_events();
            }
            // Las partículas avanzan al mismo reloj que la partida (para el GIF).
            if let (Some((_, r)), Some(a)) = (self.gfx.as_mut(), &self.assets) {
                let dt = ticks as f32 / cb_core::TICK_HZ as f32;
                self.sparks.update(r, a, w, &self.players, dt);
                r.advance_particles(dt);
                if let Some(a) = &self.assets {
                    let skins: Vec<f32> = self
                        .players
                        .iter()
                        .map(|p| a.skins[p.skin].offset_y)
                        .collect();
                    self.ui.update_trails(w, dt, &skins);
                }
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

    /// La ronda que antes acabe en asesinato (no suicidio) en el tema con ascuas y calor: la
    /// relanza y devuelve cuántos ticks faltan para la muerte que dispara la cámara lenta.
    fn murder_round(&mut self) -> u32 {
        let mut found = None;
        for seed in SEED + 40..SEED + 56 {
            self.start_murder_round(seed);
            let mut ticks = 0;
            while ticks < 60 * 120 && !self.slowmo.active() {
                self.sim(1);
                ticks += 1;
            }
            let murder = self
                .slowmo
                .highlight()
                .is_some_and(|h| h.killer.is_some_and(|k| k != h.victim));
            // La más temprana: el muerto suelta menos extras y se ve mejor cómo se quema.
            if murder && found.is_none_or(|(_, t)| ticks < t) {
                found = Some((seed, ticks));
            }
        }
        let (seed, ticks): (u64, u32) = found.unwrap_or((SEED + 40, 0));
        // Determinista: con la misma semilla la muerte llega en el mismo tick.
        self.start_murder_round(seed);
        let skip = ticks.saturating_sub(SLOWMO_LEAD);
        self.sim(skip);
        ticks - skip
    }

    fn start_murder_round(&mut self, seed: u64) {
        let cfg = config_with(&[(0, AI, 0), (5, AI, 0)], 4);
        self.launch(cfg, seed, Some("Arena"));
        self.scripted = true;
    }

    fn setup_scene(&mut self, name: &str) {
        self.scripted = false;
        match name {
            "menu" => self.screen = Screen::Menu,
            "new-game" => {
                self.screen = Screen::Menu;
                self.menu.key(KeyCode::Enter);
                // El letrero ya de frente, sin el giro de entrada.
                self.sign.settle(self.sign_card());
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
                self.sign.settle(self.sign_card());
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
                        self.sign.settle(self.sign_card());
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
            "help" => {
                self.screen = Screen::Help(Help::new());
                self.sign.settle(self.sign_card());
            }
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

        if name == "gameplay-gif" || name == "slow-motion-gif" {
            let slow = name == "slow-motion-gif";
            if sh.frame == 0 {
                let _ = std::fs::create_dir_all(sh.dir.join("frames"));
                if slow {
                    sh.lead = self.murder_round();
                    sh.acc = 0.0;
                } else {
                    self.setup_scene(name);
                }
            } else if sh.frame > 1 && !slow && sh.gif_done >= GIF_FRAMES
                || slow && sh.gif_done > 0 && sh.lead == 0 && !self.slowmo.active()
            {
                let out = if slow {
                    "slow-motion.gif"
                } else {
                    "gameplay.gif"
                };
                build_gif(&sh.dir, out);
                sh.scene += 1;
                sh.frame = 0;
                sh.gif_done = 0;
                self.shots = Some(sh);
                return;
            } else {
                if slow && sh.lead > 0 {
                    // Hasta la muerte, a velocidad normal.
                    let n = sh.lead.min(GIF_TICKS_PER_FRAME);
                    self.sim(n);
                    sh.lead -= n;
                } else if slow {
                    // Una décima de segundo real: el juego avanza lo que diga la cámara lenta.
                    let scale = self.slowmo.time_scale() as f32;
                    self.slowmo.advance(0.1);
                    sh.acc += 0.1 * scale * cb_core::TICK_HZ as f32;
                    let n = sh.acc.floor();
                    sh.acc -= n;
                    self.sim(n as u32);
                } else {
                    self.sim(GIF_TICKS_PER_FRAME);
                }
                let path = sh
                    .dir
                    .join("frames")
                    .join(format!("frame-{:03}.png", sh.gif_done));
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.request_capture(path);
                }
                sh.gif_done += 1;
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

/// Une los fotogramas en `name` (480x360, 10 fps, paleta reducida; `-fuzz` deja sin tocar los
/// píxeles que apenas cambian, que con el grano de las partículas son casi todos) y los borra.
fn build_gif(dir: &std::path::Path, name: &str) {
    let frames = dir.join("frames");
    let out = dir.join(name);
    let status = std::process::Command::new("magick")
        .args(["-delay", "10", "-loop", "0"])
        .arg(frames.join("frame-*.png"))
        .args([
            "-resize", "480x360", "-fuzz", "3%", "-layers", "Optimize", "-colors", "96",
        ])
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
