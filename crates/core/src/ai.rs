//! Controlador de IA "DOK" (ClanBomber2: Controller_AI.cpp).
//!
//! Cada tick valora cada casilla (peligro de bombas y explosiones, extras, obstáculos) y
//! planifica una cola de trabajos: huir del fuego, ir a por un extra, o llegar a una casilla
//! donde una bomba haga daño y poner una (comprobando antes que hay una salida).

use crate::bomb::Object;
use crate::bomber::{Bomber, Input};
use crate::config::Config;
use crate::extra::ExtraKind;
use crate::map::{Dir, Map, Tile, MAP_HEIGHT, MAP_WIDTH};
use crate::rng::Rng;

/// Un extra cualquiera.
const RATING_EXTRA: i32 = 30;
/// La bomba va a explotar y alcanzará esta casilla.
const RATING_HOT: i32 = -100;
/// Muerte segura: explosión, hueco.
const RATING_X: i32 = -666;
/// Nunca chocar con muros.
const RATING_BLOCKING: i32 = -1000;

const DRATING_ENEMY: i32 = 150;
const DRATING_BOX: i32 = 20;
const DRATING_DISEASE: i32 = 10;
const DRATING_DBOX: i32 = -50;
const DRATING_EXTRA: i32 = -90;
const DRATING_BOMB: i32 = -100;
/// Amigo en el radio de la bomba. Sin equipos todos son enemigos, así que no se usa todavía.
#[allow(dead_code)]
const DRATING_FRIEND: i32 = -200;

type Grid = [[i32; MAP_HEIGHT]; MAP_WIDTH];

/// Lo que la IA ve del mundo en un tick.
pub struct AiView<'a> {
    pub map: &'a Map,
    pub objects: &'a [Object],
    pub bombers: &'a [Bomber],
    /// Índice del bomber que controla.
    pub me: usize,
    pub rng: &'a mut Rng,
    pub config: &'a Config,
}

/// `Extra::get_ExtraType()`: las enfermedades son negativas.
fn extra_type(k: ExtraKind) -> i32 {
    match k {
        ExtraKind::Viagra => -3,
        ExtraKind::Joint => -2,
        ExtraKind::Koks => -1,
        ExtraKind::Glove => 1,
        ExtraKind::Kick => 2,
        ExtraKind::Skateboard => 3,
        ExtraKind::Bomb => 4,
        ExtraKind::Power => 5,
    }
}

#[derive(Clone, Debug)]
enum Job {
    /// Avanzar `distance` casillas hacia `dir`; `start` es la casilla de partida.
    Go {
        dir: Dir,
        distance: i32,
        start: i32,
        finished: bool,
        obsolete: bool,
    },
    PutBomb {
        finished: bool,
        obsolete: bool,
    },
}

impl Job {
    fn finished(&self) -> bool {
        match self {
            Job::Go { finished, .. } | Job::PutBomb { finished, .. } => *finished,
        }
    }

    fn obsolete(&self) -> bool {
        match self {
            Job::Go { obsolete, .. } | Job::PutBomb { obsolete, .. } => *obsolete,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ai {
    jobs: Vec<Job>,
    dir: Option<Dir>,
    put_bomb: bool,
    rmap: Grid,
}

impl Default for Ai {
    fn default() -> Self {
        Self::new()
    }
}

fn in_map(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && (x as usize) < MAP_WIDTH && (y as usize) < MAP_HEIGHT
}

impl AiView<'_> {
    fn me(&self) -> &Bomber {
        &self.bombers[self.me]
    }

    /// `GameObject::get_map_x/y`: casilla bajo el centro, recortada al mapa.
    fn map_pos(&self) -> (i32, i32) {
        let b = self.me();
        (
            ((b.get_x() + 20) / 40).clamp(0, MAP_WIDTH as i32 - 1),
            ((b.get_y() + 20) / 40).clamp(0, MAP_HEIGHT as i32 - 1),
        )
    }

    /// Tipo de casilla; fuera del mapa se comporta como el muro `dummy_maptile`.
    fn kind(&self, x: i32, y: i32) -> Tile {
        if in_map(x, y) {
            self.map.get(x, y)
        } else {
            Tile::Wall
        }
    }

    fn blocking(&self, x: i32, y: i32) -> bool {
        self.map.blocking_tile(x, y)
    }

    fn has_extra(&self, x: i32, y: i32) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(o, Object::Extra(e) if e.rests_on_tile() && e.tile() == (x, y)))
    }

