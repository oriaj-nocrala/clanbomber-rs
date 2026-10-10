//! Formato `.map` de ClanBomber: línea 1 autor, línea 2 máx. jugadores,
//! luego 13 filas de 17 caracteres.

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

#[derive(Clone, Debug)]
pub struct Map {
    pub author: String,
    pub max_players: u8,
    tiles: [[Tile; MAP_HEIGHT]; MAP_WIDTH],
    /// Posiciones de salida por jugador (dígitos `0`-`7` en el mapa).
    pub spawns: Vec<(u8, usize, usize)>,
    /// Casillas `R`: caja con probabilidad 2/3 (se resuelve al iniciar la partida).
    pub random_boxes: Vec<(usize, usize)>,
}

impl Map {
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, MapError> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    pub fn parse(src: &str) -> Result<Self, MapError> {
        // Replica la lectura del original: autor = primera línea; el entero
        // salta espacios en blanco; después los caracteres ignoran solo '\n'.
        let (author, rest) = src
            .split_once('\n')
            .ok_or(MapError::Format("falta autor"))?;
        let author = author.trim_end_matches('\r').to_owned();
        let rest = rest.trim_start();
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        let max_players = rest[..digits]
            .parse()
            .map_err(|_| MapError::Format("máx. jugadores inválido"))?;
        let mut chars = rest[digits..].chars().filter(|&c| c != '\n');

        let mut map = Map {
            author,
            max_players,
            tiles: [[Tile::None; MAP_HEIGHT]; MAP_WIDTH],
            spawns: Vec::new(),
            random_boxes: Vec::new(),
        };

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let c = chars.next().ok_or(MapError::Format("mapa incompleto"))?;
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
        Ok(map)
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

    pub fn set(&mut self, x: usize, y: usize, t: Tile) {
        self.tiles[x][y] = t;
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
