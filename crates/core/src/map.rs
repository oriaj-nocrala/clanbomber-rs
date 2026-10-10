//! Formato `.map` de ClanBomber: línea 1 autor, línea 2 máx. jugadores,
//! luego 13 filas de 17 caracteres.

use crate::mapdoc::MapDoc;
use crate::rng::Rng;

pub const MAP_WIDTH: usize = 17;
pub const MAP_HEIGHT: usize = 13;
pub const TILE_PIXELS: u32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    /// Orden del enum `Direction` del original: DOWN=0, LEFT=1, UP=2, RIGHT=3.
    pub fn index(self) -> u32 {
        match self {
            Dir::Down => 0,
            Dir::Left => 1,
            Dir::Up => 2,
            Dir::Right => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    /// Fuera del mapa (`-` y cualquier carácter desconocido).
    None,
    Ground,
    Wall,
    Box,
    Ice,
    Arrow(Dir),
    Trap,
}

#[derive(Debug)]
pub enum MapError {
    Format(&'static str),
    Io(std::io::Error),
}

impl From<std::io::Error> for MapError {
    fn from(e: std::io::Error) -> Self {
        MapError::Io(e)
    }
}

/// Caja alcanzada por una explosión: sigue bloqueando 0.5 s y luego pasa a suelo
/// (MapTile_Box::destroy / MapTile::act en el original).
#[derive(Clone, Copy, Debug)]
pub struct Crumble {
    pub x: usize,
    pub y: usize,
    pub left: f32,
}

/// Casilla que el observador está haciendo desaparecer al acabarse el tiempo: se encoge 1 s
/// y queda sin suelo (`MapTile::vanish`, `next = NONE`).
#[derive(Clone, Copy, Debug)]
pub struct Vanish {
    pub x: usize,
    pub y: usize,
    pub left: f32,
}

#[derive(Clone, Debug)]
pub struct Map {
    /// Nombre del archivo sin extensión (`MapEntry::get_name`).
    pub name: String,
    pub author: String,
    pub max_players: u8,
    tiles: [[Tile; MAP_HEIGHT]; MAP_WIDTH],
    /// Posiciones de salida por jugador (dígitos `0`-`7` en el mapa).
    pub spawns: Vec<(u8, usize, usize)>,
    /// Casillas `R`: caja con probabilidad 2/3 (se resuelve al iniciar la partida).
    pub random_boxes: Vec<(usize, usize)>,
    pub crumbling: Vec<Crumble>,
    pub vanishing: Vec<Vanish>,
}

impl Map {
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, MapError> {
        let path = path.as_ref();
        let mut map = Self::parse(&std::fs::read_to_string(path)?)?;
        map.name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(map)
    }

    pub fn parse(src: &str) -> Result<Self, MapError> {
        Ok(Self::from_doc(&MapDoc::parse(src)?))
    }

    /// Construye el mapa jugable a partir de su descripción en caracteres.
    pub fn from_doc(doc: &MapDoc) -> Self {
        let mut map = Map {
            name: String::new(),
            author: doc.author.clone(),
            max_players: doc.max_players,
            tiles: [[Tile::None; MAP_HEIGHT]; MAP_WIDTH],
            spawns: Vec::new(),
            random_boxes: Vec::new(),
            crumbling: Vec::new(),
            vanishing: Vec::new(),
        };

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let c = doc.data[x][y];
                map.tiles[x][y] = match c {
                    ' ' => Tile::Ground,
                    '0'..='7' => {
                        map.spawns.push((c as u8 - b'0', x, y));
                        Tile::Ground
                    }
                    '*' => Tile::Wall,
                    '+' => Tile::Box,
                    'R' => {
                        map.random_boxes.push((x, y));
                        Tile::Ground
                    }
                    'S' => Tile::Ice,
                    '<' => Tile::Arrow(Dir::Left),
                    '>' => Tile::Arrow(Dir::Right),
                    '^' => Tile::Arrow(Dir::Up),
                    'v' => Tile::Arrow(Dir::Down),
                    'o' => Tile::Trap,
                    _ => Tile::None,
                };
            }
        }
        map.spawns.sort_unstable();
        map
    }

    /// Fuera de rango devuelve `Tile::None`, como el `dummy_maptile` del original.
    pub fn get(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x as usize >= MAP_WIDTH || y as usize >= MAP_HEIGHT {
            Tile::None
        } else {
            self.tiles[x as usize][y as usize]
        }
    }

    /// ¿Bloquea el paso el tile en coordenadas de tile? Fuera del mapa se comporta como el
    /// `dummy_maptile` del original (un muro). `Tile::None` dentro del mapa no bloquea,
    /// igual que `MapTile_None` (ClanBomber2: MapTile_None.cpp).
    pub fn blocking_tile(&self, tx: i32, ty: i32) -> bool {
        if tx < 0 || ty < 0 || tx as usize >= MAP_WIDTH || ty as usize >= MAP_HEIGHT {
            return true;
        }
        matches!(self.tiles[tx as usize][ty as usize], Tile::Wall | Tile::Box)
    }

    /// Igual que [`blocking_tile`] pero en píxeles (`Map::get_maptile_xy` del original):
    /// los píxeles negativos caen fuera del mapa.
    pub fn blocking_px(&self, px: i32, py: i32) -> bool {
        if px < 0 || py < 0 {
            return true;
        }
        let t = TILE_PIXELS as i32;
        self.blocking_tile(px / t, py / t)
    }

    /// ¿Es un muro (o el borde)? `get_maptile_xy(..)->get_type() == WALL` del original; el
    /// `dummy_maptile` de fuera del mapa es un muro. Las cajas no cuentan.
    pub fn is_wall_px(&self, px: i32, py: i32) -> bool {
        if px < 0 || py < 0 {
            return true;
        }
        let t = TILE_PIXELS as i32;
        let (tx, ty) = (px / t, py / t);
        if tx as usize >= MAP_WIDTH || ty as usize >= MAP_HEIGHT {
            return true;
        }
        self.tiles[tx as usize][ty as usize] == Tile::Wall
    }

    /// `MapTile::passable`: suelo normal donde puede caer un extra (`Map::get_passable`).
    pub fn is_passable(&self, tx: i32, ty: i32) -> bool {
        !self.is_vanishing(tx, ty)
            && matches!(
                self.get(tx, ty),
                Tile::Ground | Tile::Ice | Tile::Arrow(_) | Tile::Trap
            )
    }

    /// `MapTile_None`: dentro del mapa, sin suelo (los objetos caen).
    pub fn is_hole(&self, tx: i32, ty: i32) -> bool {
        tx >= 0
            && ty >= 0
            && (tx as usize) < MAP_WIDTH
            && (ty as usize) < MAP_HEIGHT
            && self.tiles[tx as usize][ty as usize] == Tile::None
    }

    /// `MapTile::destroy()`: solo las cajas reaccionan; fuera del mapa no hace nada.
    pub fn destroy(&mut self, tx: i32, ty: i32) {
        if self.get(tx, ty) == Tile::Box && !self.is_crumbling(tx, ty) {
            self.crumbling.push(Crumble {
                x: tx as usize,
                y: ty as usize,
                left: 0.5,
            });
        }
    }

    pub fn is_crumbling(&self, tx: i32, ty: i32) -> bool {
        self.crumbling
            .iter()
            .any(|c| c.x as i32 == tx && c.y as i32 == ty)
    }

    /// `Map::act()`: avanza las cajas que se desmoronan. Devuelve las casillas que acaban
    /// de pasar a suelo, donde el original llama a `spawn_extra()`.
    pub fn act(&mut self, dt: f32) -> Vec<(usize, usize)> {
        for c in &mut self.crumbling {
            c.left -= dt;
        }
        let done: Vec<Crumble> = self
            .crumbling
            .iter()
            .copied()
            .filter(|c| c.left <= 0.0)
            .collect();
        self.crumbling.retain(|c| c.left > 0.0);
        for c in &done {
            self.tiles[c.x][c.y] = Tile::Ground;
        }
        for v in &mut self.vanishing {
            v.left -= dt;
        }
        for v in self.vanishing.iter().filter(|v| v.left <= 0.0) {
            self.tiles[v.x][v.y] = Tile::None;
        }
        self.vanishing.retain(|v| v.left > 0.0);
        done.iter().map(|c| (c.x, c.y)).collect()
    }

    pub fn is_vanishing(&self, tx: i32, ty: i32) -> bool {
        self.vanishing
            .iter()
            .any(|v| v.x as i32 == tx && v.y as i32 == ty)
    }

    /// Resuelve las casillas `R`: caja con probabilidad 2/3 (`Map::reload`).
    pub fn resolve_random_boxes(&mut self, rng: &mut Rng) {
        for (x, y) in std::mem::take(&mut self.random_boxes) {
            self.tiles[x][y] = if rng.below(3) != 0 {
                Tile::Box
            } else {
                Tile::Ground
            };
        }
    }

    /// `Map::get_random`: casilla cualquiera que no sea un hueco ni se esté desvaneciendo.
    pub fn get_random(&self, rng: &mut Rng) -> Option<(usize, usize)> {
        for _ in 0..1000 {
            let x = rng.below(MAP_WIDTH as u32) as usize;
            let y = rng.below(MAP_HEIGHT as u32) as usize;
            if self.tiles[x][y] != Tile::None && !self.is_vanishing(x as i32, y as i32) {
                return Some((x, y));
            }
        }
        None
    }

    /// `MapTile::vanish`: las cajas se rompen; los huecos no hacen nada; el resto se encoge.
    /// Devuelve `true` si la casilla empieza a desvanecerse (los extras encima deben huir).
    pub fn vanish(&mut self, tx: i32, ty: i32) -> bool {
        match self.get(tx, ty) {
            Tile::Box => {
                self.destroy(tx, ty);
                false
            }
            Tile::None => false,
            _ if self.is_vanishing(tx, ty) => false,
            _ => {
                self.vanishing.push(Vanish {
                    x: tx as usize,
                    y: ty as usize,
                    left: 1.0,
                });
                true
            }
        }
    }

    pub fn set(&mut self, x: usize, y: usize, t: Tile) {
        self.tiles[x][y] = t;
    }
}

