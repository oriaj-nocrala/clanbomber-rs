use crate::bomb::{Bomb, Explosion, Object};
use crate::bomber::{Action, Bomber, Input};
use crate::extra::{roll_extra, Extra, ExtraConfig, ExtraKind};
use crate::map::{Map, TILE_PIXELS};
use crate::movement::{advance, Env, Mover, Pos};
use crate::rng::Rng;

/// Frecuencia fija de la simulación. El render interpola entre ticks.
pub const TICK_HZ: u32 = 60;

/// Duración de un tick en segundos (el original usa `Timer::time_elapsed()` variable).
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;

pub struct World {
    pub map: Map,
    pub bombers: Vec<Bomber>,
    /// Bombas, explosiones y extras en orden de creación (`ClanBomberApplication::objects`).
    pub objects: Vec<Object>,
    pub tick: u64,
    pub rng: Rng,
    pub extra_config: ExtraConfig,
}

/// Vista de solo lectura para las consultas de colisión (`tile->bomb`, `has_bomber`).
struct View<'a> {
    objects: &'a [Object],
    bombers: &'a [Bomber],
}

impl View<'_> {
    /// Última bomba apoyada en la casilla (`maptilize_objects` deja la última en `tile->bomb`).
    fn bomb_at(&self, tx: i32, ty: i32, except: Option<usize>) -> Option<usize> {
        self.objects
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, o)| match o {
                Object::Bomb(b)
                    if Some(i) != except && b.rests_on_tile() && b.tile() == (tx, ty) =>
                {
                    Some(i)
                }
                _ => None,
            })
    }

    fn has_bomber(&self, tx: i32, ty: i32) -> bool {
        self.bombers.iter().any(|b| !b.dead && b.tile() == (tx, ty))
    }

    fn bomb_is_still(&self, i: usize) -> bool {
        matches!(&self.objects[i], Object::Bomb(b) if b.cur_dir.is_none())
    }
}

