//! Bombas y explosiones (ClanBomber2: Bomb.cpp, Explosion.cpp).

use crate::extra::Extra;
use crate::flight::Flight;
use crate::map::{Dir, Map, TILE_PIXELS};

const T: i32 = TILE_PIXELS as i32;

/// `Config::bomb_countdown / 10` segundos.
pub const BOMB_COUNTDOWN: f32 = 2.5;
/// `Config::bomb_delay / 100`: retardo máximo de una bomba alcanzada por otra explosión.
pub const BOMB_DELAY: f32 = 0.1;
/// `Explosion::detonation_period`.
pub const EXPLOSION_PERIOD: f32 = 0.5;

/// `Config::bomb_speed`: píxeles por segundo al patearla y base de la velocidad de vuelo.
pub const BOMB_SPEED: i32 = 160;

#[derive(Clone, Debug)]
pub struct Bomb {
    /// Posición en píxeles dentro del mapa (múltiplo de 40 mientras no se patee o lance).
    pub x: f32,
    pub y: f32,
    pub power: i32,
    /// Índice del bomber que la puso.
    pub owner: usize,
    pub countdown: f32,
    /// Acumulador de animación: frame = `(cur_scale as i32) % 4`.
    pub cur_scale: f32,
    pub exploding: bool,
    pub delete_me: bool,
    /// Dirección de desplazamiento (patada o lanzamiento); `None` = quieta.
    pub cur_dir: Option<Dir>,
    pub speed: i32,
    pub remainder: f32,
    pub flight: Flight,
    /// `can_fly_over_walls`: solo la trampa lanza bombas por encima de los muros.
    pub fly_over_walls: bool,
    /// `stopped`: la bomba acaba de frenar este tick (las flechas la realinean).
    pub stopped: bool,
    pub falling: bool,
    pub fall_countdown: f32,
    pub fallen_down: bool,
}

impl Bomb {
    pub fn new(tx: i32, ty: i32, power: i32, owner: usize) -> Self {
        Bomb {
            x: (tx * T) as f32,
            y: (ty * T) as f32,
            power,
            owner,
            countdown: BOMB_COUNTDOWN,
            cur_scale: 0.0,
            exploding: false,
            delete_me: false,
            cur_dir: None,
            speed: BOMB_SPEED,
            remainder: 0.0,
            flight: Flight::default(),
            fly_over_walls: false,
            stopped: false,
            falling: false,
            fall_countdown: 0.0,
            fallen_down: false,
        }
    }

    pub fn get_x(&self) -> i32 {
        self.x as i32
    }

    pub fn get_y(&self) -> i32 {
        self.y as i32
    }

    /// Casilla bajo el centro (`get_maptile()`).
    pub fn tile(&self) -> (i32, i32) {
        ((self.get_x() + 20) / T, (self.get_y() + 20) / T)
    }

    /// ¿Cuenta como `tile->bomb`? El original excluye las que vuelan o caen (`maptilize_objects`).
    pub fn rests_on_tile(&self) -> bool {
        !self.flight.active && !self.falling
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

    /// `Bomb::kick()`: empieza a deslizarse (no si está en el aire).
    pub fn kick(&mut self, dir: Dir) {
        if !self.flight.active {
            self.cur_dir = Some(dir);
        }
    }

    /// `Bomb::throww(dir, fields)`: vuela `fields` casillas (4 al lanzarla con el guante).
    pub fn throw(&mut self, dir: Dir, fields: i32) {
        self.fly_over_walls = false;
        if fields > 0 && !self.flight.active {
            self.cur_dir = Some(dir);
            let (x, y) = (self.get_x(), self.get_y());
            let d = fields * T;
            match dir {
                Dir::Up => self.fly_to(x, y - d),
                Dir::Down => self.fly_to(x, y + d),
                Dir::Left => self.fly_to(x - d, y),
                Dir::Right => self.fly_to(x + d, y),
            }
        }
    }

    fn fly_to(&mut self, x: i32, y: i32) {
        if !self.falling && !self.fallen_down {
            self.flight.start((self.x, self.y), (x, y), self.speed);
        }
    }

    /// `continue_flying`: los muros frenan el vuelo salvo con `fly_over_walls`.
    pub fn continue_flying(&mut self, map: &Map, dt: f32) {
        let mut pos = (self.x, self.y);
        let walls = if self.fly_over_walls { None } else { Some(map) };
        self.flight.advance(&mut pos, dt, walls);
        (self.x, self.y) = pos;
    }

    /// `GameObject::fall()`.
    pub fn fall(&mut self) {
        if !self.falling {
            self.falling = true;
            self.fall_countdown = 1.0;
        }
    }

    /// `GameObject::continue_falling()`.
    pub fn continue_falling(&mut self, dt: f32) {
        self.speed = (self.fall_countdown * 60.0) as i32;
        self.fall_countdown -= dt;
        if self.fall_countdown < 0.0 {
            self.fallen_down = true;
            self.fall_countdown = 0.0;
        }
    }

    /// `GameObject::snap()`: alinea con la casilla más cercana.
    pub fn snap(&mut self) {
        self.x = (((self.get_x() + 20) / T) * T) as f32;
        self.y = (((self.get_y() + 20) / T) * T) as f32;
    }

    /// `Bomb::stop()`: se detiene y se alinea con la casilla.
    pub fn stop(&mut self) {
        self.stopped = true;
        if !self.flight.active {
            self.cur_dir = None;
            self.snap();
        }
    }

    /// `GameObject::fly_to(MapTile*)`: sale despedida hacia la esquina de la casilla a su
    /// velocidad actual (la trampa la fija en 200 px/s).
    pub fn fly_to_tile(&mut self, tile: (usize, usize)) {
        self.fly_to(tile.0 as i32 * T, tile.1 as i32 * T);
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
