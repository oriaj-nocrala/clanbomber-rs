use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use cb_core::{Dir, Input, Map, Tile, World, MAP_HEIGHT, MAP_WIDTH, TICK_HZ, TILE_PIXELS};
use cb_render::{Renderer, Sheet};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Tema visual (0..6): cada tema ocupa 4 celdas en maptiles.png.
const THEME: u32 = 0;

struct Sprites {
    maptiles: Sheet,
    addons: Sheet,
    /// Skins 0 (snake) y 1 (tux): celdas de 40x60 (Resources::Bombers).
    bombers: [Sheet; 2],
    /// bombs.png: 4 frames por color de bomber (celdas de 40x40, 8 columnas).
    bombs: Sheet,
    /// explosion2.png: 7 columnas (tipo de segmento) x 3 filas (frame de animación).
    explosion: Sheet,
}

/// Offset de dibujo de los bombers de 40x60 (Bomber::Bomber: offset_x = 60, offset_y = 11).
const BOMBER_OFFSET: (f32, f32) = (60.0, 11.0);

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
    sprites: Option<Sprites>,
    keys: HashSet<KeyCode>,
    /// Pulsaciones de bomba pendientes de consumir en el próximo tick (una por flanco).
    bomb_latch: [bool; 2],
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
                    // Caja alcanzada: sprite_nr++ (MapTile_Box::destroy).
                    Tile::Box => (
                        if self.world.map.is_crumbling(x as i32, y as i32) {
                            3
                        } else {
                            2
                        },
                        None,
                    ),
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
        // Orden de z del original: explosiones (2000) < bombas (3000+y) < bombers (4000+y).
        let at = |tx: i32, ty: i32| (ox + (tx * 40) as f32, oy + (ty * 40) as f32);
        for e in self.world.explosions() {
            // Índices de segmento: Explosion.h (EXPLODE_LEFT..EXPLODE_X); +anim (0, 7 o 14).
            let (left, h, right, up, v, down, center) = (0, 1, 2, 3, 4, 5, 6);
            let a = e.anim();
            let (cx, cy) = at(e.tx, e.ty);
            r.sheet_sprite(&sp.explosion, center + a, cx, cy);
            for i in 1..=e.left {
                let (px, py) = at(e.tx - i, e.ty);
                r.sheet_sprite(
                    &sp.explosion,
                    if i == e.power { left } else { h } + a,
                    px,
                    py,
                );
            }
            for i in 1..=e.right {
                let (px, py) = at(e.tx + i, e.ty);
                r.sheet_sprite(
                    &sp.explosion,
                    if i == e.power { right } else { h } + a,
                    px,
                    py,
                );
            }
            for i in 1..=e.up {
                let (px, py) = at(e.tx, e.ty - i);
                r.sheet_sprite(&sp.explosion, if i == e.power { up } else { v } + a, px, py);
            }
            for i in 1..=e.down {
                let (px, py) = at(e.tx, e.ty + i);
                r.sheet_sprite(
                    &sp.explosion,
                    if i == e.power { down } else { v } + a,
                    px,
                    py,
                );
            }
        }
        for b in self.world.bombs() {
            // sprite_nr = color * 4 + frame (color = skin del bomber que la puso).
            let color = b.owner as u32;
            r.sheet_sprite(
                &sp.bombs,
                color * 4 + b.frame(),
                ox + b.x as f32,
                oy + b.y as f32,
            );
        }
        for (b, sheet) in self
            .world
            .bombers
            .iter()
            .zip(sp.bombers.iter().cycle())
            .filter(|(b, _)| !b.dead)
        {
            r.sheet_sprite(
                sheet,
                b.sprite_nr(),
                b.get_x() as f32 + BOMBER_OFFSET.0,
                b.get_y() as f32 + BOMBER_OFFSET.1,
            );
        }
        r.render();
    }

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
        let (bombs_tex, expl_tex) = (
            load(&mut renderer, "bombs.png"),
            load(&mut renderer, "explosion2.png"),
        );
        let (snake, tux) = (
            load(&mut renderer, "bomber_snake.png"),
            load(&mut renderer, "bomber_tux.png"),
        );
        self.sprites = Some(Sprites {
            maptiles: renderer.sheet(mt, 40.0, 40.0),
            addons: renderer.sheet(ad, 40.0, 40.0),
            bombs: renderer.sheet(bombs_tex, 40.0, 40.0),
            explosion: renderer.sheet(expl_tex, 40.0, 40.0),
            bombers: [
                renderer.sheet(snake, 40.0, 60.0),
                renderer.sheet(tux, 40.0, 60.0),
            ],
        });
        self.gfx = Some((window, renderer));
        self.last = Instant::now();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if event.state.is_pressed() {
                        if code == KeyCode::Escape {
                            el.exit();
                        }
                        if !event.repeat {
                            for (i, k) in KEYMAPS.iter().enumerate() {
                                if code == k[4] {
                                    self.bomb_latch[i] = true;
                                }
                            }
                        }
                        self.keys.insert(code);
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
                self.acc += (now - self.last).as_secs_f64().min(0.25);
                self.last = now;
                let dt = 1.0 / TICK_HZ as f64;
                while self.acc >= dt {
                    let inputs = self.inputs();
                    self.world.step(&inputs);
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
    let mut world = World::new(map);
    world.spawn_bombers(2);
    el.run_app(&mut App {
        world,
        sprites: None,
        keys: HashSet::new(),
        bomb_latch: [false; 2],
        gfx: None,
        last: Instant::now(),
        acc: 0.0,
    })
    .unwrap();
}
