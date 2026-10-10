//! Bombas y explosiones (ClanBomber2: Bomb.cpp, Explosion.cpp).

use crate::extra::Extra;
use crate::map::{Map, TILE_PIXELS};

const T: i32 = TILE_PIXELS as i32;

/// `Config::bomb_countdown / 10` segundos.
pub const BOMB_COUNTDOWN: f32 = 2.5;
/// `Config::bomb_delay / 100`: retardo máximo de una bomba alcanzada por otra explosión.
pub const BOMB_DELAY: f32 = 0.1;
/// `Explosion::detonation_period`.
pub const EXPLOSION_PERIOD: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct Bomb {
    /// Posición en píxeles dentro del mapa (múltiplo de 40 mientras no se patee).
    pub x: i32,
    pub y: i32,
    pub power: i32,
    /// Índice del bomber que la puso.
    pub owner: usize,
    pub countdown: f32,
    /// Acumulador de animación: frame = `(cur_scale as i32) % 4`.
    pub cur_scale: f32,
    pub exploding: bool,
    pub delete_me: bool,
}

impl Bomb {
    pub fn new(tx: i32, ty: i32, power: i32, owner: usize) -> Self {
        Bomb {
            x: tx * T,
            y: ty * T,
            power,
            owner,
            countdown: BOMB_COUNTDOWN,
            cur_scale: 0.0,
            exploding: false,
            delete_me: false,
        }
    }

    pub fn tile(&self) -> (i32, i32) {
        ((self.x + 20) / T, (self.y + 20) / T)
    }

    pub fn frame(&self) -> u32 {
        (self.cur_scale as i32 % 4) as u32
    }

    /// `Bomb::explode_delayed()`: acorta la mecha, pero nunca la alarga.
    /// Devuelve `true` si debe explotar ya.
    pub fn explode_delayed(&mut self) -> bool {
        if self.countdown > BOMB_DELAY {
            self.countdown = BOMB_DELAY;
            return self.countdown == 0.0;
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct Explosion {
    /// Casilla central.
    pub tx: i32,
    pub ty: i32,
    pub power: i32,
    pub owner: usize,
    pub left: i32,
    pub right: i32,
    pub up: i32,
    pub down: i32,
    pub period: f32,
    pub delete_me: bool,
    /// Casillas donde `do_destruction` llamó a `destroy()` (para destruir extras ahí).
    pub destroyed_tiles: Vec<(i32, i32)>,
}

impl Explosion {
    /// `Explosion::Explosion`: calcula longitudes y destruye cajas.
    /// TODO(extras): un extra en la casilla también frena el fuego (`has_extra`).
    pub fn new(
        map: &mut Map,
        has_extra: &dyn Fn(i32, i32) -> bool,
        x: i32,
        y: i32,
        power: i32,
        owner: usize,
    ) -> Self {
        let (tx, ty) = ((x + 20) / T, (y + 20) / T);
        let len = |dx: i32, dy: i32| {
            let mut i = 1;
            while i <= power {
                let (px, py) = (tx + dx * i, ty + dy * i);
                if map.blocking_tile(px, py) || has_extra(px, py) {
                    break;
                }
                i += 1;
            }
            i - 1
        };
        let (left, right, up, down) = (len(-1, 0), len(1, 0), len(0, -1), len(0, 1));

        // do_destruction: la casilla central y la que frenó cada brazo (si lo frenó algo).
        let mut destroyed_tiles = vec![(tx, ty)];
        if left < power {
            destroyed_tiles.push((tx - left - 1, ty));
        }
        if right < power {
            destroyed_tiles.push((tx + right + 1, ty));
        }
        if up < power {
            destroyed_tiles.push((tx, ty - up - 1));
        }
        if down < power {
            destroyed_tiles.push((tx, ty + down + 1));
        }
        for &(x, y) in &destroyed_tiles {
            map.destroy(x, y);
        }

        Explosion {
            tx,
            ty,
            power,
            owner,
            left,
            right,
            up,
            down,
            period: EXPLOSION_PERIOD,
            delete_me: false,
            destroyed_tiles,
        }
    }

    /// ¿Está la casilla en la cruz de fuego?
    pub fn covers(&self, bx: i32, by: i32) -> bool {
        (by == self.ty && bx >= self.tx - self.left && bx <= self.tx + self.right)
            || (bx == self.tx && by >= self.ty - self.up && by <= self.ty + self.down)
    }

    /// Frame de animación: 14 al principio, parpadea 7/0/7/14 según `period` (Explosion::show).
    pub fn anim(&self) -> u32 {
        let mut a = 14;
        if self.period < 0.4 {
            a = 7;
        }
        if self.period < 0.3 {
            a = 0;
        }
        if self.period < 0.2 {
            a = 7;
        }
        if self.period < 0.1 {
            a = 14;
        }
        a
    }
}

#[derive(Clone, Debug)]
pub enum Object {
    Bomb(Bomb),
    Explosion(Explosion),
    Extra(Extra),
}

impl Object {
    pub fn delete_me(&self) -> bool {
        match self {
            Object::Bomb(b) => b.delete_me,
            Object::Explosion(e) => e.delete_me,
            Object::Extra(x) => x.delete_me,
        }
    }
}