/// Lista de mapas disponibles, ordenada por nombre sin distinguir mayúsculas
/// (`Map::enum_maps` + `sort_maps`), con el cursor de `spin_up`.
#[derive(Clone, Debug)]
pub struct MapList {
    pub paths: Vec<std::path::PathBuf>,
    pub index: usize,
    /// Mapas desmarcados: se saltan al pasar de ronda (`MapEntry::is_enabled`).
    pub disabled: Vec<String>,
    /// `true` si el archivo se puede editar (los mapas propios del usuario; los que trae el
    /// juego son de solo lectura). Mismo orden que `paths`.
    pub writable: Vec<bool>,
}

impl MapList {
    /// Mapas de una sola carpeta (todos de solo lectura).
    pub fn scan(dir: impl AsRef<std::path::Path>) -> Result<Self, MapError> {
        Self::scan_dirs(&[(dir.as_ref(), false)])
    }

    /// Mapas de varias carpetas, cada una marcada como editable o no (`enum_maps`: la de datos
    /// del juego y la local del usuario). Las carpetas que no existen se ignoran.
    pub fn scan_dirs(dirs: &[(&std::path::Path, bool)]) -> Result<Self, MapError> {
        let mut found: Vec<(std::path::PathBuf, bool)> = Vec::new();
        for &(dir, writable) in dirs {
            let Ok(read) = std::fs::read_dir(dir) else {
                continue;
            };
            for e in read.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "map") {
                    found.push((p, writable));
                }
            }
        }
        found.sort_by_key(|(p, _)| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()));
        let (paths, writable) = found.into_iter().unzip();
        Ok(MapList {
            paths,
            index: 0,
            disabled: Vec::new(),
            writable,
        })
    }

    pub fn current(&self) -> Result<Map, MapError> {
        Map::load(&self.paths[self.index])
    }

    /// `Map::load_random_valid`: salta a un mapa al azar donde caben `players` jugadores.
    pub fn random_for(&mut self, players: usize, rng: &mut Rng) -> Result<Map, MapError> {
        let valid: Vec<usize> = (0..self.paths.len())
            .filter(|&i| {
                Map::load(&self.paths[i]).is_ok_and(|m| {
                    m.max_players as usize >= players && !self.disabled.contains(&m.name)
                })
            })
            .collect();
        if let Some(&i) = valid.get(rng.below(valid.len().max(1) as u32) as usize) {
            self.index = i;
        }
        self.current()
    }

    /// `Map::load_next_valid`: avanza (con vuelta al principio) hasta un mapa donde caben
    /// `players` jugadores. Si ninguno sirve se queda en el actual.
    pub fn next_for(&mut self, players: usize) -> Result<Map, MapError> {
        for _ in 0..self.paths.len() {
            self.index = (self.index + 1) % self.paths.len();
            let map = self.current()?;
            if map.max_players as usize >= players && !self.disabled.contains(&map.name) {
                return Ok(map);
            }
        }
        self.current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena() {
        let m = Map::load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/maps/Arena.map"
        ))
        .unwrap();
        assert_eq!(m.author, "Denis Oliver Kropp");
        assert_eq!(m.max_players, 5);
        assert_eq!(m.spawns.len(), 5);
        assert_eq!(m.get(0, 0), Tile::None);
        assert_eq!(m.get(4, 0), Tile::Wall);
        assert_eq!(m.get(-1, 3), Tile::None);
    }

    #[test]
    fn all_maps_parse_with_enough_spawns() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/maps");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "map") {
                let m = Map::load(&p).unwrap_or_else(|e| panic!("{p:?}: {e:?}"));
                assert!(m.spawns.len() >= m.max_players as usize, "{p:?}");
                n += 1;
            }
        }
        assert_eq!(n, 33);
    }
}
