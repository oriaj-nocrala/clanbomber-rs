//! Ciclo de partida: entrada de los bombers, reloj de ronda, fin de tiempo, ganador,
//! puntos y siguiente ronda (ClanBomber2: Observer.cpp, GameStatus.cpp, Bomber::reset).

use crate::bomb::Object;
use crate::bomber::Bomber;
use crate::events::GameEvent;
use crate::flight::Flight;
use crate::map::TILE_PIXELS;
use crate::sim::{World, TICK_DT};

/// El observador (`Observer`): al agotarse el tiempo salta de casilla en casilla a 500 px/s
/// haciendo desaparecer el suelo.
#[derive(Clone, Debug)]
pub struct Observer {
    pub x: f32,
    pub y: f32,
    pub flight: Flight,
    first_destruction: bool,
}

impl Observer {
    pub fn new() -> Self {
        Observer {
            x: 0.0,
            y: 0.0,
            flight: Flight::default(),
            first_destruction: true,
        }
    }

    /// Casilla sobre la que está (`get_maptile()`).
    fn tile(&self) -> (i32, i32) {
        ((self.x as i32 + 20) / 40, (self.y as i32 + 20) / 40)
    }
}

impl Default for Observer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Playing,
    /// Pantalla de marcador entre rondas (espera `World::confirm`). `winner` es el último
    /// bomber vivo (`GameStatus::analyze_game`); `end_of_game` si alguien llegó a la meta.
    Scores {
        winner: Option<usize>,
        end_of_game: bool,
    },
}

impl World {
    /// Empieza una partida con `n` bombers (`init_game`): cada uno en una salida al azar,
    /// volando desde el centro a 300 px/s.
    pub fn start_match(&mut self, n: usize) {
        self.rounds_enabled = true;
        self.bombers = (0..n).map(|_| Bomber::new(0, 0)).collect();
        self.ais = (0..n).map(|_| None).collect();
        self.begin_round(300);
    }

    /// Estado de ronda nuevo sobre el mapa actual: salidas barajadas y bombers volando.
    fn begin_round(&mut self, fly_speed: i32) {
        self.assign_spawns();
        for b in &mut self.bombers {
            b.reset(&self.config);
            b.begin_round(fly_speed);
        }
        for ai in self.ais.iter_mut().flatten() {
            ai.reset();
        }
        self.round_time = self.config.round_time as f32;
        self.observer = Observer::new();
        self.hurry_played = false;
        self.phase = Phase::Playing;
    }

    /// `MapEntry::randomize_bomber_positions`: reparte las salidas al azar entre los bombers.
    fn assign_spawns(&mut self) {
        let t = TILE_PIXELS as i32;
        let slots = (self.map.max_players as usize)
            .min(self.map.spawns.len())
            .max(1);
        let mut taken = vec![false; slots];
        let mut perm = vec![0usize; slots];
        for item in perm.iter_mut() {
            loop {
                let p = self.rng.below(slots as u32) as usize;
                if !taken[p] {
                    taken[p] = true;
                    *item = p;
                    break;
                }
            }
        }
        for (i, b) in self.bombers.iter_mut().enumerate() {
            let (_, x, y) = self.map.spawns[perm[i % slots]];
            b.orig = (x as i32 * t, y as i32 * t);
        }
    }

    /// Asigna los equipos (`Config::bomber[i].team`). Los equipos de un solo jugador desaparecen
    /// (`init_game`: "removing teams which only one player is in").
    pub fn set_teams(&mut self, teams: &[u8]) {
        for (b, &t) in self.bombers.iter_mut().zip(teams) {
            b.team = t;
        }
        let mut count = [0usize; 4];
        for b in &self.bombers {
            if (1..=4).contains(&b.team) {
                count[b.team as usize - 1] += 1;
            }
        }
        for b in &mut self.bombers {
            if (1..=4).contains(&b.team) && count[b.team as usize - 1] == 1 {
                b.team = 0;
            }
        }
    }

    /// ¿Hay equipos en juego? Entonces se usa el marcador por equipos.
    pub fn is_team_game(&self) -> bool {
        self.bombers.iter().any(|b| b.team != 0)
    }

    /// `Observer::active_players`: jugadores vivos, contando un equipo una sola vez... pero solo
    /// si sus miembros vivos van seguidos en la lista (el original compara con el anterior).
    pub fn active_players(&self) -> usize {
        let mut count = 0;
        let mut last_team: i32 = -1;
        for b in self.bombers.iter().filter(|b| !b.dead) {
            if b.team as i32 != last_team || b.team == 0 {
                last_team = b.team as i32;
                count += 1;
            }
        }
        count
    }

    /// Segundos de ronda que quedan (negativo = tiempo agotado).
    pub fn time_left(&self) -> f32 {
        self.round_time
    }

    /// Parpadeo de "HURRY UP" entre los 21 y los 18 s (`Observer::show`).
    pub fn hurry_up(&self) -> bool {
        self.round_time < 21.0 && self.round_time > 18.0 && (self.round_time % 0.3) > 0.15
    }

