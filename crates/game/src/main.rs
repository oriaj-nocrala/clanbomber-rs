// En Windows la versión release no abre una consola detrás de la ventana.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod assets;
mod audio;
mod credits;
mod draw;
mod dying;
mod haze;
mod help;
mod interp;
mod intro;
mod joystick;
mod lighting;
mod map_editor;
mod map_select;
mod menu;
mod paths;
mod player_setup;
mod screen;
mod shards;
mod shots;
mod slowmo;
mod sparks;
mod steps;
mod video;
mod weather;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use cb_core::{Config, ControllerKind, Input, Map, MapList, Phase, Rng, World, TICK_HZ};
use cb_render::{Camera, Renderer};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use assets::Assets;
use audio::{Audio, Sfx};
use credits::Credits;
use draw::{Player, Ui};
use help::Help;
use intro::Intro;
use joystick::Joysticks;
use map_editor::MapEditor;
use map_select::MapSelect;
use menu::{Id, Menu, Outcome};
use player_setup::PlayerSetup;
use screen::Flow;
use sparks::Sparks;
use video::Video;

/// Los tres teclados del original (izquierda, derecha, arriba, abajo, bomba):
/// flechas + Enter, WASD + Tab, e IJKL + Espacio.
const KEYMAPS: [[KeyCode; 5]; 3] = [
    [
        KeyCode::ArrowLeft,
        KeyCode::ArrowRight,
        KeyCode::ArrowUp,
        KeyCode::ArrowDown,
        KeyCode::Enter,
    ],
    [
        KeyCode::KeyA,
        KeyCode::KeyD,
        KeyCode::KeyW,
        KeyCode::KeyS,
        KeyCode::Tab,
    ],
    [
        KeyCode::KeyJ,
        KeyCode::KeyL,
        KeyCode::KeyI,
        KeyCode::KeyK,
        KeyCode::Space,
    ],
];

/// Quién maneja a un bomber (la IA va aparte, en `World::ais`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Keys(usize),
    Pad(usize),
    /// La decide la IA dentro del mundo; aquí no hay teclas.
    Ai,
}

enum Screen {
    Intro(Intro),
    Menu,
    Game,
    PlayerSetup(PlayerSetup),
    MapSelect(MapSelect),
    MapEditor(Box<MapEditor>),
    Credits(Credits),
    Help(Help),
}

struct App {
    screen: Screen,
    config: Config,
    config_path: Option<PathBuf>,
    /// Opciones gráficas (efectos de la GPU), en su propio archivo.
    video: Video,
    video_path: Option<PathBuf>,
    menu: Menu,
    world: Option<World>,
    players: Vec<Player>,
    /// Qué gobierna a cada bomber del mundo.
    controls: Vec<Control>,
    joysticks: Joysticks,
    assets: Option<Assets>,
    ui: Ui,
    /// Ráfagas de partículas que salen de lo que pasa en la partida.
    sparks: Sparks,
    /// Posiciones de antes del último tick, para dibujar a medio camino entre ticks.
    prev: Option<interp::Snapshot>,
    /// Cámara lenta en la muerte que decide la ronda.
    slowmo: slowmo::SlowMo,
    /// Si la ronda acaba en plena cámara lenta, se sigue viendo su último instante hasta que
    /// la cámara termine de alejarse (y entonces sale el marcador).
    frozen: Option<World>,
    keys: HashSet<KeyCode>,
    /// Pulsaciones de bomba pendientes de consumir en el próximo tick (una por flanco y teclado).
    bomb_latch: [bool; 3],
    gfx: Option<(Arc<Window>, Renderer)>,
    last: Instant,
    acc: f64,
    started: Instant,
    audio: Audio,
    /// Música de fondo (F2); como en el original, empieza apagada en cada arranque.
    play_music: bool,
    paused: bool,
    /// Guion de capturas (`--screenshots DIR`); sin él, `None`.
    shots: Option<shots::Shots>,
    /// El guion lleva el reloj de la partida: no se avanza en tiempo real.
    scripted: bool,
}

