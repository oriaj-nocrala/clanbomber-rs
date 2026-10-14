//! Menú principal en árbol (ClanBomber2: Menu.cpp y la construcción en `ClanBomberApplication::main`).

use cb_core::Config;
use winit::keyboard::KeyCode;

use crate::video::Video;

/// Identificador de cada entrada; el programa decide qué hacer con las acciones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    Root,
    NewGame,
    StartLocal,
    PlayerSetup,
    MapSelection,
    RandomPositions,
    RandomMapOrder,
    Points,
    RoundTime,
    Options,
    ExtraValues,
    StartBombs,
    StartPower,
    StartSkates,
    StartKick,
    StartGlove,
    MaxBombs,
    MaxPower,
    MaxSkates,
    ExtraOnOff,
    Bombs,
    Power,
    Skates,
    Kick,
    Glove,
    DiseaseOnOff,
    Joint,
    Viagra,
    Koks,
    Timing,
    BombCountdown,
    BombDelay,
    BombSpeed,
    Graphics,
    KidsMode,
    CorpseParts,
    Shake,
    // Efectos de la GPU (no existen en el original; se guardan en `Video`).
    Bloom,
    Particles,
    Crt,
    FireShader,
    Shockwaves,
    SpriteFx,
    Lighting,
    MapEditor,
    Credits,
    Help,
    // Los mismos ajustes aparecen también en Options (`*_DUPLICATE` del original).
    RandomPositions2,
    RandomMapOrder2,
    Points2,
    RoundTime2,
    Quit,
}

impl Id {
    /// Valor de la opción en la configuración (las entradas que no son valores devuelven 0).
    pub fn get(self, c: &Config) -> i32 {
        let b = |v: bool| v as i32;
        match self {
            Id::RandomPositions | Id::RandomPositions2 => b(c.random_positions),
            Id::RandomMapOrder | Id::RandomMapOrder2 => b(c.random_map_order),
            Id::Points | Id::Points2 => c.points_to_win,
            Id::RoundTime | Id::RoundTime2 => c.round_time,
            Id::StartBombs => c.start_bombs,
            Id::StartPower => c.start_power,
            Id::StartSkates => c.start_skateboards,
            Id::StartKick => b(c.start_kick),
            Id::StartGlove => b(c.start_glove),
            Id::MaxBombs => c.max_bombs,
            Id::MaxPower => c.max_power,
            Id::MaxSkates => c.max_skateboards,
            Id::Bombs => b(c.bombs),
            Id::Power => b(c.power),
            Id::Skates => b(c.skateboards),
            Id::Kick => b(c.kick),
            Id::Glove => b(c.glove),
            Id::Joint => b(c.joint),
            Id::Viagra => b(c.viagra),
            Id::Koks => b(c.koks),
            Id::BombCountdown => c.bomb_countdown,
            Id::BombDelay => c.bomb_delay,
            Id::BombSpeed => c.bomb_speed,
            Id::KidsMode => b(c.kids_mode),
            Id::CorpseParts => c.corpse_parts,
            Id::Shake => b(c.shaky_explosions),
            _ => 0,
        }
    }

    pub fn set(self, c: &mut Config, v: i32) {
        let b = v != 0;
        match self {
            Id::RandomPositions | Id::RandomPositions2 => c.random_positions = b,
            Id::RandomMapOrder | Id::RandomMapOrder2 => c.random_map_order = b,
            Id::Points | Id::Points2 => c.points_to_win = v,
            Id::RoundTime | Id::RoundTime2 => c.round_time = v,
            Id::StartBombs => c.start_bombs = v,
            Id::StartPower => c.start_power = v,
            Id::StartSkates => c.start_skateboards = v,
            Id::StartKick => c.start_kick = b,
            Id::StartGlove => c.start_glove = b,
            Id::MaxBombs => c.max_bombs = v,
            Id::MaxPower => c.max_power = v,
            Id::MaxSkates => c.max_skateboards = v,
            Id::Bombs => c.bombs = b,
            Id::Power => c.power = b,
            Id::Skates => c.skateboards = b,
            Id::Kick => c.kick = b,
            Id::Glove => c.glove = b,
            Id::Joint => c.joint = b,
            Id::Viagra => c.viagra = b,
            Id::Koks => c.koks = b,
            Id::BombCountdown => c.bomb_countdown = v,
            Id::BombDelay => c.bomb_delay = v,
            Id::BombSpeed => c.bomb_speed = v,
            Id::KidsMode => c.kids_mode = b,
            Id::CorpseParts => c.corpse_parts = v,
            Id::Shake => c.shaky_explosions = b,
            _ => {}
        }
    }
}

