//! Mapa editable tal como está en disco: autor, jugadores y la cuadrícula de caracteres
//! (ClanBomber2: MapEntry.cpp). El editor trabaja sobre esto y [`Map`] se deriva de ello.

use crate::map::{Map, MapError, MAP_HEIGHT, MAP_WIDTH};

/// Mapa nuevo del editor (`map_template`): marco de muros y cuatro salidas en las esquinas.
const TEMPLATE: [&str; MAP_HEIGHT] = [
    "*****************",
    "*0             2*",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*               *",
    "*3             1*",
    "*****************",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDoc {
    pub author: String,
    pub max_players: u8,
    /// `data[x][y]`: `' '` suelo, `*` muro, `+` caja, `R` caja al azar, `S` hielo, `<>^v`
    /// flechas, `o` trampa, `0`-`7` salidas y cualquier otro carácter (`-`) un hueco.
    pub data: [[char; MAP_HEIGHT]; MAP_WIDTH],
}

impl MapDoc {
    /// `MapEntry(name, write)`: autor "anonymous coward" y 4 jugadores.
    pub fn template() -> Self {
        let mut data = [[' '; MAP_HEIGHT]; MAP_WIDTH];
        for (y, row) in TEMPLATE.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                data[x][y] = c;
            }
        }
        MapDoc {
            author: "anonymous coward".into(),
            max_players: 4,
            data,
        }
    }

    /// Lee el formato `.map`: línea de autor, entero de jugadores y 13 filas de 17 caracteres
    /// (el entero salta espacios en blanco y los caracteres solo ignoran `\n`, como el original).
    pub fn parse(src: &str) -> Result<Self, MapError> {
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

        let mut data = [[' '; MAP_HEIGHT]; MAP_WIDTH];
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                data[x][y] = chars.next().ok_or(MapError::Format("mapa incompleto"))?;
            }
        }
        Ok(MapDoc {
            author,
            max_players,
            data,
        })
    }

    /// `MapEntry::write_back`: autor, jugadores y una fila por línea.
    pub fn to_text(&self) -> String {
        let mut s = format!("{}\n{}\n", self.author, self.max_players);
        for y in 0..MAP_HEIGHT {
            s.extend((0..MAP_WIDTH).map(|x| self.data[x][y]));
            s.push('\n');
        }
        s
    }

    pub fn set(&mut self, x: usize, y: usize, c: char) {
        self.data[x][y] = c;
    }

    /// `MapEntry::set_bomber_pos`: la salida `number` (0-7) pasa a esta casilla y su sitio
    /// anterior queda como suelo.
    pub fn set_bomber_pos(&mut self, x: usize, y: usize, number: u8) {
        let digit = (b'0' + number) as char;
        for col in self.data.iter_mut() {
            for c in col.iter_mut() {
                if *c == digit {
                    *c = ' ';
                }
            }
        }
        self.data[x][y] = digit;
    }

    /// Posición de la salida `number`, si está en el mapa.
    pub fn bomber_pos(&self, number: u8) -> Option<(usize, usize)> {
        let digit = (b'0' + number) as char;
        (0..MAP_WIDTH)
            .flat_map(|x| (0..MAP_HEIGHT).map(move |y| (x, y)))
            .find(|&(x, y)| self.data[x][y] == digit)
    }

    /// `MapEntry::set_max_players`: solo acepta de 2 a 8.
    pub fn set_max_players(&mut self, n: i32) {
        if (2..=8).contains(&n) {
            self.max_players = n as u8;
        }
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, MapError> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    /// Escribe el mapa, creando la carpeta si hace falta.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }

    pub fn to_map(&self) -> Map {
        Map::from_doc(self)
    }
}

/// Nombre de archivo seguro (sin separadores de ruta ni espacios en los bordes) para un mapa nuevo.
pub fn map_file_name(name: &str) -> Option<String> {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | '\0') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim_matches('.').to_owned();
    (!clean.is_empty()).then(|| format!("{clean}.map"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_map_roundtrips_through_text() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/maps");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "map") {
                let doc = MapDoc::parse(&std::fs::read_to_string(&p).unwrap()).unwrap();
                assert_eq!(MapDoc::parse(&doc.to_text()).unwrap(), doc, "{p:?}");
                n += 1;
            }
        }
        assert_eq!(n, 33);
    }

    #[test]
    fn template_is_a_valid_four_player_map() {
        let doc = MapDoc::template();
        let map = MapDoc::parse(&doc.to_text()).unwrap().to_map();
        assert_eq!(
            (map.max_players, map.author.as_str()),
            (4, "anonymous coward")
        );
        assert_eq!(map.spawns.len(), 4);
        assert_eq!(doc.bomber_pos(1), Some((15, 11)));
    }

    #[test]
    fn moving_a_start_position_clears_the_old_one() {
        let mut doc = MapDoc::template();
        doc.set_bomber_pos(8, 6, 0);
        assert_eq!(doc.bomber_pos(0), Some((8, 6)));
        assert_eq!(doc.data[1][1], ' ');
    }

    #[test]
    fn player_count_is_limited_to_two_through_eight() {
        let mut doc = MapDoc::template();
        doc.set_max_players(9);
        assert_eq!(doc.max_players, 4);
        doc.set_max_players(1);
        assert_eq!(doc.max_players, 4);
        doc.set_max_players(8);
        assert_eq!(doc.max_players, 8);
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(map_file_name("  My Map ").as_deref(), Some("My Map.map"));
        assert_eq!(
            map_file_name("../../etc/passwd").as_deref(),
            Some("_.._etc_passwd.map")
        );
        assert_eq!(map_file_name("   "), None);
        assert_eq!(map_file_name("..."), None);
    }

    #[test]
    fn save_and_load_through_a_file() {
        let dir = std::env::temp_dir().join(format!("cb-mapdoc-test-{}", std::process::id()));
        let path = dir.join("nested").join("x.map");
        let mut doc = MapDoc::template();
        doc.set(3, 3, '+');
        doc.save(&path).unwrap();
        assert_eq!(MapDoc::load(&path).unwrap(), doc);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn edits_show_up_in_the_derived_map() {
        let mut doc = MapDoc::template();
        doc.set(5, 5, 'S');
        doc.set(6, 5, '>');
        let map = doc.to_map();
        assert_eq!(map.get(5, 5), crate::Tile::Ice);
        assert_eq!(map.get(6, 5), crate::Tile::Arrow(crate::Dir::Right));
    }
}
