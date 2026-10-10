mod assets;
mod draw;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use cb_core::{Input, Map, MapList, Phase, World, TICK_HZ};
use cb_render::Renderer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use assets::Assets;
use draw::Ui;

/// Teclas de los dos primeros keymaps del original: flechas + Enter, y WASD + Tab.
const KEYMAPS: [[KeyCode; 5]; 2] = [
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
];

struct App {
    world: World,
    assets: Option<Assets>,
    ui: Ui,
    keys: HashSet<KeyCode>,
    /// Pulsaciones de bomba pendientes de consumir en el próximo tick (una por flanco).
    bomb_latch: [bool; 2],
    gfx: Option<(Arc<Window>, Renderer)>,
    last: Instant,
    acc: f64,
}

impl App {
    /// La tecla de bomba actúa solo al pulsar (Controller_Keyboard::update: flanco de subida).
    fn inputs(&mut self) -> Vec<Input> {
        let keys = &self.keys;
        let latch = &mut self.bomb_latch;
        KEYMAPS
            .iter()
            .zip(latch.iter_mut())
            .map(|(k, bomb)| {
                let down = |i: usize| keys.contains(&k[i]);
                Input {
                    left: down(0),
                    right: down(1),
                    up: down(2),
                    down: down(3),
                    bomb: std::mem::take(bomb),
                }
            })
            .collect()
    }

    fn key_pressed(&mut self, el: &ActiveEventLoop, code: KeyCode, repeat: bool) {
        match code {
            KeyCode::Escape => el.exit(),
            // Del marcador a la siguiente ronda (GameStatus::show espera la barra espaciadora).
            KeyCode::Space if !repeat && matches!(self.world.phase, Phase::Scores { .. }) => {
                self.world.confirm();
                self.bomb_latch = [false; 2];
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
        self.keys.insert(code);
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
                        self.key_pressed(el, code, event.repeat);
                    } else {
                        self.keys.remove(&code);
                    }
                }
            }
            WindowEvent::Resized(s) => {
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // Paso fijo: la simulación avanza en ticks de 1/TICK_HZ, independiente del framerate.
                let now = Instant::now();
                let frame = (now - self.last).as_secs_f64().min(0.25);
                self.acc += frame;
                self.last = now;
                let dt = 1.0 / TICK_HZ as f64;
                while self.acc >= dt {
                    let inputs = self.inputs();
                    self.world.step(&inputs);
                    self.acc -= dt;
                }
                self.ui.update(&self.world, frame as f32);
                if let (Some((w, r)), Some(a)) = (self.gfx.as_mut(), self.assets.as_ref()) {
                    draw::draw(r, a, &self.world, &self.ui);
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "assets/maps/Arena.map".into());
    // Semilla distinta en cada partida.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);

    // Las rondas siguientes recorren los mapas de la misma carpeta, en orden alfabético.
    let mut playlist = Path::new(&path)
        .parent()
        .and_then(|dir| MapList::scan(dir).ok());
    if let Some(list) = playlist.as_mut() {
        match list
            .paths
            .iter()
            .position(|p| p.file_name() == Path::new(&path).file_name())
        {
            Some(i) => list.index = i,
            None => playlist = None,
        }
    }
    let map = Map::load(&path).unwrap_or_else(|e| panic!("no se pudo cargar {path}: {e:?}"));
    let mut world = World::with_seed(map, seed);
    world.playlist = playlist;
    world.start_match(KEYMAPS.len());

    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut App {
        world,
        assets: None,
        ui: Ui::new(),
        keys: HashSet::new(),
        bomb_latch: [false; 2],
        gfx: None,
        last: Instant::now(),
        acc: 0.0,
    })
    .unwrap();
}