/// Mapas que trae el juego (de solo lectura).
fn maps_dir() -> PathBuf {
    paths::assets_dir().join("maps")
}

/// Mapas del usuario, en la carpeta de datos de cada sistema (`~/.local/share/clanbomber-rs/maps` en
/// Linux, `~/Library/Application Support/...` en macOS, `%APPDATA%\...` en Windows): los que crea el
/// editor, que se pueden editar y borrar.
fn user_maps_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("clanbomber-rs")
        .join("maps")
}

/// Todos los mapas disponibles: los del juego y los del usuario, por orden alfabético.
fn all_maps() -> Option<MapList> {
    MapList::scan_dirs(&[(&maps_dir(), false), (&user_maps_dir(), true)]).ok()
}

fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

impl App {
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            if let Err(e) = self.config.save(path) {
                eprintln!("no se pudo guardar la configuración en {path:?}: {e}");
            }
        }
    }

    /// `Start Local`: monta el mundo con los jugadores activos. No hace nada si no hay rivales.
    fn start_game(&mut self) {
        let (config, seed) = (self.config.clone(), clock_seed());
        self.launch(config, seed, None);
    }

    /// Monta el mundo con los jugadores activos de `config`. `map` fuerza el primer mapa por
    /// nombre. Devuelve `false` (sin cambiar nada) si no hay al menos dos rivales o mapas.
    fn launch(&mut self, config: Config, seed: u64, map: Option<&str>) -> bool {
        if config.number_of_opponents() < 2 {
            return false;
        }
        let players: Vec<_> = config.enabled_bombers().cloned().collect();
        let Some(mut list) = all_maps() else {
            return false;
        };
        if list.paths.is_empty() {
            return false;
        }
        list.disabled = config.disabled_maps.clone();
        // Primer mapa: el pedido por nombre, el elegido (`start_map`) o uno al azar; salta a uno
        // activo donde quepan todos.
        let named = map.and_then(|n| {
            list.paths
                .iter()
                .position(|p| p.file_stem().is_some_and(|s| s == n))
        });
        let first = if let Some(i) = named {
            list.index = i;
            list.current()
        } else if config.random_map_order {
            list.random_for(players.len(), &mut Rng::new(seed))
        } else {
            list.index = (config.start_map.max(0) as usize).min(list.paths.len() - 1);
            match list.current() {
                Ok(m)
                    if m.max_players as usize >= players.len()
                        && !list.disabled.contains(&m.name) =>
                {
                    Ok(m)
                }
                _ => list.next_for(players.len()),
            }
        };
        let Ok(map): Result<Map, _> = first else {
            return false;
        };

        let mut world = World::with_config(map, seed, config);
        world.playlist = Some(list);
        world.start_match(players.len());
        let teams: Vec<u8> = players.iter().map(|p| p.team).collect();
        world.set_teams(&teams);
        // IA (`Controller_AI`).
        for (i, p) in players.iter().enumerate() {
            if matches!(p.controller, ControllerKind::Ai) {
                world.set_ai(i, true);
            }
        }
        self.controls = players
            .iter()
            .map(|p| match p.controller {
                ControllerKind::Keymap(n) => Control::Keys(n as usize - 1),
                // `Joystick(n)` es el n-ésimo mando (desde 1), como se enseña en Player Setup.
                ControllerKind::Joystick(n) => Control::Pad((n as usize).saturating_sub(1)),
                // La IA va aparte (`World::ais`).
                ControllerKind::Ai => Control::Ai,
            })
            .collect();
        self.players = players
            .into_iter()
            .map(|p| Player {
                name: p.name,
                skin: p.skin as usize,
            })
            .collect();
        self.world = Some(world);
        self.bomb_latch = [false; 3];
        self.ui = Ui::new();
        self.sparks = Sparks::new();
        self.prev = None;
        self.slowmo.reset();
        self.frozen = None;
        self.screen = Screen::Game;
        self.paused = false;
        self.audio.set_music(self.play_music);
        self.acc = 0.0;
        self.last = Instant::now();
        true
    }

    fn open_map_select(&mut self) {
        let Some(a) = self.assets.as_ref() else {
            return;
        };
        let Some(list) = all_maps() else {
            return;
        };
        if let Some(ms) = MapSelect::new(&list, &self.config, a) {
            self.screen = Screen::MapSelect(ms);
        }
    }

    fn open_map_editor(&mut self) {
        let Some(a) = self.assets.as_ref() else {
            return;
        };
        if let Some(ed) = MapEditor::new(&maps_dir(), &user_maps_dir(), a) {
            self.screen = Screen::MapEditor(Box::new(ed));
        }
    }

    /// La tecla de bomba actúa solo al pulsar (Controller_Keyboard::update: flanco de subida).
    /// Si dos bombers comparten teclado, solo el primero recibe cada pulsación.
    fn inputs(&mut self) -> Vec<Input> {
        let keys = &self.keys;
        let latch = &mut self.bomb_latch;
        let pads = &mut self.joysticks;
        self.controls
            .iter()
            .map(|c| match *c {
                Control::Ai => Input::default(),
                Control::Pad(i) => pads.input(i),
                Control::Keys(i) => {
                    let k = &KEYMAPS[i];
                    let down = |j: usize| keys.contains(&k[j]);
                    Input {
                        left: down(0),
                        right: down(1),
                        up: down(2),
                        down: down(3),
                        bomb: std::mem::take(&mut latch[i]),
                    }
                }
            })
            .collect()
    }

    fn key_pressed(
        &mut self,
        el: &ActiveEventLoop,
        code: KeyCode,
        text: Option<&str>,
        repeat: bool,
    ) {
        // Las pantallas de menú manejan sus propias teclas y vuelven al menú con `Flow::Back`.
        let was_typing = match &self.screen {
            Screen::PlayerSetup(s) => s.is_editing(),
            Screen::MapEditor(s) => s.is_typing(),
            _ => false,
        };
        let (flow, save) = match &mut self.screen {
            Screen::PlayerSetup(s) => s.key(code, text, &mut self.config, self.joysticks.count()),
            Screen::MapSelect(s) => s.key(code, &mut self.config),
            Screen::MapEditor(s) => match self.assets.as_ref() {
                Some(a) => (s.key(code, text, a, &self.config), false),
                None => (Flow::Stay, false),
            },
            Screen::Credits(s) => (s.key(code, true), false),
            Screen::Help(s) => (s.key(), false),
            Screen::Intro(s) => (s.key(code), false),
            Screen::Menu => {
                self.menu_key(el, code);
                return;
            }
            Screen::Game => {
                self.game_key(code, repeat);
                return;
            }
        };
        if save {
            self.save_config();
        }
        if flow == Flow::Back {
            self.screen = Screen::Menu;
        } else if !was_typing {
            let sound = match (&self.screen, code) {
                // Moverse: `Menu_break`; elegir o cambiar algo: `Menu_clear`.
                (Screen::PlayerSetup(_), c) if is_arrow(c) => Some(Sfx::Break),
                (Screen::PlayerSetup(_), _) if save => Some(Sfx::Clear),
                (Screen::MapSelect(_), _) if save => Some(Sfx::Clear),
                (Screen::MapSelect(_), KeyCode::ArrowUp | KeyCode::ArrowDown) => Some(Sfx::Break),
                (Screen::MapEditor(_), c) if is_arrow(c) => Some(Sfx::Break),
                (Screen::MapEditor(_), KeyCode::Escape | KeyCode::Backspace) => None,
                (Screen::MapEditor(_), _) => Some(Sfx::Clear),
                _ => None,
            };
            if let Some(sfx) = sound {
                self.audio.play(sfx);
            }
        }
    }

    fn menu_key(&mut self, el: &ActiveEventLoop, code: KeyCode) {
        let at_root = self.menu.at_root();
        let outcome = self.menu.key(code);
        match (&outcome, code) {
            (_, KeyCode::ArrowUp | KeyCode::ArrowDown) | (Outcome::Changed(..), _) => {
                self.audio.play(Sfx::Break)
            }
            // Entrar en un submenú o volver a uno superior.
            (Outcome::None, KeyCode::Enter | KeyCode::NumpadEnter) => self.audio.play(Sfx::Clear),
            (Outcome::None, KeyCode::Escape) if !at_root => self.audio.play(Sfx::Clear),
            _ => {}
        }
        match outcome {
            Outcome::Activate(Id::Quit) => el.exit(),
            Outcome::Activate(Id::StartLocal) => self.start_game(),
            Outcome::Activate(Id::PlayerSetup) => {
                self.screen = Screen::PlayerSetup(PlayerSetup::new())
            }
            Outcome::Activate(Id::MapSelection) => self.open_map_select(),
            Outcome::Activate(Id::MapEditor) => self.open_map_editor(),
            Outcome::Activate(Id::Credits) => {
                self.screen = Screen::Credits(Credits::new(clock_seed()))
            }
            Outcome::Activate(Id::Help) => self.screen = Screen::Help(Help::new()),
            Outcome::Activate(_) | Outcome::None => {}
            Outcome::Changed(id, v) if self.video.set(id, v) => {
                self.menu.sync_video(&self.video);
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.set_effects(self.video.effects());
                }
                if let Some(path) = &self.video_path {
                    if let Err(e) = self.video.save(path) {
                        eprintln!("no se pudo guardar las opciones gráficas en {path:?}: {e}");
                    }
                }
            }
            Outcome::Changed(id, v) => {
                id.set(&mut self.config, v);
                self.menu.sync(&self.config);
                self.save_config();
            }
        }
    }

    /// Vuelve al menú dejando la partida (la música se apaga al salir de `run_game`).
    fn leave_game(&mut self) {
        self.screen = Screen::Menu;
        self.world = None;
        self.paused = false;
        self.audio.set_music(false);
    }

    fn game_key(&mut self, code: KeyCode, repeat: bool) {
        match code {
            // `quit current game`: de vuelta al menú.
            KeyCode::Escape => {
                self.leave_game();
                return;
            }
            // Música de fondo y pausa (`run_game`).
            KeyCode::F2 => {
                self.play_music = !self.play_music;
                self.audio.set_music(self.play_music);
            }
            KeyCode::KeyP | KeyCode::F3 => self.paused = !self.paused,
            // Del marcador a la siguiente ronda; tras la última, de vuelta al menú
            // (GameStatus::show espera la barra espaciadora).
            // Con la imagen congelada (fin de la cámara lenta) aún no se ve el marcador.
            KeyCode::Space if !repeat && self.frozen.is_none() => {
                if let Some(w) = self.world.as_mut() {
                    if let Phase::Scores { end_of_game, .. } = w.phase {
                        if end_of_game {
                            self.leave_game();
                            return;
                        }
                        w.confirm();
                        self.bomb_latch = [false; 3];
                    }
                }
            }
            _ => {}
        }
        if !repeat {
            for (i, k) in KEYMAPS.iter().enumerate() {
                if code == k[4] {
                    self.bomb_latch[i] = true;
                }
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("ClanBomber")
            .with_inner_size(LogicalSize::new(800.0, 600.0));
        let window = Arc::new(el.create_window(attrs).expect("ventana"));
        let mut renderer = pollster::block_on(Renderer::new(window.clone()));
        renderer.set_effects(self.video.effects());
        self.assets = Some(Assets::load(&mut renderer));
        self.gfx = Some((window, renderer));
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state.is_pressed() {
                        self.keys.insert(code);
                        self.key_pressed(el, code, event.text.as_deref(), event.repeat);
                    } else {
                        self.keys.remove(&code);
                        // La pantalla de créditos acelera mientras se mantienen las flechas.
                        if let Screen::Credits(c) = &mut self.screen {
                            c.key(code, false);
                        }
                    }
                }
            }
            WindowEvent::Resized(s) => {
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.shots_step(el);
                self.joysticks.update();
                let now = Instant::now();
                let frame = (now - self.last).as_secs_f64().min(0.25);
                self.last = now;
                match &mut self.screen {
                    Screen::Game => {
                        // Paso fijo: la simulación avanza en ticks de 1/TICK_HZ, independiente del framerate.
                        // La cámara lenta solo reparte el tiempo real más despacio entre los ticks.
                        if !self.paused {
                            self.slowmo.advance(frame as f32);
                        }
                        let scale = if self.video.shockwaves {
                            self.slowmo.time_scale()
                        } else {
                            1.0
                        };
                        self.acc += frame * scale;
                        let dt = 1.0 / TICK_HZ as f64;
                        if !self.slowmo.active() {
                            self.frozen = None;
                        }
                        if self.paused || self.scripted || self.frozen.is_some() {
                            self.acc = 0.0;
                            // Sin interpolar: se dibuja el estado tal cual.
                            self.prev = None;
                        }
                        let mut played = 0.0;
                        while self.acc >= dt {
                            played += dt as f32;
                            let inputs = self.inputs();
                            if let Some(w) = self.world.as_mut() {
                                self.prev = Some(interp::Snapshot::of(w));
                                let was_dead: Vec<bool> =
                                    w.bombers.iter().map(|b| b.dead).collect();
                                let before = self.slowmo.active().then(|| w.clone());
                                w.step(&inputs);
                                self.slowmo.after_tick(w, &was_dead);
                                for e in w.take_events() {
                                    self.audio.play(e.into());
                                }
                                if !matches!(w.phase, Phase::Playing) && before.is_some() {
                                    // Fin de ronda en plena cámara lenta: se congela la imagen.
                                    self.frozen = before;
                                    self.prev = None;
                                    self.acc = 0.0;
                                    break;
                                }
                            }
                            self.acc -= dt;
                        }
                        // Los adornos avanzan con el tiempo real escalado (continuo), no a saltos
                        // de tick: en cámara lenta llega un tick cada varios fotogramas.
                        let fx_dt = if self.paused || self.scripted {
                            played
                        } else {
                            (frame * scale) as f32
                        };
                        if let Some(w) = self.world.as_ref().filter(|_| self.frozen.is_none()) {
                            if !matches!(w.phase, Phase::Playing) {
                                self.slowmo.reset();
                            }
                            self.ui.update(w, frame as f32);
                            if let Some(a) = &self.assets {
                                let skins: Vec<f32> = self
                                    .players
                                    .iter()
                                    .map(|p| a.skins[p.skin].offset_y)
                                    .collect();
                                self.ui.update_trails(w, fx_dt, &skins);
                            }
                            if let (Some((_, r)), Some(a)) = (self.gfx.as_mut(), &self.assets) {
                                self.sparks.update(r, a, w, &self.players, fx_dt);
                                r.advance_particles(fx_dt);
                            }
                        }
                        if let Some((_, r)) = self.gfx.as_mut() {
                            let shaking = self.world.as_ref().is_some_and(|w| w.shake > 0);
                            let live = !self.paused && self.frozen.is_none();
                            r.set_shake(if shaking && live { 1.0 } else { 0.0 });
                            r.set_camera(if self.video.shockwaves {
                                self.slowmo.camera()
                            } else {
                                Camera::default()
                            });
                        }
                    }
                    Screen::Credits(c) => {
                        let tick = c.update(frame as f32, self.config.kids_mode);
                        if tick.stopped {
                            self.audio.play(Sfx::CreditsStop);
                        }
                        for n in 0..tick.splashes {
                            self.audio.play(if n % 2 == 0 {
                                Sfx::Splash1
                            } else {
                                Sfx::Splash2
                            });
                        }
                    }
                    Screen::Intro(i) => {
                        let done = i.update(frame as f32) == Flow::Back;
                        if let Some(sfx) = i.sound.take() {
                            self.audio.play(sfx);
                        }
                        if done {
                            self.screen = Screen::Menu;
                        }
                    }
                    _ => self.acc = 0.0,
                }
                if !matches!(self.screen, Screen::Game) {
                    if let Some((_, r)) = self.gfx.as_mut() {
                        r.set_shake(0.0);
                        r.set_camera(Camera::default());
                    }
                }

                let time = self.started.elapsed().as_secs_f32();
                if let (Some((w, r)), Some(a)) = (self.gfx.as_mut(), self.assets.as_ref()) {
                    match (&self.screen, &self.world) {
                        (Screen::Game, Some(world)) => draw::draw(
                            r,
                            a,
                            // Entre dos ticks: lo que sobra en el acumulador dice cuánto.
                            &match (&self.frozen, &self.prev) {
                                (Some(f), _) => f.clone(),
                                (None, Some(p)) => {
                                    interp::view(world, p, (self.acc * TICK_HZ as f64) as f32)
                                }
                                (None, None) => world.clone(),
                            },
                            &self.ui,
                            &self.sparks,
                            &self.players,
                            self.paused,
                        ),
                        (Screen::PlayerSetup(s), _) => {
                            s.draw(r, a, &self.config, time);
                            r.render();
                        }
                        (Screen::MapSelect(s), _) => {
                            s.draw(r, a, &self.config);
                            r.render();
                        }
                        (Screen::MapEditor(s), _) => {
                            s.draw(r, a, &self.config);
                            r.render();
                        }
                        (Screen::Intro(s), _) => {
                            s.draw(r, a);
                            r.render();
                        }
                        (Screen::Credits(s), _) => {
                            s.draw(r, a);
                            r.render();
                        }
                        (Screen::Help(s), _) => {
                            s.draw(r, a);
                            r.render();
                        }
                        _ => {
                            self.menu.draw(r, a);
                            r.render();
                        }
                    }
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    // `--screenshots DIR`: recorre las pantallas del juego, guarda un PNG de cada una (y un GIF
    // de una partida) en DIR y sale. No lee ni escribe la configuración del usuario.
    let mut args = std::env::args().skip(1);
    let shots_dir = match args.next().as_deref() {
        Some("--screenshots") => args.next().map(PathBuf::from),
        _ => None,
    };

    let (config_path, config, video_path, video) = if shots_dir.is_some() {
        (None, Config::default(), None, Video::default())
    } else {
        let path = Config::default_path();
        let config = path.as_deref().map(Config::load).unwrap_or_default();
        let video_path = Video::default_path();
        let video = video_path.as_deref().map(Video::load).unwrap_or_default();
        (path, config, video_path, video)
    };
    let mut menu = Menu::new(&config);
    menu.sync_video(&video);

    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut App {
        screen: if shots_dir.is_some() {
            Screen::Menu
        } else {
            Screen::Intro(Intro::new(clock_seed()))
        },
        config,
        config_path,
        video,
        video_path,
        menu,
        world: None,
        players: Vec::new(),
        controls: Vec::new(),
        joysticks: Joysticks::new(),
        assets: None,
        ui: Ui::new(),
        sparks: Sparks::new(),
        prev: None,
        slowmo: slowmo::SlowMo::new(),
        frozen: None,
        keys: HashSet::new(),
        bomb_latch: [false; 3],
        gfx: None,
        last: Instant::now(),
        acc: 0.0,
        started: Instant::now(),
        audio: Audio::new(),
        play_music: false,
        paused: false,
        shots: shots_dir.map(shots::Shots::new),
        scripted: false,
    })
    .unwrap();
}

fn is_arrow(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight
    )
}
