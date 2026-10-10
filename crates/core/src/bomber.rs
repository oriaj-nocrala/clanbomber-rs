//! Bomber: movimiento por píxeles replicado de `GameObject::move_*` y `Bomber::act`
//! (ClanBomber2: GameObject.cpp, Bomber.cpp).
//!
//! Las posiciones son píxeles dentro del mapa (0..680 x 0..520), como `x`/`y` del
//! original: `f32` internamente, enteros truncados (`get_x`) en las comprobaciones.

use crate::disease::{Disease, DiseaseKind};
use crate::extra::ExtraKind;
use crate::map::{Dir, TILE_PIXELS};
use crate::movement::{advance, Env, Mover, Pos};

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
    pub falling: bool,
    pub fall_countdown: f32,
    pub fallen_down: bool,
    pub disease: Option<Disease>,
    /// Estado del controlador, que las enfermedades alteran (`Controller`).
    reverse: bool,
    bomb_always: bool,
    active: bool,
    /// Extras ganados por tipo, para soltarlos al morir (`extra_bombs`, ...).
    extra_bombs: i32,
    extra_power: i32,
    extra_skateboards: i32,
    extra_kick: i32,
    extra_gloves: i32,
    /// Extras que este bomber ha soltado y el `World` debe colocar: (tipo, x, y en píxeles).
    pub drops: Vec<(ExtraKind, i32, i32)>,
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
            falling: false,
            fall_countdown: 0.0,
            fallen_down: false,
            disease: None,
            reverse: false,
            bomb_always: false,
            active: true,
            extra_bombs: 0,
            extra_power: 0,
            extra_skateboards: 0,
            extra_kick: 0,
            extra_gloves: 0,
            drops: Vec::new(),
        }
    }

    /// Casilla (en tiles) sobre la que está el bomber: `get_maptile()` del original.
    pub fn tile(&self) -> (i32, i32) {
        ((self.get_x() + 20) / T, (self.get_y() + 20) / T)
    }

    /// `Bomber::loose_all_extras`: suelta cada extra ganado (el `World` los coloca y lanza).
    fn loose_all_extras(&mut self) {
        let (x, y) = (self.get_x(), self.get_y());
        for (kind, n) in [
            (ExtraKind::Bomb, &mut self.extra_bombs),
            (ExtraKind::Power, &mut self.extra_power),
            (ExtraKind::Skateboard, &mut self.extra_skateboards),
            (ExtraKind::Kick, &mut self.extra_kick),
            (ExtraKind::Glove, &mut self.extra_gloves),
        ] {
            for _ in 0..*n {
                self.drops.push((kind, x, y));
            }
            *n = 0;
        }
    }

    pub fn is_diseased(&self) -> bool {
        self.disease.is_some()
    }

    /// `Disease::start()`: aplica el efecto.
    fn start_disease(&mut self, kind: DiseaseKind) {
        match kind {
            DiseaseKind::Fast => self.speed += 600,
            DiseaseKind::PutBomb => self.bomb_always = true,
            DiseaseKind::Stoned => self.reverse = !self.reverse,
            DiseaseKind::Frozen => self.active = false,
        }
    }

    /// `Disease::stop()`: revierte el efecto.
    fn stop_disease(&mut self, kind: DiseaseKind) {
        match kind {
            DiseaseKind::Fast => self.speed -= 600,
            DiseaseKind::PutBomb => self.bomb_always = false,
            DiseaseKind::Stoned => self.reverse = !self.reverse,
            DiseaseKind::Frozen => self.active = true,
        }
    }

    /// `Bomber::infect`: la nueva enfermedad empieza antes de curar la anterior, como en el
    /// original (importa para `Stoned`, cuyo efecto es un interruptor).
    pub fn infect(&mut self, kind: DiseaseKind) {
        self.start_disease(kind);
        let new = Disease {
            kind,
            countdown: kind.duration(),
        };
        if let Some(old) = self.disease.replace(new) {
            self.stop_disease(old.kind);
        }
    }

    /// `Bomber::loose_disease`: se cura y suelta el extra de su enfermedad.
    pub fn loose_disease(&mut self) {
        if let Some(d) = self.disease.take() {
            self.stop_disease(d.kind);
            self.drops
                .push((d.kind.extra(), self.get_x(), self.get_y()));
        }
    }

    /// `Controller_Keyboard::is_*`: con `Stoned` izquierda/derecha y arriba/abajo se
    /// intercambian; congelado no responde nada; `PutBomb` ignora la tecla de bomba.
    fn filter_input(&self, i: Input) -> Input {
        let mut o = if self.reverse {
            Input {
                left: i.right,
                right: i.left,
                up: i.down,
                down: i.up,
                bomb: i.bomb,
            }
        } else {
            i
        };
        if !self.active {
            o = Input::default();
        }
        // `bomb_mode == ALWAYS` devuelve `true` incluso congelado.
        o.bomb = self.bomb_always || (o.bomb && self.active);
        o
    }

    /// `Bomber::gain_extra_*`; los máximos son los de `Config::max_*` del original.
    pub fn gain_extra_bomb(&mut self) {
        if MAX_BOMBS > self.bombs {
            self.bombs += 1;
            self.extra_bombs += 1;
            self.cur_bombs += 1;
        }
    }

    pub fn gain_extra_power(&mut self) {
        if MAX_POWER > self.power {
            self.power += 1;
            self.extra_power += 1;
        }
    }

    pub fn gain_extra_skateboard(&mut self) {
        if MAX_SKATEBOARDS > self.skateboards {
            self.skateboards += 1;
            self.extra_skateboards += 1;
            self.speed += 40;
        }
    }

    pub fn gain_extra_kick(&mut self) {
        self.can_kick = true;
        self.extra_kick += 1;
    }

    pub fn gain_extra_glove(&mut self) {
        self.gloves += 1;
        self.extra_gloves += 1;
    }

    /// `Bomber::die()`. Devuelve `true` si acaba de morir.
    /// TODO(cadáver): el original crea un `Bomber_Corpse` y suelta los extras.
    pub fn die(&mut self) -> bool {
        if self.dead || self.is_falling() {
            return false;
        }
        self.dead = true;
        self.deaths += 1;
        if !self.fallen_down {
            self.loose_all_extras();
        }
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

    /// `GameObject::is_falling()`: cayendo y aún visible.
    pub fn is_falling(&self) -> bool {
        self.falling && !self.fallen_down
    }

    /// `GameObject::fall()`: pisó una casilla sin suelo.
    pub fn fall(&mut self) {
        if !self.falling {
            self.falling = true;
            self.fall_countdown = 1.0;
        }
    }

    /// `GameObject::continue_falling()`: se frena y se desvanece durante 1 s.
    fn continue_falling(&mut self, dt: f32) {
        self.speed = (self.fall_countdown * 60.0) as i32;
        self.fall_countdown -= dt;
        if self.fall_countdown < 0.0 {
            self.fallen_down = true;
            self.fall_countdown = 0.0;
        }
    }

    /// `Bomber::act` completo salvo enfermedades. `dt` en segundos.
    pub fn act(&mut self, dt: f32, input: Input, env: &Env) -> Action {
        let mut action = Action::default();
        if self.dead {
            return action;
        }
        // Disease::act: la enfermedad se cura sola al agotarse el contador.
        if let Some(mut d) = self.disease.take() {
            d.countdown -= dt;
            if d.countdown < 0.0 {
                self.stop_disease(d.kind);
            } else {
                self.disease = Some(d);
            }
        }
        let input = self.filter_input(input);
        if self.falling {
            self.continue_falling(dt);
        }
        if self.fallen_down {
            self.die();
            return action;
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

        let moved = dir.is_some();
        match dir {
            Some(d) => {
                self.dir = d;
                // `speed/90` es división entera en el original.
                self.anim_count += dt * 20.0 * (self.speed / 90) as f32;
                let mover = Mover {
                    can_pass_bomber: true,
                    can_kick: self.can_kick,
                    self_bomb: None,
                };
                let mut pos = Pos {
                    x: &mut self.x,
                    y: &mut self.y,
                    remainder: &mut self.remainder,
                };
                action.kick = advance(&mut pos, self.speed, dt, d, &mover, env).kick;
            }
            None => self.anim_count = 0.0,
        }
        if self.anim_count >= 9.0 {
            self.anim_count = 1.0;
        }

        if input.bomb {
            let (tx, ty) = self.tile();
            match env.q.bomb_at(tx, ty) {
                // Con guantes y sin moverse, la bomba bajo los pies sale lanzada.
                Some(b) => {
                    if self.gloves > 0 && !moved {
                        action.throw_bomb = Some((b, self.dir));
                        self.anim_count = 2.0;
                    }
                }
                None => action.put_bomb = self.cur_bombs > 0 && !self.falling,
            }
        }
        action
    }
}

/// Lo que el bomber pide al mundo en un tick; el `World` posee las bombas y lo ejecuta.
#[derive(Default, Debug)]
pub struct Action {
    pub put_bomb: bool,
    /// Bomba (índice del objeto) a lanzar con el guante, y hacia dónde.
    pub throw_bomb: Option<(usize, Dir)>,
    /// Bomba (índice del objeto) a patear, y hacia dónde.
    pub kick: Option<(usize, Dir)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn arena() -> Map {
        Map::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/maps/Arena.map"
        ))
        .unwrap()
    }

    struct Empty;
    impl crate::movement::Query for Empty {
        fn bomb_at(&self, _: i32, _: i32) -> Option<usize> {
            None
        }
        fn has_bomber(&self, _: i32, _: i32) -> bool {
            false
        }
        fn bomb_is_still(&self, _: usize) -> bool {
            true
        }
    }

    fn env(map: &Map) -> Env<'_> {
        Env { map, q: &Empty }
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
            b.act(1.0 / 60.0, DOWN, &env(&map));
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
            b.act(1.0 / 60.0, RIGHT, &env(&map));
        }
        assert_eq!(b.get_x(), 15 * 40);
    }

    #[test]
    fn spawn_direction_follows_original() {
        assert_eq!(Bomber::new(40, 40).dir, Dir::Right);
        assert_eq!(Bomber::new(15 * 40, 40).dir, Dir::Left);
    }
}
