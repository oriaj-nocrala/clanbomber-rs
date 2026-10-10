//! Bomber: movimiento por píxeles replicado de `GameObject::move_*` y `Bomber::act`
//! (ClanBomber2: GameObject.cpp, Bomber.cpp).
//!
//! Las posiciones son píxeles dentro del mapa (0..680 x 0..520), como `x`/`y` del
//! original: `f32` internamente, enteros truncados (`get_x`) en las comprobaciones.

use crate::map::{Dir, Map, TILE_PIXELS};

/// Teclas pulsadas por un jugador en este tick.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub bomb: bool,
}

#[derive(Clone, Debug)]
pub struct Bomber {
    pub x: f32,
    pub y: f32,
    pub dir: Dir,
    /// Píxeles por segundo: `90 + skateboards * 40` (Bomber::reset).
    pub speed: i32,
    /// Fracción de píxel acumulada entre ticks (`remainder` del original).
    remainder: f32,
    /// Contador de animación; el frame es su parte entera (1..=8 al caminar).
    pub anim_count: f32,
}

const T: i32 = TILE_PIXELS as i32;

impl Bomber {
    /// `orig_x`/`orig_y` en píxeles. Mira hacia la izquierda si nace a la derecha de x=300.
    pub fn new(orig_x: i32, orig_y: i32) -> Self {
        Bomber {
            x: orig_x as f32,
            y: orig_y as f32,
            dir: if orig_x > 300 { Dir::Left } else { Dir::Right },
            speed: 90,
            remainder: 0.0,
            anim_count: 0.0,
        }
    }

    pub fn get_x(&self) -> i32 {
        self.x as i32
    }

    pub fn get_y(&self) -> i32 {
        self.y as i32
    }

    /// Índice de sprite: `cur_dir * 10 + (int)anim_count`.
    pub fn sprite_nr(&self) -> u32 {
        self.dir.index() * 10 + self.anim_count as u32
    }

    /// Parte de `Bomber::act` que mueve al bomber. `dt` en segundos.
    pub fn act(&mut self, dt: f32, input: Input, map: &Map) {
        // Prioridad del original: izquierda, arriba, derecha, abajo.
        let dir = if input.left {
            Some(Dir::Left)
        } else if input.up {
            Some(Dir::Up)
        } else if input.right {
            Some(Dir::Right)
        } else if input.down {
            Some(Dir::Down)
        } else {
            None
        };

        match dir {
            Some(d) => {
                self.dir = d;
                // `speed/90` es división entera en el original.
                self.anim_count += dt * 20.0 * (self.speed / 90) as f32;
                self.step(dt, map);
            }
            None => self.anim_count = 0.0,
        }
        if self.anim_count >= 9.0 {
            self.anim_count = 1.0;
        }
    }

    /// `GameObject::move()`: avanza `span` píxeles de uno en uno; se detiene al chocar.
    fn step(&mut self, dt: f32, map: &Map) {
        let advance = dt * self.speed as f32;
        let mut span = advance as i32;
        self.remainder += advance - (advance as i32) as f32;
        span += self.remainder as i32;
        self.remainder -= (self.remainder as i32) as f32;

        for _ in 0..span {
            let moved = match self.dir {
                Dir::Left => self.move_left(map),
                Dir::Right => self.move_right(map),
                Dir::Up => self.move_up(map),
                Dir::Down => self.move_down(map),
            };
            if !moved {
                return;
            }
        }
    }

    // Los cuatro `move_*` conservan la estructura del original, incluido el ajuste
    // lateral (`y++`/`y--`) que desliza al bomber alrededor de las esquinas.
    // TODO(bombas): `maptile->bomb` bloquea el paso y permite patear; llega con las bombas.

    fn move_right(&mut self, map: &Map) -> bool {
        if map.blocking_px(self.x as i32 + T, self.y as i32 + 20) {
            return false;
        }
        self.x += 1.0;
        let (gx, gy) = (self.get_x(), self.get_y());
        if gy % T > 19 {
            if map.blocking_tile((gx + T) / T, (gy - 20) / T) {
                self.y += 1.0;
            }
        } else if gy % T > 0 && map.blocking_tile((gx + T) / T, (gy + 60) / T) {
            self.y -= 1.0;
        }
        true
    }

    fn move_left(&mut self, map: &Map) -> bool {
        if map.blocking_px(self.x as i32 - 1, self.y as i32 + 20) {
            return false;
        }
        self.x -= 1.0;
        let (gx, gy) = (self.get_x(), self.get_y());
        if gy % T > 19 {
            if map.blocking_tile((gx - 1) / T, (gy - 20) / T) {
                self.y += 1.0;
            }
        } else if gy % T > 0 && map.blocking_tile((gx - 1) / T, (gy + 60) / T) {
            self.y -= 1.0;
        }
        true
    }

    fn move_up(&mut self, map: &Map) -> bool {
        if map.blocking_px(self.x as i32 + 20, self.y as i32 - 1) {
            return false;
        }
        self.y -= 1.0;
        let (gx, gy) = (self.get_x(), self.get_y());
        if gx % T > 19 {
            if map.blocking_tile((gx - 20) / T, (gy - 1) / T) {
                self.x += 1.0;
            }
        } else if gx % T > 0 && map.blocking_tile((gx + 60) / T, (gy - 1) / T) {
            self.x -= 1.0;
        }
        true
    }

    fn move_down(&mut self, map: &Map) -> bool {
        if map.blocking_px(self.x as i32 + 20, self.y as i32 + T) {
            return false;
        }
        self.y += 1.0;
        let (gx, gy) = (self.get_x(), self.get_y());
        if gx % T > 19 {
            if map.blocking_tile((gx - 20) / T, (gy + T) / T) {
                self.x += 1.0;
            }
        } else if gx % T > 0 && map.blocking_tile((gx + 60) / T, (gy + T) / T) {
            self.x -= 1.0;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> Map {
        Map::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/maps/Arena.map"
        ))
        .unwrap()
    }

    const RIGHT: Input = Input {
        right: true,
        left: false,
        up: false,
        down: false,
        bomb: false,
    };
    const DOWN: Input = Input {
        down: true,
        left: false,
        up: false,
        right: false,
        bomb: false,
    };

    fn open_field() -> Map {
        Map::parse(&("x\n1\n".to_owned() + &" ".repeat(17 * 13))).unwrap()
    }

    #[test]
    fn walks_at_90_px_per_second() {
        let map = open_field();
        let mut b = Bomber::new(100, 100);
        for _ in 0..60 {
            b.act(1.0 / 60.0, DOWN, &map);
        }
        // 60 ticks a 1/60 s = 1 s -> ~90 px (el resto fraccionario se acumula).
        assert!((b.y - 100.0 - 90.0).abs() <= 1.0, "y={}", b.y);
    }

    #[test]
    fn stops_at_wall() {
        let map = arena();
        // Fila 6: "*2 ++++ 4 ++++ 3*": muro en x=0 y x=16.
        let mut b = Bomber::new(15 * 40, 6 * 40);
        for _ in 0..600 {
            b.act(1.0 / 60.0, RIGHT, &map);
        }
        assert_eq!(b.get_x(), 15 * 40);
    }

    #[test]
    fn spawn_direction_follows_original() {
        assert_eq!(Bomber::new(40, 40).dir, Dir::Right);
        assert_eq!(Bomber::new(15 * 40, 40).dir, Dir::Left);
    }
}
