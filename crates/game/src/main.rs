mod assets;
mod credits;
mod draw;
mod help;
mod map_select;
mod menu;
mod player_setup;
mod screen;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use cb_core::{Config, ControllerKind, Input, Map, MapList, Phase, Rng, World, TICK_HZ};
use cb_render::Renderer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use assets::Assets;
use credits::Credits;
use draw::{Player, Ui};
use help::Help;
use map_select::MapSelect;
use menu::{Id, Menu, Outcome};
use player_setup::PlayerSetup;
use screen::Flow;

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

enum Screen {
    Menu,
    Game,
    PlayerSetup(PlayerSetup),
    MapSelect(MapSelect),
    Credits(Credits),
    Help(Help),
}

struct App {
    screen: Screen,
    config: Config,
    config_path: Option<PathBuf>,
    menu: Menu,
    world: Option<World>,
    players: Vec<Player>,
    /// Teclado que gobierna a cada bomber del mundo (`None`: sin controlador implementado).
    keymaps: Vec<Option<usize>>,
    assets: Option<Assets>,
    ui: Ui,
    keys: HashSet<KeyCode>,
    /// Pulsaciones de bomba pendientes de consumir en el próximo tick (una por flanco y teclado).
    bomb_latch: [bool; 3],
    gfx: Option<(Arc<Window>, Renderer)>,
    last: Instant,
    acc: f64,
    started: Instant,
}

fn maps_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps")
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
        if self.config.number_of_opponents() < 2 {
            return;
        }
        let players: Vec<_> = self.config.enabled_bombers().cloned().collect();
        let seed = clock_seed();
        let Ok(mut list) = MapList::scan(maps_dir()) else {
            return;
        };
        if list.paths.is_empty() {
            return;
        }
        list.disabled = self.config.disabled_maps.clone();
        // Primer mapa: el elegido (`start_map`) o uno al azar; salta a uno activo donde quepan todos.
        let first = if self.config.random_map_order {
            list.random_for(players.len(), &mut Rng::new(seed))
        } else {
            list.index = (self.config.start_map.max(0) as usize).min(list.paths.len() - 1);
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
            return;
        };

        let mut world = World::with_config(map, seed, self.config.clone());
        world.playlist = Some(list);
        world.start_match(players.len());
        self.keymaps = players
            .iter()
            .map(|p| match p.controller {
                ControllerKind::Keymap(n) => Some(n as usize - 1),
                // TODO(IA, ratón, joystick): sin controlador todavía; el bomber se queda quieto.
                _ => None,
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
        self.screen = Screen::Game;
        self.acc = 0.0;
        self.last = Instant::now();
    }

    fn open_map_select(&mut self) {
        let Some(a) = self.assets.as_ref() else {
            return;
        };
        if let Some(ms) = MapSelect::new(&maps_dir(), &self.config, a) {
            self.screen = Screen::MapSelect(ms);
        }
    }

    /// La tecla de bomba actúa solo al pulsar (Controller_Keyboard::update: flanco de subida).
    /// Si dos bombers comparten teclado, solo el primero recibe cada pulsación.
    fn inputs(&mut self) -> Vec<Input> {
        let keys = &self.keys;
        let latch = &mut self.bomb_latch;
        self.keymaps
            .iter()
            .map(|km| match *km {
                None => Input::default(),
                Some(i) => {
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
        let (flow, save) = match &mut self.screen {
            Screen::PlayerSetup(s) => s.key(code, text, &mut self.config),
            Screen::MapSelect(s) => s.key(code, &mut self.config),
            Screen::Credits(s) => (s.key(code, true), false),
            Screen::Help(s) => (s.key(), false),
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
        }
    }

    fn menu_key(&mut self, el: &ActiveEventLoop, code: KeyCode) {
        match self.menu.key(code) {
            Outcome::Activate(Id::Quit) => el.exit(),
            Outcome::Activate(Id::StartLocal) => self.start_game(),
            Outcome::Activate(Id::PlayerSetup) => {
                self.screen = Screen::PlayerSetup(PlayerSetup::new())
            }
            Outcome::Activate(Id::MapSelection) => self.open_map_select(),
            Outcome::Activate(Id::Credits) => self.screen = Screen::Credits(Credits::new()),
            Outcome::Activate(Id::Help) => self.screen = Screen::Help(Help::new()),
            Outcome::Activate(_) | Outcome::None => {}
            Outcome::Changed(id, v) => {
                id.set(&mut self.config, v);
                self.menu.sync(&self.config);
                self.save_config();
            }
        }
    }

    fn game_key(&mut self, code: KeyCode, repeat: bool) {
        match code {
            // `quit current game`: de vuelta al menú.
            KeyCode::Escape => {
                self.screen = Screen::Menu;
                self.world = None;
                return;
            }
            // Del marcador a la siguiente ronda; tras la última, de vuelta al menú
            // (GameStatus::show espera la barra espaciadora).
            KeyCode::Space if !repeat => {
                if let Some(w) = self.world.as_mut() {
                    if let Phase::Scores { end_of_game, .. } = w.phase {
                        if end_of_game {
                            self.screen = Screen::Menu;
                            self.world = None;
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
                let now = Instant::now();
                let frame = (now - self.last).as_secs_f64().min(0.25);
                self.last = now;
                match &mut self.screen {
                    Screen::Game => {
                        // Paso fijo: la simulación avanza en ticks de 1/TICK_HZ, independiente del framerate.
                        self.acc += frame;
                        let dt = 1.0 / TICK_HZ as f64;
                        while self.acc >= dt {
                            let inputs = self.inputs();
                            if let Some(w) = self.world.as_mut() {
                                w.step(&inputs);
                            }
                            self.acc -= dt;
                        }
                        if let Some(w) = &self.world {
                            self.ui.update(w, frame as f32);
                        }
                    }
                    Screen::Credits(c) => c.update(frame as f32),
                    _ => self.acc = 0.0,
                }

                let time = self.started.elapsed().as_secs_f32();
                if let (Some((w, r)), Some(a)) = (self.gfx.as_mut(), self.assets.as_ref()) {
                    match (&self.screen, &self.world) {
                        (Screen::Game, Some(world)) => {
                            draw::draw(r, a, world, &self.ui, &self.players)
                        }
                        (Screen::PlayerSetup(s), _) => {
                            s.draw(r, a, &self.config, time);
                            r.render();
                        }
                        (Screen::MapSelect(s), _) => {
                            s.draw(r, a, &self.config);
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
    let config_path = Config::default_path();
    let config = config_path.as_deref().map(Config::load).unwrap_or_default();
    let menu = Menu::new(&config);

    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut App {
        screen: Screen::Menu,
        config,
        config_path,
        menu,
        world: None,
        players: Vec::new(),
        keymaps: Vec::new(),
        assets: None,
        ui: Ui::new(),
        keys: HashSet::new(),
        bomb_latch: [false; 3],
        gfx: None,
        last: Instant::now(),
        acc: 0.0,
        started: Instant::now(),
    })
    .unwrap();
}