enum Kind {
    /// Abre un submenú (si tiene hijos) o dispara una acción.
    Item,
    Value {
        min: i32,
        max: i32,
        value: i32,
    },
}

struct Item {
    id: Id,
    text: String,
    parent: Option<usize>,
    children: Vec<usize>,
    kind: Kind,
}

/// Un submenú tal como se muestra.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: Id,
    /// Niveles por debajo de la raíz.
    pub depth: usize,
    pub title: String,
    /// Nombre y valor (si es una opción) de cada fila.
    pub rows: Vec<(String, Option<String>)>,
    pub selection: usize,
}

/// Lo que ocurrió tras una pulsación.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    /// Se pulsó Enter sobre una acción.
    Activate(Id),
    /// Se cambió un valor con izquierda/derecha.
    Changed(Id, i32),
}

pub struct Menu {
    items: Vec<Item>,
    /// Submenú abierto (índice en `items`).
    current: usize,
    /// Fila seleccionada dentro del submenú (0 = primera).
    selection: usize,
}

impl Menu {
    fn add(&mut self, parent: usize, id: Id, text: &str, kind: Kind) {
        let idx = self.items.len();
        self.items.push(Item {
            id,
            text: text.to_owned(),
            parent: Some(parent),
            children: Vec::new(),
            kind,
        });
        self.items[parent].children.push(idx);
    }

    fn index_of(&self, id: Id) -> usize {
        self.items
            .iter()
            .position(|i| i.id == id)
            .expect("entrada de menú")
    }

