use std::sync::Arc;
use std::time::Instant;

use cb_core::{Dir, Map, Tile, World, MAP_HEIGHT, MAP_WIDTH, TICK_HZ, TILE_PIXELS};
use cb_render::{Renderer, Sheet};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Tema visual (0..6): cada tema ocupa 4 celdas en maptiles.png.
const THEME: u32 = 0;

struct Sprites {
    maptiles: Sheet,
    addons: Sheet,
}

struct App {
    world: World,
    sprites: Option<Sprites>,
    gfx: Option<(Arc<Window>, Renderer)>,
    last: Instant,
    acc: f64,
}

impl App {
    fn draw(&mut self) {
        let (Some((_, r)), Some(sp)) = (self.gfx.as_mut(), self.sprites.as_ref()) else {
            return;
        };
        let t = TILE_PIXELS as f32;
        // El mapa mide 680x520; se centra en el lienzo virtual de 800x600 (offset 60,40 como el original).
        let (ox, oy) = (
            (cb_render::VIRTUAL_W - t * MAP_WIDTH as f32) / 2.0,
            (cb_render::VIRTUAL_H - t * MAP_HEIGHT as f32) / 2.0,
        );
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = self.world.map.get(x as i32, y as i32);
                let (px, py) = (ox + x as f32 * t, oy + y as f32 * t);
                // Índices de celda según Map::show() y los MapTile_* del original.
                let (base, addon) = match tile {
                    Tile::None => continue,
                    Tile::Ground => (0, None),
                    Tile::Wall => (1, None),
                    Tile::Box => (2, None),
                    Tile::Ice => (0, Some(0)),
                    // Orden del original: DOWN=0, LEFT=1, UP=2, RIGHT=3; los addons de flecha empiezan en 1.
                    Tile::Arrow(d) => (
                        0,
                        Some(
                            1 + match d {
                                Dir::Down => 0,
                                Dir::Left => 1,
                                Dir::Up => 2,
                                Dir::Right => 3,
                            },
                        ),
                    ),
                    Tile::Trap => (0, Some(7)),
                };
                r.sheet_sprite(&sp.maptiles, THEME * 4 + base, px, py);
                if let Some(a) = addon {
                    r.sheet_sprite(&sp.addons, a, px, py);
                }
            }
        }
        r.render();
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
        let load = |r: &mut Renderer, name: &str| {
            let path = format!("{}/../../assets/pics/{name}", env!("CARGO_MANIFEST_DIR"));
            r.load_texture(&path)
                .unwrap_or_else(|e| panic!("{path}: {e}"))
        };
        let (mt, ad) = (
            load(&mut renderer, "maptiles.png"),
            load(&mut renderer, "maptile_addons.png"),
        );
        self.sprites = Some(Sprites {
            maptiles: renderer.sheet(mt, 40.0, 40.0),
            addons: renderer.sheet(ad, 40.0, 40.0),
        });
        self.gfx = Some((window, renderer));
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state.is_pressed()
                    && event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) =>
            {
                el.exit()
            }
            WindowEvent::Resized(s) => {
                if let Some((_, r)) = self.gfx.as_mut() {
                    r.resize(s.width, s.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // Paso fijo: la simulación avanza en ticks de 1/TICK_HZ, independiente del framerate.
                let now = Instant::now();
                self.acc += (now - self.last).as_secs_f64().min(0.25);
                self.last = now;
                let dt = 1.0 / TICK_HZ as f64;
                while self.acc >= dt {
                    self.world.step();
                    self.acc -= dt;
                }
                self.draw();
                if let Some((w, _)) = &self.gfx {
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
    let map = Map::load(&path).unwrap_or_else(|e| panic!("no se pudo cargar {path}: {e:?}"));
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    el.run_app(&mut App {
        world: World::new(map),
        sprites: None,
        gfx: None,
        last: Instant::now(),
        acc: 0.0,
    })
    .unwrap();
}
