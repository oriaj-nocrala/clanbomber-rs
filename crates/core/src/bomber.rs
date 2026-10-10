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
    pub dead: bool,
    /// Bombas que puede colocar ahora mismo (`cur_bombs`); sube al explotar una suya.
    pub cur_bombs: i32,
    /// Bombas simultáneas máximas (`Config::start_bombs`).
    pub bombs: i32,
    /// Alcance de la explosión (`Config::start_power`).
    pub power: i32,
    pub kills: i32,
    pub deaths: i32,
    pub can_kick: bool,
    pub gloves: i32,
    pub skateboards: i32,
}

const T: i32 = TILE_PIXELS as i32;

// Config::max_* del original.
const MAX_SKATEBOARDS: i32 = 5;
const MAX_POWER: i32 = 12;
const MAX_BOMBS: i32 = 9;

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
            dead: false,
            cur_bombs: 1,
            bombs: 1,
            power: 1,
            kills: 0,
            deaths: 0,
            can_kick: false,
            gloves: 0,
            skateboards: 0,
        }
    }

    /// Casilla (en tiles) sobre la que está el bomber: `get_maptile()` del original.
    pub fn tile(&self) -> (i32, i32) {
        ((self.get_x() + 20) / T, (self.get_y() + 20) / T)
    }

    /// `Bomber::gain_extra_*`; los máximos son los de `Config::max_*` del original.
    pub fn gain_extra_bomb(&mut self) {
        if MAX_BOMBS > self.bombs {
            self.bombs += 1;
            self.cur_bombs += 1;
        }
    }

    pub fn gain_extra_power(&mut self) {
        if MAX_POWER > self.power {
            self.power += 1;
        }
    }

    pub fn gain_extra_skateboard(&mut self) {
        if MAX_SKATEBOARDS > self.skateboards {
            self.skateboards += 1;
            self.speed += 40;
        }
    }

    pub fn gain_extra_kick(&mut self) {
        self.can_kick = true;
    }

    pub fn gain_extra_glove(&mut self) {
        self.gloves += 1;
    }

    /// `Bomber::die()`. Devuelve `true` si acaba de morir.
    /// TODO(cadáver): el original crea un `Bomber_Corpse` y suelta los extras.
    pub fn die(&mut self) -> bool {
        if self.dead {
            return false;
        }
        self.dead = true;
        self.deaths += 1;
        self.x = 0.0;
        self.y = 0.0;
        true
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

    /// `Bomber::act` completo salvo enfermedades, caída y guantes.
    /// `bomb_at(tx, ty)`: ¿hay una bomba en esa casilla? Devuelve `true` si quiere colocar
    /// una bomba (la crea el `World`, que es quien las posee).
    pub fn act(
        &mut self,
        dt: f32,
        input: Input,
        map: &Map,
        bomb_at: &dyn Fn(i32, i32) -> bool,
    ) -> bool {
        if self.dead {
            return false;
        }
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
                self.step(dt, map, bomb_at);
            }
            None => self.anim_count = 0.0,
        }
        if self.anim_count >= 9.0 {
            self.anim_count = 1.0;
        }

        // Con una bomba bajo los pies no se puede poner otra (los guantes la lanzarían: TODO).
        input.bomb && self.cur_bombs > 0 && !bomb_at(self.tile().0, self.tile().1)
    }

    /// `GameObject::move()`: avanza `span` píxeles de uno en uno; se detiene al chocar.
    fn step(&mut self, dt: f32, map: &Map, bomb_at: &dyn Fn(i32, i32) -> bool) {
        let advance = dt * self.speed as f32;
        let mut span = advance as i32;
        self.remainder += advance - (advance as i32) as f32;
        span += self.remainder as i32;
        self.remainder -= (self.remainder as i32) as f32;

        for _ in 0..span {
            let moved = match self.dir {
                Dir::Left => self.move_left(map, bomb_at),
                Dir::Right => self.move_right(map, bomb_at),
                Dir::Up => self.move_up(map, bomb_at),
                Dir::Down => self.move_down(map, bomb_at),
            };
            if !moved {
                return;
            }
        }
    }

    // Los cuatro `move_*` conservan la estructura del original, incluido el ajuste
    // lateral (`y++`/`y--`) que desliza al bomber alrededor de las esquinas.
    // Una bomba en la casilla de destino bloquea salvo que el bomber ya esté más allá de la
    // mitad de su casilla (`get_x() % 40 > 19`, etc.). TODO(patada): con `can_kick` la empuja.

    fn move_right(&mut self, map: &Map, bomb_at: &dyn Fn(i32, i32) -> bool) -> bool {
        if map.blocking_px(self.x as i32 + T, self.y as i32 + 20) {
            return false;
        }
        if bomb_at((self.get_x() + T) / T, (self.get_y() + 20) / T) && self.get_x() % T <= 19 {
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

    fn move_left(&mut self, map: &Map, bomb_at: &dyn Fn(i32, i32) -> bool) -> bool {
        if map.blocking_px(self.x as i32 - 1, self.y as i32 + 20) {
            return false;
        }
        if bomb_at((self.get_x() - 1) / T, (self.get_y() + 20) / T) && self.get_x() % T >= 20 {
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

    fn move_up(&mut self, map: &Map, bomb_at: &dyn Fn(i32, i32) -> bool) -> bool {
        if map.blocking_px(self.x as i32 + 20, self.y as i32 - 1) {
            return false;
        }
        if bomb_at((self.get_x() + 20) / T, (self.get_y() - 1) / T) && self.get_y() % T >= 20 {
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

    fn move_down(&mut self, map: &Map, bomb_at: &dyn Fn(i32, i32) -> bool) -> bool {
        if map.blocking_px(self.x as i32 + 20, self.y as i32 + T) {
            return false;
        }
        if bomb_at((self.get_x() + 20) / T, (self.get_y() + T) / T) && self.get_y() % T <= 19 {
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
            b.act(1.0 / 60.0, DOWN, &map, &|_, _| false);
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
            b.act(1.0 / 60.0, RIGHT, &map, &|_, _| false);
        }
        assert_eq!(b.get_x(), 15 * 40);
    }

    #[test]
    fn spawn_direction_follows_original() {
        assert_eq!(Bomber::new(40, 40).dir, Dir::Right);
        assert_eq!(Bomber::new(15 * 40, 40).dir, Dir::Left);
    }
}