    /// Construye el menú con los valores actuales de `c` (mismas entradas y límites que el original).
    pub fn new(c: &Config) -> Menu {
        let mut m = Menu {
            items: vec![Item {
                id: Id::Root,
                text: "Main Menu".into(),
                parent: None,
                children: Vec::new(),
                kind: Kind::Item,
            }],
            current: 0,
            selection: 0,
        };
        let value = |id: Id, min: i32, max: i32| Kind::Value {
            min,
            max,
            value: id.get(c),
        };
        let root = 0;

        m.add(root, Id::NewGame, "New Game", Kind::Item);
        let g = m.index_of(Id::NewGame);
        m.add(g, Id::StartLocal, "Start Local", Kind::Item);
        m.add(g, Id::PlayerSetup, "Local Player Setup", Kind::Item);
        m.add(g, Id::MapSelection, "Map Selection", Kind::Item);
        m.add(
            g,
            Id::RandomPositions,
            "Random Bomber Positions",
            value(Id::RandomPositions, 0, 1),
        );
        m.add(
            g,
            Id::RandomMapOrder,
            "Random Map Order",
            value(Id::RandomMapOrder, 0, 1),
        );
        m.add(g, Id::Points, "Points to win", value(Id::Points, 1, 10));
        m.add(g, Id::RoundTime, "Round Time", value(Id::RoundTime, 5, 300));

        m.add(root, Id::Options, "Options", Kind::Item);
        let o = m.index_of(Id::Options);
        m.add(o, Id::ExtraValues, "Start/Max Extras", Kind::Item);
        let e = m.index_of(Id::ExtraValues);
        m.add(
            e,
            Id::StartBombs,
            "Start Bombs",
            value(Id::StartBombs, 1, 15),
        );
        m.add(
            e,
            Id::StartPower,
            "Start Power",
            value(Id::StartPower, 1, 15),
        );
        m.add(
            e,
            Id::StartSkates,
            "Start Skateboards",
            value(Id::StartSkates, 0, 10),
        );
        m.add(e, Id::StartKick, "Start Kick", value(Id::StartKick, 0, 1));
        m.add(
            e,
            Id::StartGlove,
            "Start Glove",
            value(Id::StartGlove, 0, 1),
        );
        m.add(e, Id::MaxBombs, "Max. Bombs", value(Id::MaxBombs, 1, 15));
        m.add(e, Id::MaxPower, "Max. Power", value(Id::MaxPower, 1, 15));
        m.add(
            e,
            Id::MaxSkates,
            "Max. Skateboards",
            value(Id::MaxSkates, 0, 10),
        );

        m.add(o, Id::ExtraOnOff, "Enable/Disable Extras", Kind::Item);
        let x = m.index_of(Id::ExtraOnOff);
        m.add(x, Id::Bombs, "Bombs", value(Id::Bombs, 0, 1));
        m.add(x, Id::Power, "Power", value(Id::Power, 0, 1));
        m.add(x, Id::Skates, "Skateboard", value(Id::Skates, 0, 1));
        m.add(x, Id::Kick, "Kick", value(Id::Kick, 0, 1));
        m.add(x, Id::Glove, "Glove", value(Id::Glove, 0, 1));

        m.add(o, Id::DiseaseOnOff, "Enable/Disable Diseases", Kind::Item);
        let d = m.index_of(Id::DiseaseOnOff);
        m.add(d, Id::Joint, "Joint", value(Id::Joint, 0, 1));
        m.add(d, Id::Viagra, "Viagra", value(Id::Viagra, 0, 1));
        m.add(d, Id::Koks, "Koks", value(Id::Koks, 0, 1));

        m.add(o, Id::Timing, "Bomb Timing and Speed", Kind::Item);
        let t = m.index_of(Id::Timing);
        m.add(
            t,
            Id::BombCountdown,
            "Bomb Countdown (1/10 s)",
            value(Id::BombCountdown, 0, 50),
        );
        m.add(
            t,
            Id::BombDelay,
            "Bomb Chain Reaction Delay (1/100 s)",
            value(Id::BombDelay, 0, 50),
        );
        m.add(
            t,
            Id::BombSpeed,
            "Moving Bombs Speed (pixels per second)",
            value(Id::BombSpeed, 10, 500),
        );

        m.add(o, Id::Graphics, "Graphic Options", Kind::Item);
        let gr = m.index_of(Id::Graphics);
        m.add(gr, Id::KidsMode, "Kidz Mode", value(Id::KidsMode, 0, 1));
        m.add(
            gr,
            Id::CorpseParts,
            "Corpse Parts",
            value(Id::CorpseParts, 0, 100),
        );
        m.add(gr, Id::Shake, "Shaky Explosions", value(Id::Shake, 0, 1));
        m.add(gr, Id::Bloom, "Glow (Bloom)", value(Id::Bloom, 0, 1));
        m.add(gr, Id::Particles, "Particles", value(Id::Particles, 0, 1));
        m.add(gr, Id::Crt, "CRT Filter", value(Id::Crt, 0, 1));
        m.add(
            gr,
            Id::FireShader,
            "Shader Explosions",
            value(Id::FireShader, 0, 1),
        );
        m.add(
            gr,
            Id::Shockwaves,
            "Cinematic Effects",
            value(Id::Shockwaves, 0, 1),
        );
        m.add(
            gr,
            Id::SpriteFx,
            "Sprite Effects",
            value(Id::SpriteFx, 0, 1),
        );
        m.add(
            gr,
            Id::Lighting,
            "Dynamic Lighting",
            value(Id::Lighting, 0, 1),
        );

        m.add(
            o,
            Id::RandomPositions2,
            "Random Bomber Positions",
            value(Id::RandomPositions2, 0, 1),
        );
        m.add(
            o,
            Id::RandomMapOrder2,
            "Random Map Order",
            value(Id::RandomMapOrder2, 0, 1),
        );
        m.add(o, Id::Points2, "Points to win", value(Id::Points2, 1, 10));
        m.add(
            o,
            Id::RoundTime2,
            "Round Time",
            value(Id::RoundTime2, 5, 300),
        );

        m.add(root, Id::MapEditor, "Map Editor", Kind::Item);
        m.add(root, Id::Credits, "Show Credits", Kind::Item);
        m.add(root, Id::Help, "Help Screen", Kind::Item);
        m.add(root, Id::Quit, "Quit Game", Kind::Item);
        m
    }

    /// Vuelve a leer todos los valores de la configuración (las entradas duplicadas se mantienen
    /// iguales entre sí).
    pub fn sync(&mut self, c: &Config) {
        for item in &mut self.items {
            if let Kind::Value { min, max, value } = &mut item.kind {
                *value = item.id.get(c).clamp(*min, *max);
            }
        }
    }

    /// Lee los valores de las opciones gráficas.
    pub fn sync_video(&mut self, v: &Video) {
        for item in &mut self.items {
            if let (Kind::Value { min, max, value }, Some(x)) = (&mut item.kind, v.get(item.id)) {
                *value = x.clamp(*min, *max);
            }
        }
    }