    fn has_bomb(&self, x: i32, y: i32) -> bool {
        self.objects
            .iter()
            .any(|o| matches!(o, Object::Bomb(b) if b.rests_on_tile() && b.tile() == (x, y)))
    }

    /// `MapTile::destroyed`: caja a punto de romperse.
    fn destroyed(&self, x: i32, y: i32) -> bool {
        self.map.is_crumbling(x, y)
    }

    /// `bomber_rating`: enemigos en la casilla suman, amigos restan (sin equipos: todos enemigos).
    fn bomber_rating(&self, x: i32, y: i32) -> i32 {
        self.bombers
            .iter()
            .enumerate()
            .filter(|(i, b)| *i != self.me && b.on_ground() && b.tile() == (x, y))
            .map(|_| DRATING_ENEMY)
            .sum()
    }

    /// `extra_rating`: las enfermedades valen un poco; los extras buenos, no volarlos.
    fn extra_rating(&self, x: i32, y: i32) -> i32 {
        let r: i32 = self
            .objects
            .iter()
            .filter_map(|o| match o {
                Object::Extra(e) if e.rests_on_tile() && e.tile() == (x, y) => {
                    Some(extra_type(e.kind) * DRATING_EXTRA)
                }
                _ => None,
            })
            .sum();
        if r > 0 {
            DRATING_DISEASE
        } else {
            r
        }
    }
}

impl Ai {
    pub fn new() -> Self {
        Ai {
            jobs: Vec::new(),
            dir: None,
            put_bomb: false,
            rmap: [[0; MAP_HEIGHT]; MAP_WIDTH],
        }
    }

    /// `Controller_AI::reset`: olvida lo planeado (al empezar una ronda).
    pub fn reset(&mut self) {
        self.dir = None;
        self.put_bomb = false;
        self.jobs.clear();
    }

    /// `Controller_AI::update` + `is_left/right/up/down/bomb`: las teclas que "pulsa" este tick.
    pub fn update(&mut self, v: &mut AiView) -> Input {
        let active = v.me().active();
        if active {
            self.generate_rating_map(v);
            if self.job_ready(v) {
                self.do_job(v);
            }
        }
        Input {
            left: self.dir == Some(Dir::Left) && active,
            right: self.dir == Some(Dir::Right) && active,
            up: self.dir == Some(Dir::Up) && active,
            down: self.dir == Some(Dir::Down) && active,
            bomb: self.put_bomb && active,
        }
    }

    fn rating(&self, x: i32, y: i32) -> i32 {
        if in_map(x, y) {
            self.rmap[x as usize][y as usize]
        } else {
            RATING_BLOCKING
        }
    }

    fn is_hotspot(&self, x: i32, y: i32) -> bool {
        self.rating(x, y) <= RATING_HOT
    }

    fn is_death(&self, x: i32, y: i32) -> bool {
        self.rating(x, y) <= RATING_X
    }

    // ---- Trabajos ----

    /// `Job_Go::init`: apunta la casilla de partida y se descarta si el primer paso es mortal.
    fn new_go(&mut self, v: &AiView, dir: Dir) -> Job {
        let (mx, my) = v.map_pos();
        let (ahead, start) = match dir {
            Dir::Up => ((mx, my - 1), my),
            Dir::Down => ((mx, my + 1), my),
            Dir::Left => ((mx - 1, my), mx),
            Dir::Right => ((mx + 1, my), mx),
        };
        let obsolete = self.is_death(ahead.0, ahead.1);
        if obsolete {
            self.dir = None;
        }
        Job::Go {
            dir,
            distance: 1,
            start,
            finished: false,
            obsolete,
        }
    }

    /// Destructor de un trabajo: devuelve el control a neutro.
    fn drop_job(&mut self, job: &Job) {
        match job {
            Job::Go { .. } => self.dir = None,
            Job::PutBomb { .. } => self.put_bomb = false,
        }
    }