impl crate::movement::Query for View<'_> {
    fn bomb_at(&self, tx: i32, ty: i32) -> Option<usize> {
        View::bomb_at(self, tx, ty, None)
    }

    fn has_bomber(&self, tx: i32, ty: i32) -> bool {
        View::has_bomber(self, tx, ty)
    }

    fn bomb_is_still(&self, i: usize) -> bool {
        View::bomb_is_still(self, i)
    }
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
        View {
            objects: &self.objects,
            bombers: &self.bombers,
        }
        .bomb_at(tx, ty, None)
        .is_some()
    }

    pub fn has_extra(&self, tx: i32, ty: i32) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(o, Object::Extra(e) if e.rests_on_tile() && e.tile() == (tx, ty)))
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
        self.fall_into_holes();

        // Los objetos creados durante este tick (explosiones) actúan a partir del siguiente.
        for i in 0..self.objects.len() {
            match &self.objects[i] {
                Object::Bomb(_) => self.act_bomb(i),
                Object::Explosion(_) => self.act_explosion(i),
                Object::Extra(_) => self.act_extra(i),
            }
        }

        for i in 0..self.bombers.len() {
            self.infect_others(i);
            let input = inputs.get(i).copied().unwrap_or_default();
            // Se actúa sobre una copia para poder consultar al resto del mundo.
            let mut me = self.bombers[i].clone();
            let action = {
                let view = View {
                    objects: &self.objects,
                    bombers: &self.bombers,
                };
                me.act(TICK_DT, input, &Self::env(&self.map, &view))
            };
            self.bombers[i] = me;
            self.apply(i, action);
        }
        self.flush_drops();
        self.tick += 1;
    }

    fn env<'a>(map: &'a Map, view: &'a View<'a>) -> Env<'a> {
        Env { map, q: view }
    }

    /// `Bomber::infect_others`: un bomber enfermo contagia a los sanos que comparten casilla.
    fn infect_others(&mut self, i: usize) {
        let Some(d) = self.bombers[i].disease else {
            return;
        };
        let tile = self.bombers[i].tile();
        if self.bombers[i].dead {
            return;
        }
        for b in self.bombers.iter_mut() {
            if !b.dead && !b.is_diseased() && b.tile() == tile {
                b.infect(d.kind);
            }
        }
    }

    /// `Map::get_passable`: casilla transitable al azar (hasta 1000 intentos).
    fn get_passable(&mut self) -> Option<(usize, usize)> {
        for _ in 0..1000 {
            let x = self.rng.below(crate::map::MAP_WIDTH as u32) as usize;
            let y = self.rng.below(crate::map::MAP_HEIGHT as u32) as usize;
            if self.map.is_passable(x as i32, y as i32) {
                return Some((x, y));
            }
        }
        None
    }

    /// Coloca los extras que los bombers han soltado (al curarse o morir) y los lanza a una
    /// casilla libre al azar.
    fn flush_drops(&mut self) {
        for k in 0..self.bombers.len() {
            for (kind, x, y) in std::mem::take(&mut self.bombers[k].drops) {
                let mut e = Extra::new(x, y, kind);
                if let Some(t) = self.get_passable() {
                    e.fly_to(t);
                }
                self.objects.push(Object::Extra(e));
            }
        }
    }

    /// `Extra::destroy`: los destruibles parpadean y se borran; los demás salen volando.
    fn destroy_extra(&mut self, i: usize) {
        let Object::Extra(e) = &mut self.objects[i] else {
            return;
        };
        if e.destroyed || e.flight.active {
            return;
        }
        if e.kind.destroyable() {
            e.destroy();
        } else if let Some(t) = self.get_passable() {
            if let Object::Extra(e) = &mut self.objects[i] {
                e.fly_to(t);
            }
        }
    }

    /// Índices de los extras apoyados (no en vuelo) en casillas que cumplen `pred`.
    fn extras_where(&self, pred: impl Fn((i32, i32)) -> bool) -> Vec<usize> {
        self.objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                Object::Extra(e) if e.rests_on_tile() && pred(e.tile()) => Some(i),
                _ => None,
            })
            .collect()
    }

    /// Ejecuta lo que pidió el bomber `owner`: patada, lanzamiento y poner bomba.
    fn apply(&mut self, owner: usize, a: Action) {
        if let Some((i, dir)) = a.kick {
            if let Object::Bomb(b) = &mut self.objects[i] {
                b.kick(dir);
            }
        }
        if let Some((i, dir)) = a.throw_bomb {
            if let Object::Bomb(b) = &mut self.objects[i] {
                b.throw(dir, 4);
            }
        }
        if a.put_bomb {
            self.put_bomb(owner);
        }
    }

    /// `MapTile_None::act`: lo que pisa una casilla sin suelo cae.
    fn fall_into_holes(&mut self) {
        for b in self.bombers.iter_mut().filter(|b| !b.dead) {
            let (tx, ty) = b.tile();
            if self.map.is_hole(tx, ty) {
                b.fall();
            }
        }
        for o in self.objects.iter_mut() {
            match o {
                Object::Bomb(b) if !b.flight.active && self.map.is_hole(b.tile().0, b.tile().1) => {
                    b.fall()
                }
                Object::Extra(e) if self.map.is_hole(e.tile().0, e.tile().1) => e.fall(),
                _ => {}
            }
        }
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

    /// `Extra::act`: vuelo, caída, parpadeo de destrucción y efecto sobre quien lo pisa.
    fn act_extra(&mut self, i: usize) {
        let Object::Extra(e) = &mut self.objects[i] else {
            return;
        };
        // GameObject::act: si vuela solo avanza el vuelo (el contador de destrucción corre igual).
        if e.flight.active {
            e.continue_flying(TICK_DT);
        } else if e.falling {
            // En el original un extra que cae nunca se borra pero queda invisible; aquí se borra.
            e.fall_countdown -= TICK_DT;
            if e.fall_countdown < 0.0 {
                e.delete_me = true;
            }
            return;
        }
        if e.destroyed {
            e.destroy_countdown -= TICK_DT;
            if e.destroy_countdown <= 0.0 {
                e.delete_me = true;
                return;
            }
        }
        if e.flight.active {
            return;
        }
        let (tile, kind) = (e.tile(), e.kind);
        let mut taken = false;
        for b in self
            .bombers
            .iter_mut()
            .filter(|b| !b.dead && b.tile() == tile)
        {
            // Todo extra cura primero la enfermedad que hubiera (y suelta su extra).
            b.loose_disease();
            match kind {
                ExtraKind::Bomb => b.gain_extra_bomb(),
                ExtraKind::Power => b.gain_extra_power(),
                ExtraKind::Skateboard => b.gain_extra_skateboard(),
                ExtraKind::Kick => b.gain_extra_kick(),
                ExtraKind::Glove => b.gain_extra_glove(),
                ExtraKind::Joint | ExtraKind::Viagra | ExtraKind::Koks => {
                    b.infect(kind.disease().expect("extra de enfermedad"))
                }
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

    /// `Bomb::act`: vuelo, aterrizaje, deslizamiento, mecha y explosión.
    fn act_bomb(&mut self, i: usize) {
        let Object::Bomb(orig) = &self.objects[i] else {
            return;
        };
        let mut b = orig.clone();

        // GameObject::act: volando solo avanza el vuelo; si no, cae si toca.
        if b.flight.active {
            b.continue_flying(&self.map, TICK_DT);
        } else if b.falling {
            b.continue_falling(TICK_DT);
        }

        let mut explode = false;
        if b.fallen_down {
            b.delete_me = true;
        } else if !b.flight.active {
            let view = View {
                objects: &self.objects,
                bombers: &self.bombers,
            };
            if b.flight.progress == 1.0 {
                // Acaba de aterrizar: si cae sobre otra bomba, rebota una casilla más.
                b.flight.progress = 0.0;
                let (tx, ty) = b.tile();
                match (view.bomb_at(tx, ty, Some(i)), b.cur_dir) {
                    (Some(_), Some(dir)) => b.throw(dir, 1),
                    _ => b.cur_dir = None,
                }
            } else {
                if let Some(dir) = b.cur_dir {
                    let mover = Mover {
                        can_pass_bomber: false,
                        can_kick: false,
                        self_bomb: Some(i),
                    };
                    let env = Self::env(&self.map, &view);
                    let mut pos = Pos {
                        x: &mut b.x,
                        y: &mut b.y,
                        remainder: &mut b.remainder,
                    };
                    if !advance(&mut pos, b.speed, TICK_DT, dir, &mover, &env).moved {
                        b.stop();
                    }
                }
                b.cur_scale += TICK_DT * 10.0;
                b.countdown -= TICK_DT;
                explode = b.countdown <= 0.0;
            }
        }

        self.objects[i] = Object::Bomb(b);
        if explode {
            self.explode(i);
        }
    }

    /// `Bomb::explode`: crea la explosión en la casilla de la bomba y se marca para borrar.
    fn explode(&mut self, i: usize) {
        let Object::Bomb(b) = &mut self.objects[i] else {
            return;
        };
        if b.exploding || b.falling {
            return;
        }
        b.exploding = true;
        b.delete_me = true;
        // `snap()`: la bomba se alinea con la casilla antes de explotar.
        let (x, y) = (((b.get_x() + 20) / 40) * 40, ((b.get_y() + 20) / 40) * 40);
        let (power, owner) = (b.power, b.owner);
        let objects = &self.objects;
        let has_extra = |tx: i32, ty: i32| {
            objects
                .iter()
                .any(|o| matches!(o, Object::Extra(e) if e.rests_on_tile() && e.tile() == (tx, ty)))
        };
        let e = Explosion::new(&mut self.map, &has_extra, x, y, power, owner);
        // MapTile::destroy(): las casillas alcanzadas destruyen los extras que hay en ellas.
        for i in self.extras_where(|t| e.destroyed_tiles.contains(&t)) {
            self.destroy_extra(i);
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
        {
            let view = View {
                objects: &self.objects,
                bombers: &self.bombers,
            };
            for (dx, dy, start, len) in arms {
                for k in start..=len {
                    if let Some(j) = view.bomb_at(e.tx + dx * k, e.ty + dy * k, None) {
                        to_explode.push(j);
                        break;
                    }
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

        // destroy_extras: los que están en la cruz y no vuelan.
        for j in self.extras_where(|(bx, by)| e.covers(bx, by)) {
            self.destroy_extra(j);
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
    use crate::disease::DiseaseKind;
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

    fn world_with(cells: &[(usize, usize, char)]) -> World {
        let mut rows: Vec<Vec<char>> = vec![vec![' '; 17]; 13];
        for &(x, y, c) in cells {
            rows[y][x] = c;
        }
        let body: String = rows.iter().flatten().collect();
        let mut w = World::new(Map::parse(&("x\n2\n".to_owned() + &body)).unwrap());
        w.bombers = vec![Bomber::new(5 * 40, 5 * 40), Bomber::new(10 * 40, 5 * 40)];
        w
    }

    const RIGHT: Input = Input {
        right: true,
        left: false,
        up: false,
        down: false,
        bomb: false,
    };

    #[test]
    fn kicked_bomb_slides_until_it_meets_a_bomber() {
        let mut w = world_with(&[]);
        w.bombers[0] = Bomber::new(3 * 40, 5 * 40);
        w.bombers[0].can_kick = true;
        let mut b = Bomb::new(4, 5, 1, 0);
        b.countdown = 100.0;
        w.objects.push(Object::Bomb(b));
        for _ in 0..240 {
            w.step(&[RIGHT]);
        }
        // Se suelta la tecla: la bomba, ya frenada, queda quieta.
        for _ in 0..5 {
            w.step(&[]);
        }
        let b = w.bombs().next().unwrap();
        // Bomber 1 está en la casilla 10: la bomba se detiene pegada a ella, alineada.
        assert_eq!((b.get_x(), b.cur_dir), (360, None));
    }

    #[test]
    fn bomb_is_solid_without_kick() {
        let mut w = world_with(&[]);
        w.bombers[0] = Bomber::new(3 * 40, 5 * 40);
        let mut b = Bomb::new(4, 5, 1, 0);
        b.countdown = 100.0;
        w.objects.push(Object::Bomb(b));
        for _ in 0..120 {
            w.step(&[RIGHT]);
        }
        assert_eq!(w.bombs().next().unwrap().get_x(), 160, "no se mueve");
        assert!(w.bombers[0].get_x() < 160);
    }

    #[test]
    fn glove_throws_the_bomb_four_tiles() {
        let mut w = world_with(&[]);
        w.bombers[0].gloves = 1;
        w.step(&[BOMB]);
        w.step(&[BOMB]);
        assert!(w.bombs().next().unwrap().flight.active);
        for _ in 0..62 {
            w.step(&[]);
        }
        let b = w.bombs().next().unwrap();
        assert!(!b.flight.active);
        assert_eq!((b.get_x(), b.get_y()), (9 * 40, 5 * 40));
        // Durante el vuelo la mecha no corre: sigue casi completa.
        assert!(b.countdown > 2.4, "countdown={}", b.countdown);
    }

    #[test]
    fn without_glove_the_bomb_stays() {
        let mut w = world_with(&[]);
        w.step(&[BOMB]);
        w.step(&[BOMB]);
        assert!(!w.bombs().next().unwrap().flight.active);
    }

    #[test]
    fn thrown_bomb_stops_at_a_wall() {
        let mut w = world_with(&[(7, 5, '*')]);
        w.bombers[0].gloves = 1;
        w.step(&[BOMB]);
        w.step(&[BOMB]);
        for _ in 0..70 {
            w.step(&[]);
        }
        let b = w.bombs().next().unwrap();
        assert!(!b.flight.active);
        assert!(b.get_x() + 39 < 7 * 40, "x={}", b.get_x());
    }

    #[test]
    fn thrown_bomb_hops_over_another_bomb() {
        let mut w = world_with(&[]);
        w.bombers[0].gloves = 1;
        let mut other = Bomb::new(9, 5, 1, 1);
        other.countdown = 100.0;
        w.objects.push(Object::Bomb(other));
        w.step(&[BOMB]);
        w.step(&[BOMB]);
        for _ in 0..140 {
            w.step(&[]);
        }
        // Aterriza en la casilla 9 (ocupada) y rebota una más hasta la 10.
        let xs: Vec<i32> = w.bombs().map(|b| b.get_x()).collect();
        assert!(xs.contains(&(9 * 40)) && xs.contains(&(10 * 40)), "{xs:?}");
    }

    #[test]
    fn walking_into_a_hole_kills_after_a_second() {
        let mut w = world_with(&[(7, 5, '-')]);
        w.bombers[0] = Bomber::new(6 * 40, 5 * 40);
        for _ in 0..30 {
            w.step(&[RIGHT]);
        }
        assert!(w.bombers[0].is_falling(), "cae al pisar el hueco");
        assert!(!w.bombers[0].dead);
        for _ in 0..120 {
            w.step(&[]);
        }
        assert!(w.bombers[0].dead);
    }

    #[test]
    fn bomb_in_a_hole_falls_and_is_removed() {
        let mut w = world_with(&[(7, 5, '-')]);
        let mut b = Bomb::new(7, 5, 1, 0);
        b.countdown = 100.0;
        w.objects.push(Object::Bomb(b));
        for _ in 0..90 {
            w.step(&[]);
        }
        assert_eq!(w.bombs().count(), 0);
    }

    fn put_extra(w: &mut World, kind: ExtraKind, tx: i32, ty: i32) {
        w.objects
            .push(Object::Extra(Extra::new(tx * 40, ty * 40, kind)));
    }

    #[test]
    fn koks_makes_you_fast_for_ten_seconds() {
        let mut w = world_with(&[]);
        put_extra(&mut w, ExtraKind::Koks, 5, 5);
        w.step(&[]);
        assert!(w.bombers[0].is_diseased());
        assert_eq!(w.bombers[0].speed, 690);
        for _ in 0..(10.0 * 60.0) as usize + 2 {
            w.step(&[]);
        }
        assert!(!w.bombers[0].is_diseased());
        assert_eq!(w.bombers[0].speed, 90);
        assert_eq!(w.extras().count(), 0, "curarse solo no suelta nada");
    }

    #[test]
    fn joint_reverses_the_controls_and_then_restores_them() {
        let mut w = world_with(&[]);
        put_extra(&mut w, ExtraKind::Joint, 5, 5);
        w.step(&[]);
        let x0 = w.bombers[0].x;
        for _ in 0..20 {
            w.step(&[RIGHT]);
        }
        assert!(w.bombers[0].x < x0, "derecha va a la izquierda");
        for _ in 0..(10.0 * 60.0) as usize {
            w.step(&[]);
        }
        let x1 = w.bombers[0].x;
        for _ in 0..20 {
            w.step(&[RIGHT]);
        }
        assert!(w.bombers[0].x > x1, "curado: derecha vuelve a ser derecha");
    }

    #[test]
    fn viagra_drops_bombs_without_pressing_anything() {
        let mut w = world_with(&[]);
        w.bombers[0].bombs = 3;
        w.bombers[0].cur_bombs = 3;
        put_extra(&mut w, ExtraKind::Viagra, 5, 5);
        w.step(&[]);
        for _ in 0..90 {
            w.step(&[RIGHT]);
        }
        assert!(w.bombs().count() >= 2, "bombs={}", w.bombs().count());
    }

    #[test]
    fn disease_spreads_to_healthy_bombers_on_the_same_tile() {
        let mut w = world_with(&[]);
        w.bombers[1] = Bomber::new(5 * 40, 5 * 40);
        w.bombers[0].infect(DiseaseKind::Stoned);
        w.step(&[]);
        assert_eq!(
            w.bombers[1].disease.map(|d| d.kind),
            Some(DiseaseKind::Stoned)
        );
    }

    #[test]
    fn a_good_extra_cures_and_drops_the_disease_extra() {
        let mut w = world_with(&[]);
        w.bombers[0].infect(DiseaseKind::Fast);
        put_extra(&mut w, ExtraKind::Power, 5, 5);
        w.step(&[]);
        assert!(!w.bombers[0].is_diseased());
        assert_eq!(w.bombers[0].speed, 90);
        let drops: Vec<_> = w
            .extras()
            .filter(|e| !e.delete_me)
            .map(|e| (e.kind, e.flight.active))
            .collect();
        assert_eq!(drops, vec![(ExtraKind::Koks, true)], "el Koks sale volando");
        // Tras el vuelo queda en una casilla libre, listo para recogerse.
        for _ in 0..120 {
            w.step(&[]);
        }
        let e = w.extras().next().unwrap();
        assert!(!e.flight.active);
        assert_eq!((e.x as i32 % 40, e.y as i32 % 40), (0, 0));
    }

    #[test]
    fn dying_drops_the_extras_you_had_gained() {
        let mut w = world_with(&[]);
        w.bombers[0].gain_extra_bomb();
        w.bombers[0].gain_extra_power();
        w.bombers[0].gain_extra_kick();
        let mut b = Bomb::new(5, 5, 1, 1);
        b.countdown = 0.01;
        w.objects.push(Object::Bomb(b));
        for _ in 0..8 {
            w.step(&[]);
        }
        assert!(w.bombers[0].dead);
        let mut kinds: Vec<_> = w.extras().map(|e| e.kind as u8).collect();
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                ExtraKind::Bomb as u8,
                ExtraKind::Power as u8,
                ExtraKind::Kick as u8
            ]
        );
    }

    #[test]
    fn explosions_make_disease_extras_fly_instead_of_destroying_them() {
        let mut w = world_with(&[]);
        w.bombers[0] = Bomber::new(1 * 40, 1 * 40);
        put_extra(&mut w, ExtraKind::Joint, 5, 6);
        w.objects.push(Object::Bomb(Bomb::new(5, 5, 2, 0)));
        for _ in 0..(2.5 * 60.0) as usize + 5 {
            w.step(&[]);
        }
        let e = w.extras().next().expect("sigue existiendo");
        assert!(
            e.flight.active || (e.x as i32, e.y as i32) != (5 * 40, 6 * 40),
            "salió despedido"
        );
        for _ in 0..180 {
            w.step(&[]);
        }
        assert_eq!(w.extras().count(), 1);
    }
}