    pub fn at_root(&self) -> bool {
        self.current == 0
    }

    /// Fila seleccionada en el submenú abierto.
    #[cfg(test)]
    fn selection(&self) -> usize {
        self.selection
    }

    /// Lo que muestra el submenú abierto: título, filas con su valor y la fila elegida.
    pub fn page(&self) -> Page {
        let cur = &self.items[self.current];
        Page {
            id: cur.id,
            depth: self.depth(),
            title: cur.text.clone(),
            rows: cur
                .children
                .iter()
                .map(|&i| {
                    let it = &self.items[i];
                    let value = match it.kind {
                        Kind::Value {
                            min: 0,
                            max: 1,
                            value,
                        } => Some(if value != 0 { "Yes" } else { "No" }.to_owned()),
                        Kind::Value { value, .. } => Some(value.to_string()),
                        Kind::Item => None,
                    };
                    (it.text.clone(), value)
                })
                .collect(),
            selection: self.selection,
        }
    }

    /// Niveles por debajo de la raíz del submenú abierto.
    fn depth(&self) -> usize {
        let mut d = 0;
        let mut i = self.current;
        while let Some(p) = self.items[i].parent {
            d += 1;
            i = p;
        }
        d
    }

    /// La caja de la raíz elegida: la fila en la raíz o, en un submenú, la rama donde está.
    pub fn root_choice(&self) -> usize {
        let mut i = self.current;
        while let Some(p) = self.items[i].parent {
            if p == 0 {
                return self.items[0]
                    .children
                    .iter()
                    .position(|&c| c == i)
                    .unwrap_or(0);
            }
            i = p;
        }
        self.selection
    }

    /// Los nombres de las entradas de la raíz (las cajas del tablero).
    pub fn root_labels(&self) -> Vec<&str> {
        self.items[0]
            .children
            .iter()
            .map(|&i| self.items[i].text.as_str())
            .collect()
    }

    /// Vuelve a la raíz con la primera fila seleccionada.
    #[cfg(test)]
    fn reset(&mut self) {
        self.current = 0;
        self.selection = 0;
    }

    pub fn key(&mut self, code: KeyCode) -> Outcome {
        // En el tablero de la raíz las cajas van en fila: izquierda/derecha también mueven.
        let code = match code {
            KeyCode::ArrowLeft if self.at_root() => KeyCode::ArrowUp,
            KeyCode::ArrowRight if self.at_root() => KeyCode::ArrowDown,
            c => c,
        };
        let n = self.items[self.current].children.len();
        let sel = self.items[self.current]
            .children
            .get(self.selection)
            .copied();
        match code {
            KeyCode::ArrowDown => self.selection = (self.selection + 1) % n,
            KeyCode::ArrowUp => self.selection = (self.selection + n - 1) % n,
            KeyCode::ArrowRight | KeyCode::ArrowLeft => {
                let dir = if code == KeyCode::ArrowRight { 1 } else { -1 };
                if let Some(i) = sel {
                    let id = self.items[i].id;
                    // El tiempo de ronda avanza de 5 en 5 (`CONFIG_ROUND_TIME` del original).
                    let step = if matches!(id, Id::RoundTime | Id::RoundTime2) {
                        5 * dir
                    } else {
                        dir
                    };
                    if let Kind::Value { min, max, value } = &mut self.items[i].kind {
                        *value = (*value + step).clamp(*min, *max);
                        return Outcome::Changed(id, *value);
                    }
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                if let Some(i) = sel {
                    if !self.items[i].children.is_empty() {
                        self.current = i;
                        self.selection = 0;
                    } else if matches!(self.items[i].kind, Kind::Item) {
                        return Outcome::Activate(self.items[i].id);
                    }
                }
            }
            KeyCode::Escape => {
                // Al subir queda seleccionada la entrada de la que se venía.
                if let Some(parent) = self.items[self.current].parent {
                    let from = self.current;
                    self.current = parent;
                    self.selection = self.items[parent]
                        .children
                        .iter()
                        .position(|&c| c == from)
                        .unwrap_or(0);
                }
            }
            _ => {}
        }
        Outcome::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_enter_back_and_wrap() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        // Raíz: New Game, Options, Map Editor, Credits, Help, Quit Game. Subir desde la primera da la vuelta a "Quit Game".
        m.key(KeyCode::ArrowUp);
        assert_eq!(m.key(KeyCode::Enter), Outcome::Activate(Id::Quit));
        m.reset();
        assert_eq!(m.key(KeyCode::Enter), Outcome::None, "entra en New Game");
        assert_eq!(m.key(KeyCode::Enter), Outcome::Activate(Id::StartLocal));
        m.key(KeyCode::Escape);
        assert_eq!(
            m.key(KeyCode::Enter),
            Outcome::None,
            "vuelve a entrar en New Game"
        );
    }

    #[test]
    fn left_and_right_walk_the_root_boxes() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        m.key(KeyCode::ArrowRight);
        m.key(KeyCode::ArrowRight);
        assert_eq!(m.selection(), 2);
        m.key(KeyCode::ArrowLeft);
        assert_eq!(m.selection(), 1);
        assert_eq!(m.root_labels()[1], "Options");
        m.key(KeyCode::Enter); // Options
        m.key(KeyCode::ArrowDown);
        m.key(KeyCode::Enter); // Enable/Disable Extras
        assert_eq!(m.root_choice(), 1, "sigue en la rama de Options");
    }