    fn clear_all_jobs(&mut self) {
        for j in std::mem::take(&mut self.jobs) {
            self.drop_job(&j);
        }
    }

    /// `job_ready`: descarta lo obsoleto o terminado y busca trabajo nuevo si no queda nada.
    fn job_ready(&mut self, v: &mut AiView) -> bool {
        loop {
            if self.jobs.is_empty() {
                self.find_new_jobs(v);
                return !self.jobs.is_empty();
            }
            if self.jobs[0].obsolete() {
                self.clear_all_jobs();
                continue;
            }
            if self.jobs[0].finished() {
                let done = self.jobs.remove(0);
                self.drop_job(&done);
                if !self.jobs.is_empty() {
                    self.reinit_head(v);
                }
                continue;
            }
            return true;
        }
    }

    /// `Job::init()` del primero de la cola al llegarle el turno.
    fn reinit_head(&mut self, v: &AiView) {
        if let Some(Job::Go { dir, .. }) = self.jobs.first().cloned() {
            let fresh = self.new_go(v, dir);
            // `init` solo puede marcar como obsoleto, nunca revertirlo.
            if let (
                Job::Go { obsolete: old, .. },
                Job::Go {
                    start, obsolete, ..
                },
            ) = (&mut self.jobs[0], fresh)
            {
                *old |= obsolete;
                if let Job::Go { start: s, .. } = &mut self.jobs[0] {
                    *s = start;
                }
            }
        }
    }

    /// `do_job`: ejecuta el primer trabajo.
    fn do_job(&mut self, v: &AiView) {
        let (mx, my) = v.map_pos();
        let (gx, gy) = (v.me().get_x(), v.me().get_y());
        let stopped = v.me().stopped;
        let cur_bombs = v.me().cur_bombs;
        let bomb_here = v.has_bomb((gx + 20) / 40, (gy + 20) / 40);
        match self.jobs.first_mut() {
            Some(Job::Go {
                dir,
                distance,
                start,
                finished,
                obsolete,
            }) => {
                self.dir = Some(*dir);
                let done = match dir {
                    Dir::Up => my <= *start - *distance && gy % 40 < 15,
                    Dir::Down => my >= *start + *distance && gy % 40 > 25,
                    Dir::Left => mx <= *start - *distance && gx % 40 < 15,
                    Dir::Right => mx >= *start + *distance && gx % 40 > 25,
                };
                if done {
                    *finished = true;
                    self.dir = None;
                }
                if stopped {
                    *obsolete = true;
                }
            }
            Some(Job::PutBomb { finished, obsolete }) => {
                self.put_bomb = true;
                *finished = true;
                if bomb_here || cur_bombs == 0 {
                    *obsolete = true;
                }
            }
            None => {}
        }
    }

    fn find_new_jobs(&mut self, v: &mut AiView) {
        if self.avoid_bombs(v) {
            return;
        }
        if self.find_way(v, RATING_EXTRA, RATING_HOT, 10) {
            return;
        }
        if v.me().cur_bombs > 0 && self.free_extras(v, 5) {}
    }

    /// Si está sobre una casilla peligrosa, huye al sitio seguro más cercano.
    fn avoid_bombs(&mut self, v: &mut AiView) -> bool {
        let (x, y) = v.map_pos();
        if self.is_hotspot(x, y) {
            if !self.find_way(v, 0, -1, 666) && !self.find_way(v, 0, RATING_HOT, 666) {
                self.find_way(v, 0, RATING_X, 666);
            }
            return true;
        }
        false
    }

    // ---- Mapa de valoraciones ----