    /// `Observer::act`. Devuelve `true` si la ronda acaba de terminar (el mundo se congela).
    pub(crate) fn observer_act(&mut self) -> bool {
        if !self.rounds_enabled || self.phase != Phase::Playing {
            return false;
        }
        // GameObject::act del observador.
        if self.observer.flight.active {
            let mut pos = (self.observer.x, self.observer.y);
            self.observer.flight.advance(&mut pos, TICK_DT, None);
            (self.observer.x, self.observer.y) = pos;
        }

        // El reloj no corre hasta que todos han aterrizado; entonces se encienden los controles.
        if self.round_time == self.config.round_time as f32 {
            if self.bombers.iter().any(|b| b.flight.active) {
                return false;
            }
            for b in &mut self.bombers {
                b.activate();
            }
        }
        self.round_time -= TICK_DT;
        // Aviso de que se acaba el tiempo (se oye una vez, al entrar en los últimos 21 s).
        if self.round_time < 21.0 && self.round_time > 18.0 && !self.hurry_played {
            self.hurry_played = true;
            self.events.push(GameEvent::HurryUp);
        }

        if self.round_time < 0.0 && !self.observer.flight.active {
            if self.observer.first_destruction {
                self.observer.first_destruction = false;
                self.events.push(GameEvent::TimeOver);
            } else {
                let (tx, ty) = self.observer.tile();
                self.vanish_tile(tx, ty);
                self.events.push(GameEvent::Crunch);
            }
            if let Some(t) = self.map.get_random(&mut self.rng) {
                let dest = (t.0 as i32 * 40, t.1 as i32 * 40);
                let from = (self.observer.x, self.observer.y);
                self.observer.flight.start(from, dest, 500);
            }
        }

        if self.active_players() < 2 {
            // Se espera a que acabe lo que está en marcha.
            if self.objects.iter().any(|o| match o {
                Object::Explosion(_) => true,
                Object::Extra(e) => e.flight.active,
                _ => false,
            }) {
                return false;
            }
            if self.bombers.iter().any(|b| !b.dead && b.is_falling()) {
                return false;
            }
            self.finish_round();
            return true;
        }
        false
    }

    /// Reparto de puntos y marcador al terminar la ronda.
    fn finish_round(&mut self) {
        for b in self.bombers.iter_mut().filter(|b| !b.dead) {
            b.points += 1;
        }
        let (winner, end_of_game) = self.analyze_game();

        // `load_next_valid`: el mapa siguiente se carga ya (el marcador enseña cuál viene).
        let next = match &mut self.playlist {
            Some(list) if self.config.random_map_order => list
                .random_for(self.bombers.len(), &mut self.rng)
                .unwrap_or_else(|_| self.pristine.clone()),
            Some(list) => list
                .next_for(self.bombers.len())
                .unwrap_or_else(|_| self.pristine.clone()),
            None => self.pristine.clone(),
        };
        self.next_map = Some(next);
        if end_of_game {
            self.events.push(GameEvent::Klatsch);
        }
        self.phase = Phase::Scores {
            winner,
            end_of_game,
        };
    }

    /// `GameStatus::analyze_game` / `GameStatus_Team::analyze_game`: quién gana la ronda y si
    /// con ella termina la partida.
    fn analyze_game(&self) -> (Option<usize>, bool) {
        let goal = self.config.points_to_win;
        if !self.is_team_game() {
            // El ganador es el último vivo; la partida acaba cuando alguien llega a la meta.
            let winner = self.bombers.iter().rposition(|b| !b.dead);
            return (winner, self.bombers.iter().any(|b| b.points == goal));
        }

        let mut winner = None;
        let mut end_of_game = false;
        let mut team_count = [0i32; 4];
        let mut team_points = [0i32; 4];
        for (i, b) in self.bombers.iter().enumerate() {
            if b.team == 0 {
                if !b.dead {
                    winner = Some(i);
                    if b.points == goal {
                        end_of_game = true;
                    }
                }
            } else {
                if !b.dead {
                    winner = Some(i);
                }
                team_count[b.team as usize - 1] += 1;
                team_points[b.team as usize - 1] += b.points;
            }
        }
        // Un equipo gana la partida cuando la media de sus miembros llega a la meta.
        for t in 0..4 {
            if team_count[t] > 0 && team_points[t] >= team_count[t] * goal {
                end_of_game = true;
            }
        }
        (winner, end_of_game)
    }

    /// Nombre del mapa de la siguiente ronda (para el marcador).
    pub fn next_map_name(&self) -> Option<&str> {
        self.next_map.as_ref().map(|m| m.name.as_str())
    }

    /// Pasa del marcador a la siguiente ronda; si la partida había terminado, empieza otra
    /// con los puntos a cero (el original vuelve al menú).
    pub fn confirm(&mut self) {
        let Phase::Scores { end_of_game, .. } = self.phase else {
            return;
        };
        self.objects.clear();
        self.trapped.clear();
        let mut map = self
            .next_map
            .take()
            .unwrap_or_else(|| self.pristine.clone());
        map.resolve_random_boxes(&mut self.rng);
        self.set_map(map);
        if end_of_game {
            for b in &mut self.bombers {
                b.points = 0;
            }
        }
        self.begin_round(if end_of_game { 300 } else { 200 });
    }

    /// `MapTile::vanish` de la casilla bajo el observador: los extras de encima huyen.
    fn vanish_tile(&mut self, tx: i32, ty: i32) {
        if !self.map.vanish(tx, ty) {
            return;
        }
        let ids: Vec<usize> = self
            .objects
            .iter()
            .enumerate()
            .filter_map(|(i, o)| match o {
                Object::Extra(e) if e.rests_on_tile() && e.tile() == (tx, ty) => Some(i),
                _ => None,
            })
            .collect();
        for i in ids {
            if let Some(t) = self.get_passable() {
                if let Object::Extra(e) = &mut self.objects[i] {
                    e.fly_to(t);
                }
            }
        }
    }
}