    #[test]
    fn page_shows_values_as_text() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        m.key(KeyCode::Enter); // New Game
        let p = m.page();
        assert_eq!(
            (p.id, p.depth, p.title.as_str()),
            (Id::NewGame, 1, "New Game")
        );
        assert_eq!(p.rows[0], ("Start Local".to_owned(), None));
        let yes_no = if c.random_positions { "Yes" } else { "No" };
        assert_eq!(
            p.rows[3].1.as_deref(),
            Some(yes_no),
            "Random Bomber Positions"
        );
        assert_eq!(p.rows[6].1.as_deref(), Some("90"), "Round Time");
    }

    #[test]
    fn escape_selects_the_submenu_it_came_from() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        m.key(KeyCode::ArrowRight);
        m.key(KeyCode::Enter); // Options
        m.key(KeyCode::ArrowDown);
        m.key(KeyCode::Enter); // Enable/Disable Extras
        m.key(KeyCode::Escape);
        assert_eq!(m.selection(), 1, "Enable/Disable Extras");
        m.key(KeyCode::Escape);
        assert!(m.at_root());
        assert_eq!(m.selection(), 1, "Options");
    }

    #[test]
    fn values_clamp_and_report_changes() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        m.key(KeyCode::Enter); // New Game
        for _ in 0..5 {
            m.key(KeyCode::ArrowDown); // hasta Points to win
        }
        for _ in 0..20 {
            m.key(KeyCode::ArrowRight);
        }
        assert_eq!(
            m.key(KeyCode::ArrowRight),
            Outcome::Changed(Id::Points, 10),
            "tope en 10"
        );
        for _ in 0..20 {
            m.key(KeyCode::ArrowLeft);
        }
        assert_eq!(
            m.key(KeyCode::ArrowLeft),
            Outcome::Changed(Id::Points, 1),
            "suelo en 1"
        );
    }

    #[test]
    fn round_time_moves_in_steps_of_five() {
        let c = Config::default();
        let mut m = Menu::new(&c);
        m.key(KeyCode::Enter); // New Game
        for _ in 0..6 {
            m.key(KeyCode::ArrowDown); // hasta Round Time
        }
        assert_eq!(
            m.key(KeyCode::ArrowRight),
            Outcome::Changed(Id::RoundTime, 95)
        );
        assert_eq!(
            m.key(KeyCode::ArrowLeft),
            Outcome::Changed(Id::RoundTime, 90)
        );
        assert_eq!(
            m.key(KeyCode::ArrowLeft),
            Outcome::Changed(Id::RoundTime, 85)
        );
    }

    #[test]
    fn duplicated_entries_follow_the_config() {
        let mut c = Config::default();
        let mut m = Menu::new(&c);
        c.round_time = 120;
        m.sync(&c);
        for id in [Id::RoundTime, Id::RoundTime2] {
            let i = m.index_of(id);
            assert!(matches!(m.items[i].kind, Kind::Value { value: 120, .. }));
        }
    }

    #[test]
    fn ids_roundtrip_through_the_config() {
        let mut c = Config::default();
        Id::BombSpeed.set(&mut c, 200);
        Id::Koks.set(&mut c, 0);
        assert_eq!(
            (
                Id::BombSpeed.get(&c),
                Id::Koks.get(&c),
                c.bomb_speed,
                c.koks
            ),
            (200, 0, 200, false)
        );
    }
}