    fn generate_rating_map(&mut self, v: &AiView) {
        self.rmap = [[0; MAP_HEIGHT]; MAP_WIDTH];
        for o in v.objects {
            match o {
                Object::Bomb(b) => {
                    let (x, y) = (
                        ((b.get_x() + 20) / 40).clamp(0, 16),
                        ((b.get_y() + 20) / 40).clamp(0, 12),
                    );
                    self.apply_bomb_rating(v, x, y, b.power, b.countdown, b.cur_dir);
                }
                Object::Explosion(e) => {
                    let (x, y) = (e.tx, e.ty);
                    self.add(x, y, RATING_X);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        for i in 1..=e.power {
                            let (nx, ny) = (x + dx * i, y + dy * i);
                            if !in_map(nx, ny) || v.blocking(nx, ny) || v.has_extra(nx, ny) {
                                break;
                            }
                            self.add(nx, ny, RATING_X);
                        }
                    }
                }
                Object::Extra(e) => {
                    let (x, y) = (
                        ((e.x as i32 + 20) / 40).clamp(0, 16),
                        ((e.y as i32 + 20) / 40).clamp(0, 12),
                    );
                    self.add(x, y, RATING_EXTRA * extra_type(e.kind));
                    if self.rating(x, y) < 0 {
                        self.rmap[x as usize][y as usize] = RATING_HOT;
                    }
                }
            }
        }
        // Valoración del terreno.
        for y in 0..MAP_HEIGHT as i32 {
            for x in 0..MAP_WIDTH as i32 {
                if v.blocking(x, y) {
                    self.add(x, y, RATING_BLOCKING);
                } else if !v.map.is_passable(x, y) {
                    self.add(x, y, RATING_X);
                }
            }
        }
    }

    fn add(&mut self, x: i32, y: i32, r: i32) {
        if in_map(x, y) {
            self.rmap[x as usize][y as usize] += r;
        }
    }

    /// `apply_bomb_rating`: marca el alcance de una bomba (y de su trayectoria si se mueve).
    fn apply_bomb_rating(
        &mut self,
        v: &AiView,
        mut x: i32,
        mut y: i32,
        power: i32,
        countdown: f32,
        dir: Option<Dir>,
    ) {
        let me = v.me();
        let mut rating = RATING_HOT;
        if countdown > v.config.bomb_countdown as f32 / 10.0 - 0.1 {
            rating = -1;
        } else if countdown < 40.0 / me.speed.max(1) as f32 {
            rating = RATING_X;
        }

        // Una bomba quieta bloquea (salvo que se pueda patear y no esté sobre una flecha).
        let on_arrow = matches!(v.kind(x, y), Tile::Arrow(_));
        if in_map(x, y) {
            self.rmap[x as usize][y as usize] = if me.can_kick && !on_arrow {
                rating
            } else {
                RATING_BLOCKING
            };
        }

        // Cuántas casillas recorrerá en su dirección antes de parar.
        let mut length = 1;
        if let Some(d) = dir {
            let (dx, dy) = match d {
                Dir::Up => (0, -1),
                Dir::Down => (0, 1),
                Dir::Left => (-1, 0),
                Dir::Right => (1, 0),
            };
            loop {
                let (nx, ny) = (x + dx * length, y + dy * length);
                if v.blocking(nx, ny) || !in_map(nx, ny) {
                    break;
                }
                length += 1;
                let (ax, ay) = (x + dx * length, y + dy * length);
                // Una flecha que no mira en su dirección la detiene.
                if matches!(v.kind(ax, ay), Tile::Arrow(ad) if ad != d) {
                    break;
                }
            }
        }

        while length > 0 {
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for i in 1..=power {
                    let (nx, ny) = (x + dx * i, y + dy * i);
                    if !in_map(nx, ny) {
                        break;
                    }
                    self.rmap[nx as usize][ny as usize] += rating;
                    if v.blocking(nx, ny) || v.has_extra(nx, ny) {
                        break;
                    }
                }
            }
            match dir {
                Some(Dir::Up) => y -= 1,
                Some(Dir::Down) => y += 1,
                Some(Dir::Left) => x -= 1,
                Some(Dir::Right) => x += 1,
                None => {}
            }
            length -= 1;
        }
    }

    // ---- Búsqueda de caminos ----

    /// `find_way`: busca (en anchura, con vecinos en orden aleatorio) la casilla más cercana con
    /// valoración `>= dest_rating` sin pisar nada `<= avoid_rating`, y encola el camino. Para
    /// un extra (`dest_rating > 0`) solo se encola el primer paso.
    fn find_way(
        &mut self,
        v: &mut AiView,
        dest_rating: i32,
        avoid_rating: i32,
        max_distance: i32,
    ) -> bool {
        let start = v.map_pos();
        let Some(path) = self.bfs_path(v, start, dest_rating, avoid_rating, max_distance) else {
            return false;
        };
        // `path` va del inicio al destino; cada paso se crea con el estado actual.
        let jobs: Vec<Job> = path.iter().map(|&d| self.new_go(v, d)).collect();
        if dest_rating > 0 {
            // `rev_way.back()` es el primer paso.
            self.jobs.push(jobs[0].clone());
        } else {
            self.jobs.extend(jobs);
        }
        true
    }

    /// El camino (como pasos) de `start` a la casilla destino, o `None`.
    fn bfs_path(
        &self,
        v: &mut AiView,
        start: (i32, i32),
        dest_rating: i32,
        avoid_rating: i32,
        max_distance: i32,
    ) -> Option<Vec<Dir>> {
        let mut vmap = [[-1i32; MAP_HEIGHT]; MAP_WIDTH];
        let mut new_queue = std::collections::VecDeque::new();
        let mut working = std::collections::VecDeque::new();
        let mut distance = 0;
        let mut dest: Option<(i32, i32)> = None;

        vmap[start.0 as usize][start.1 as usize] = 0;
        new_queue.push_back(start);

        loop {
            working.append(&mut new_queue);
            distance += 1;
            loop {
                let Some(p) = working.pop_front() else { break };
                let mut dirs = [Dir::Up, Dir::Right, Dir::Down, Dir::Left];
                for _ in 0..10 {
                    let a = v.rng.below(4) as usize;
                    let b = v.rng.below(4) as usize;
                    dirs.swap(a, b);
                }
                for d in dirs {
                    let (dx, dy) = match d {
                        Dir::Up => (0, -1),
                        Dir::Right => (1, 0),
                        Dir::Down => (0, 1),
                        Dir::Left => (-1, 0),
                    };
                    let n = (p.0 + dx, p.1 + dy);
                    if !in_map(n.0, n.1) {
                        continue;
                    }
                    let r = self.rmap[n.0 as usize][n.1 as usize];
                    if r > avoid_rating && vmap[n.0 as usize][n.1 as usize] < 0 {
                        new_queue.push_back(n);
                        vmap[n.0 as usize][n.1 as usize] = distance;
                        if r >= dest_rating {
                            dest = Some(n);
                        }
                    }
                }
                if dest.is_some() || working.is_empty() {
                    break;
                }
            }
            if !(max_distance >= distance && dest.is_none() && !new_queue.is_empty()) {
                break;
            }
        }

        let mut cur = dest?;
        let mut rev: Vec<Dir> = Vec::new();
        while cur != start {
            distance -= 1;
            // Mismo orden de prueba que el original: arriba, derecha, abajo, izquierda; el paso
            // que se guarda es el inverso (de `cur` hacia el inicio).
            let tries = [
                (0, -1, Dir::Down),
                (1, 0, Dir::Left),
                (0, 1, Dir::Up),
                (-1, 0, Dir::Right),
            ];
            let mut moved = false;
            for (dx, dy, step) in tries {
                let n = (cur.0 + dx, cur.1 + dy);
                if in_map(n.0, n.1) && vmap[n.0 as usize][n.1 as usize] == distance {
                    rev.push(step);
                    cur = n;
                    moved = true;
                    break;
                }
            }
            if !moved {
                // No debería pasar: el mapa de distancias siempre tiene un predecesor.
                return None;
            }
        }
        rev.reverse();
        Some(rev)
    }

    /// `free_extras`: busca la casilla cercana donde una bomba haría más (cajas, enemigos),
    /// comprobando que habría salida, y encola ir allí y ponerla.
    fn free_extras(&mut self, v: &mut AiView, max_distance: i32) -> bool {
        let power = v.me().power;
        let start = v.map_pos();
        let mut vmap = [[-1i32; MAP_HEIGHT]; MAP_WIDTH];
        let mut dmap = [[0i32; MAP_HEIGHT]; MAP_WIDTH];
        let mut new_queue = std::collections::VecDeque::new();
        let mut working = std::collections::VecDeque::new();
        let mut distance = 0;
        let mut best: Option<(i32, i32)> = None;
        let mut best_z = 0;

        vmap[start.0 as usize][start.1 as usize] = 0;
        new_queue.push_back(start);

        'outer: loop {
            working.append(&mut new_queue);
            distance += 1;
            loop {
                let Some((x, y)) = working.pop_front() else {
                    break;
                };

                // Qué daría una bomba aquí, brazo a brazo.
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    for i in 1..=power {
                        let (nx, ny) = (x + dx * i, y + dy * i);
                        if !in_map(nx, ny) || v.kind(nx, ny) == Tile::Wall {
                            break;
                        }
                        dmap[x as usize][y as usize] += v.bomber_rating(nx, ny);
                        if v.has_bomb(nx, ny) {
                            dmap[x as usize][y as usize] += DRATING_BOMB;
                            break;
                        }
                        if v.has_extra(nx, ny) {
                            dmap[x as usize][y as usize] += v.extra_rating(nx, ny);
                            break;
                        }
                        if self.rmap[nx as usize][ny as usize] < RATING_BLOCKING
                            || v.destroyed(nx, ny)
                        {
                            dmap[x as usize][y as usize] += DRATING_DBOX;
                            break;
                        }
                        if v.kind(nx, ny) == Tile::Box {
                            dmap[x as usize][y as usize] += DRATING_BOX;
                            break;
                        }
                        if v.blocking(nx, ny) {
                            break;
                        }
                    }
                }

                // ¿Habría salida tras poner una bomba aquí? Se simula con el mapa modificado.
                if dmap[x as usize][y as usize] > 0 {
                    let saved = self.rmap;
                    let countdown = v.config.bomb_countdown as f32 / 10.0;
                    self.apply_bomb_rating(v, x, y, power, countdown, None);
                    self.rmap[x as usize][y as usize] = -1;
                    if self.bfs_path(v, (x, y), 0, RATING_HOT, 666).is_none() {
                        dmap[x as usize][y as usize] = -1000;
                    }
                    self.rmap = saved;
                }

                let d = dmap[x as usize][y as usize];
                if d > 0 && d == best_z && v.rng.below(2) != 0 {
                    if distance >= max_distance {
                        break 'outer;
                    }
                    best = Some((x, y));
                }
                if d > best_z {
                    if best_z > 0 && distance >= max_distance {
                        break 'outer;
                    }
                    best = Some((x, y));
                    best_z = d;
                }

                // Vecinos transitables y seguros: arriba, derecha, abajo, izquierda.
                for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
                    let n = (x + dx, y + dy);
                    if in_map(n.0, n.1)
                        && self.rmap[n.0 as usize][n.1 as usize] > RATING_HOT
                        && vmap[n.0 as usize][n.1 as usize] < 0
                    {
                        new_queue.push_back(n);
                        vmap[n.0 as usize][n.1 as usize] = distance;
                    }
                }
                if working.is_empty() {
                    break;
                }
            }
            if new_queue.is_empty() || best_z == 666 {
                break;
            }
        }

        let Some(best) = best else { return false };
        // Camino de `start` a `best` siguiendo las distancias de la búsqueda.
        let mut cur = best;
        let mut dist = vmap[best.0 as usize][best.1 as usize];
        let mut rev = Vec::new();
        while cur != start {
            dist -= 1;
            let tries = [
                (0, -1, Dir::Down),
                (1, 0, Dir::Left),
                (0, 1, Dir::Up),
                (-1, 0, Dir::Right),
            ];
            let mut moved = false;
            for (dx, dy, step) in tries {
                let n = (cur.0 + dx, cur.1 + dy);
                if in_map(n.0, n.1) && vmap[n.0 as usize][n.1 as usize] == dist {
                    rev.push(step);
                    cur = n;
                    moved = true;
                    break;
                }
            }
            if !moved {
                return false;
            }
        }
        rev.reverse();
        let jobs: Vec<Job> = rev.iter().map(|&d| self.new_go(v, d)).collect();
        self.jobs.extend(jobs);
        self.jobs.push(Job::PutBomb {
            finished: false,
            obsolete: false,
        });
        true
    }
}
