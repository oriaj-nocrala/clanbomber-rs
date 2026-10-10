use crate::bomb::{Bomb, Explosion, Object};
use crate::bomber::{Bomber, Input};
use crate::extra::{roll_extra, Extra, ExtraConfig, ExtraKind};
use crate::map::{Map, TILE_PIXELS};
use crate::rng::Rng;

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
    pub rng: Rng,
    pub extra_config: ExtraConfig,
}

impl World {
    pub fn new(map: Map) -> Self {
        Self::with_seed(map, 0)
    }

    /// La semilla fija todo lo aleatorio (extras que sueltan las cajas).
    pub fn with_seed(map: Map, seed: u64) -> Self {
        World {
            map,
            bombers: Vec::new(),
            objects: Vec::new(),
            tick: 0,
            rng: Rng::new(seed),
            extra_config: ExtraConfig::default(),
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

    pub fn has_extra(&self, tx: i32, ty: i32) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(o, Object::Extra(e) if e.tile() == (tx, ty)))
    }

    pub fn extras(&self) -> impl Iterator<Item = &Extra> {
        self.objects.iter().filter_map(|o| match o {
            Object::Extra(e) => Some(e),
            _ => None,
        })
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
        // Una caja que acaba de desmoronarse puede soltar un extra (MapTile_Box::act).
        for (x, y) in self.map.act(TICK_DT) {
            self.spawn_extra(x as i32 * 40, y as i32 * 40);
        }

        // Los objetos creados durante este tick (explosiones) actúan a partir del siguiente.
        for i in 0..self.objects.len() {
            match &self.objects[i] {
                Object::Bomb(_) => self.act_bomb(i),
                Object::Explosion(_) => self.act_explosion(i),
                Object::Extra(_) => self.act_extra(i),
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

    /// `MapTile::spawn_extra()`.
    fn spawn_extra(&mut self, x: i32, y: i32) {
        if let Some(kind) = roll_extra(&mut self.rng, &self.extra_config) {
            self.objects.push(Object::Extra(Extra::new(x, y, kind)));
        }
    }

    /// `Extra::act`: parpadeo de destrucción y efecto sobre los bombers que lo pisan.
    fn act_extra(&mut self, i: usize) {
        let Object::Extra(e) = &mut self.objects[i] else {
            return;
        };
        if e.destroyed {
            e.destroy_countdown -= TICK_DT;
            if e.destroy_countdown <= 0.0 {
                e.delete_me = true;
                return;
            }
        }
        let (tile, kind) = (e.tile(), e.kind);
        let mut taken = false;
        for b in self
            .bombers
            .iter_mut()
            .filter(|b| !b.dead && b.tile() == tile)
        {
            // TODO(enfermedades): `loose_disease()` y las enfermedades de Joint/Viagra/Koks.
            match kind {
                ExtraKind::Bomb => b.gain_extra_bomb(),
                ExtraKind::Power => b.gain_extra_power(),
                ExtraKind::Skateboard => b.gain_extra_skateboard(),
                ExtraKind::Kick => b.gain_extra_kick(),
                ExtraKind::Glove => b.gain_extra_glove(),
                ExtraKind::Joint | ExtraKind::Viagra | ExtraKind::Koks => {}
            }
            taken = true;
        }
        if taken {
            if let Object::Extra(e) = &mut self.objects[i] {
                e.delete_me = true;
            }
        }
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
        let objects = &self.objects;
        let has_extra = |tx: i32, ty: i32| {
            objects
                .iter()
                .any(|o| matches!(o, Object::Extra(e) if e.tile() == (tx, ty)))
        };
        let e = Explosion::new(&mut self.map, &has_extra, x, y, power, owner);
        // MapTile::destroy(): las casillas alcanzadas destruyen los extras que hay en ellas.
        for &t in &e.destroyed_tiles {
            for o in self.objects.iter_mut() {
                if let Object::Extra(x) = o {
                    if x.tile() == t {
                        x.destroy();
                    }
                }
            }
        }
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

    #[test]
    fn walking_over_extras_applies_them_once() {
        let mut w = open_world();
        let (x, y) = (5 * 40, 5 * 40);
        for k in [
            ExtraKind::Bomb,
            ExtraKind::Power,
            ExtraKind::Skateboard,
            ExtraKind::Kick,
        ] {
            w.objects.push(Object::Extra(Extra::new(x, y, k)));
        }
        w.step(&[]);
        let b = &w.bombers[0];
        assert_eq!(
            (b.bombs, b.cur_bombs, b.power, b.speed, b.can_kick),
            (2, 2, 2, 130, true)
        );
        w.step(&[]);
        w.step(&[]);
        assert_eq!(w.extras().count(), 0, "se consumen");
        assert_eq!(w.bombers[0].power, 2);
    }

    #[test]
    fn explosion_destroys_extras_but_not_diseases() {
        let mut w = open_world();
        w.bombers[0] = Bomber::new(1 * 40, 1 * 40);
        w.objects
            .push(Object::Extra(Extra::new(5 * 40, 8 * 40, ExtraKind::Bomb)));
        w.objects
            .push(Object::Extra(Extra::new(5 * 40, 8 * 40, ExtraKind::Joint)));
        w.objects.push(Object::Bomb(Bomb::new(5, 7, 2, 0)));
        // 2.5 s de mecha + 0.5 s de parpadeo del extra destruido.
        for _ in 0..(3.0 * 60.0) as usize + 6 {
            w.step(&[]);
        }
        let left: Vec<_> = w.extras().map(|e| e.kind).collect();
        assert_eq!(left, vec![ExtraKind::Joint]);
    }

    #[test]
    fn extra_stops_the_fire() {
        let mut w = open_world();
        w.bombers[0] = Bomber::new(1 * 40, 1 * 40);
        // Extra en (7,5): una bomba potencia 4 en (5,5) llega solo hasta (6,5).
        w.objects
            .push(Object::Extra(Extra::new(7 * 40, 5 * 40, ExtraKind::Joint)));
        w.objects.push(Object::Bomb(Bomb::new(5, 5, 4, 0)));
        for _ in 0..(2.5 * 60.0) as usize + 3 {
            w.step(&[]);
        }
        let e = w.explosions().next().unwrap();
        assert_eq!((e.left, e.right), (4, 1));
    }

    #[test]
    fn crumbled_boxes_drop_extras_deterministically() {
        let build = || {
            let mut rows = vec![" ".repeat(17); 13];
            for x in 0..17 {
                rows[2].replace_range(x..x + 1, "+");
            }
            let src = "x\n1\n".to_owned() + &rows.concat();
            let mut w = World::with_seed(Map::parse(&src).unwrap(), 11);
            for x in 0..17 {
                w.map.destroy(x, 2);
            }
            for _ in 0..40 {
                w.step(&[]);
            }
            w.extras().map(|e| (e.x, e.kind)).collect::<Vec<_>>()
        };
        let (a, b) = (build(), build());
        assert_eq!(a, b);
        assert!(!a.is_empty() && a.len() < 17);
    }
}
