use crate::bomb::{Bomb, Explosion, Object};
use crate::bomber::{Bomber, Input};
use crate::map::{Map, TILE_PIXELS};

/// Frecuencia fija de la simulación. El render interpola entre ticks.
pub const TICK_HZ: u32 = 60;

/// Duración de un tick en segundos (el original usa `Timer::time_elapsed()` variable).
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;

pub struct World {
    pub map: Map,
    pub bombers: Vec<Bomber>,
    /// Bombas y explosiones en orden de creación (`ClanBomberApplication::objects`).
    pub objects: Vec<Object>,
    pub tick: u64,
}

impl World {
    pub fn new(map: Map) -> Self {
        World {
            map,
            bombers: Vec::new(),
            objects: Vec::new(),
            tick: 0,
        }
    }

    /// Crea un bomber por cada una de las primeras `n` posiciones de salida del mapa.
    pub fn spawn_bombers(&mut self, n: usize) {
        let t = TILE_PIXELS as i32;
        self.bombers = self
            .map
            .spawns
            .iter()
            .take(n)
            .map(|&(_, x, y)| Bomber::new(x as i32 * t, y as i32 * t))
            .collect();
    }

    pub fn bomb_at(&self, tx: i32, ty: i32) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(o, Object::Bomb(b) if b.tile() == (tx, ty)))
    }

    pub fn bombs(&self) -> impl Iterator<Item = &Bomb> {
        self.objects.iter().filter_map(|o| match o {
            Object::Bomb(b) => Some(b),
            _ => None,
        })
    }

    pub fn explosions(&self) -> impl Iterator<Item = &Explosion> {
        self.objects.iter().filter_map(|o| match o {
            Object::Explosion(e) => Some(e),
            _ => None,
        })
    }

    /// Avanza exactamente un tick. `inputs[i]` es la entrada del bomber `i`.
    /// Mismo orden que `ClanBomberApplication`: borrar objetos marcados, luego mapa,
    /// objetos y bombers (`delete_some` + `act_all`).
    pub fn step(&mut self, inputs: &[Input]) {
        self.delete_some();
        self.map.act(TICK_DT);

        // Los objetos creados durante este tick (explosiones) actúan a partir del siguiente.
        for i in 0..self.objects.len() {
            match &self.objects[i] {
                Object::Bomb(_) => self.act_bomb(i),
                Object::Explosion(_) => self.act_explosion(i),
            }
        }

        for i in 0..self.bombers.len() {
            let input = inputs.get(i).copied().unwrap_or_default();
            let wants_bomb = {
                let objects = &self.objects;
                let bomb_at = |tx: i32, ty: i32| {
                    objects
                        .iter()
                        .any(|o| matches!(o, Object::Bomb(b) if b.tile() == (tx, ty)))
                };
                self.bombers[i].act(TICK_DT, input, &self.map, &bomb_at)
            };
            if wants_bomb {
                self.put_bomb(i);
            }
        }
        self.tick += 1;
    }

    /// `Map::add_bomb` + `Bomb::Bomb`: la bomba descuenta una de las disponibles del bomber.
    fn put_bomb(&mut self, owner: usize) {
        let (tx, ty) = self.bombers[owner].tile();
        if self.bomb_at(tx, ty) {
            return;
        }
        let power = self.bombers[owner].power;
        self.bombers[owner].cur_bombs -= 1;
        self.objects
            .push(Object::Bomb(Bomb::new(tx, ty, power, owner)));
    }

    /// `ClanBomberApplication::delete_some`; el destructor de `Bomb` devuelve la bomba al bomber.
    fn delete_some(&mut self) {
        let mut returned = Vec::new();
        self.objects.retain(|o| {
            if o.delete_me() {
                if let Object::Bomb(b) = o {
                    returned.push(b.owner);
                }
                false
            } else {
                true
            }
        });
        for owner in returned {
            self.bombers[owner].cur_bombs += 1;
        }
    }

    /// `Bomb::act` (sin patada ni vuelo: TODO(extras)).
    fn act_bomb(&mut self, i: usize) {
        let Object::Bomb(b) = &mut self.objects[i] else {
            return;
        };
        b.cur_scale += TICK_DT * 10.0;
        b.countdown -= TICK_DT;
        if b.countdown <= 0.0 {
            self.explode(i);
        }
    }

    /// `Bomb::explode`: crea la explosión en la casilla de la bomba y se marca para borrar.
    fn explode(&mut self, i: usize) {
        let Object::Bomb(b) = &mut self.objects[i] else {
            return;
        };
        if b.exploding {
            return;
        }
        b.exploding = true;
        b.delete_me = true;
        // `snap()`: la bomba se alinea con la casilla antes de explotar.
        let (x, y) = (((b.x + 20) / 40) * 40, ((b.y + 20) / 40) * 40);
        let (power, owner) = (b.power, b.owner);
        let e = Explosion::new(&mut self.map, x, y, power, owner);
        self.objects.push(Object::Explosion(e));
    }

    /// `Explosion::act`: detona bombas vecinas, mata bombers y envejece.
    fn act_explosion(&mut self, i: usize) {
        let Object::Explosion(e) = &self.objects[i] else {
            return;
        };
        let e = e.clone();

        // detonate_other_bombs: la primera bomba de cada brazo (el brazo izquierdo
        // empieza en la casilla central, igual que el original).
        let arms: [(i32, i32, i32, i32); 4] = [
            (-1, 0, 0, e.left),
            (1, 0, 1, e.right),
            (0, -1, 1, e.up),
            (0, 1, 1, e.down),
        ];
        let mut to_explode = Vec::new();
        for (dx, dy, start, len) in arms {
            for k in start..=len {
                let (tx, ty) = (e.tx + dx * k, e.ty + dy * k);
                if let Some(j) = self
                    .objects
                    .iter()
                    .position(|o| matches!(o, Object::Bomb(b) if b.tile() == (tx, ty)))
                {
                    to_explode.push(j);
                    break;
                }
            }
        }
        for j in to_explode {
            let Object::Bomb(b) = &mut self.objects[j] else {
                continue;
            };
            if b.explode_delayed() {
                self.explode(j);
            }
        }

        // kill_bombers
        for k in 0..self.bombers.len() {
            let (bx, by) = self.bombers[k].tile();
            if e.covers(bx, by) && self.bombers[k].die() && k != e.owner {
                self.bombers[e.owner].kills += 1;
            }
        }

        let Object::Explosion(e) = &mut self.objects[i] else {
            return;
        };
        e.period -= TICK_DT;
        if e.period < 0.0 {
            e.delete_me = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;

    fn open_world() -> World {
        let src = "x\n2\n".to_owned() + &" ".repeat(17 * 13);
        let mut w = World::new(Map::parse(&src).unwrap());
        w.bombers = vec![Bomber::new(5 * 40, 5 * 40), Bomber::new(10 * 40, 5 * 40)];
        w
    }

    const BOMB: Input = Input {
        bomb: true,
        left: false,
        right: false,
        up: false,
        down: false,
    };

    #[test]
    fn step_is_deterministic() {
        let mut a = open_world();
        for _ in 0..100 {
            a.step(&[]);
        }
        assert_eq!(a.tick, 100);
    }

    #[test]
    fn bomb_explodes_after_countdown_and_returns_to_owner() {
        let mut w = open_world();
        w.step(&[BOMB]);
        assert_eq!(w.bombs().count(), 1);
        assert_eq!(w.bombers[0].cur_bombs, 0);
        // Con una sola bomba disponible no se puede poner otra.
        w.step(&[BOMB]);
        assert_eq!(w.bombs().count(), 1);

        for _ in 0..(2.5 * 60.0) as usize + 2 {
            w.step(&[]);
        }
        assert_eq!(w.explosions().count(), 1);
        // La bomba se borra al inicio del tick siguiente y devuelve el cupo.
        w.step(&[]);
        assert_eq!(w.bombs().count(), 0);
        assert_eq!(w.bombers[0].cur_bombs, 1);
    }

    #[test]
    fn explosion_kills_bomber_in_line_and_lasts_half_a_second() {
        let mut w = open_world();
        // Bomber 1 queda dentro del alcance (potencia 1) de la bomba del bomber 0.
        w.bombers[1] = Bomber::new(6 * 40, 5 * 40);
        w.step(&[BOMB]);
        for _ in 0..(2.5 * 60.0) as usize + 4 {
            w.step(&[]);
        }
        assert!(w.bombers[1].dead);
        assert!(
            w.bombers[0].dead,
            "el que pone la bomba también muere si no se aparta"
        );
        for _ in 0..40 {
            w.step(&[]);
        }
        assert_eq!(w.explosions().count(), 0);
    }

    #[test]
    fn box_crumbles_then_becomes_ground() {
        use crate::map::Tile;
        let mut rows = vec![" ".repeat(17); 13];
        rows[5].replace_range(6..7, "+");
        let src = "x\n1\n".to_owned() + &rows.concat();
        let mut w = World::new(Map::parse(&src).unwrap());
        w.bombers = vec![Bomber::new(5 * 40, 5 * 40)];
        w.step(&[BOMB]);
        for _ in 0..(2.5 * 60.0) as usize + 3 {
            w.step(&[]);
        }
        assert!(w.map.is_crumbling(6, 5));
        assert_eq!(
            w.map.get(6, 5),
            Tile::Box,
            "sigue bloqueando mientras se desmorona"
        );
        for _ in 0..40 {
            w.step(&[]);
        }
        assert_eq!(w.map.get(6, 5), Tile::Ground);
    }

    #[test]
    fn chain_reaction_uses_bomb_delay() {
        let mut w = open_world();
        // A está a punto de explotar; B (a 2 casillas) tiene la mecha completa.
        let mut a = Bomb::new(5, 5, 2, 0);
        a.countdown = 0.02;
        w.objects.push(Object::Bomb(a));
        w.objects.push(Object::Bomb(Bomb::new(7, 5, 1, 1)));
        for _ in 0..4 {
            w.step(&[]);
        }
        // Tras la explosión de A, la mecha de B se acorta a 0.1 s.
        let b = w.bombs().find(|b| b.tile() == (7, 5)).unwrap();
        assert!(b.countdown <= 0.1 + 1e-4, "countdown={}", b.countdown);
        for _ in 0..10 {
            w.step(&[]);
        }
        assert_eq!(w.explosions().filter(|e| e.tx == 7).count(), 1);
    }
}
