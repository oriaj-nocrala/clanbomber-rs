//! Opciones de partida (`Config` de ClanBomber2: GameConfig.cpp) y su persistencia.
//!
//! El formato en disco es propio (`clave=valor` por línea); no lee el `clanbomber.cfg`
//! posicional del original.

use crate::extra::ExtraConfig;

pub const MAX_PLAYERS: usize = 8;
/// Número de aspectos de bomber (`NR_BOMBERSKINS`).
pub const NR_SKINS: u8 = 8;

/// `Controller::CONTROLLER_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerKind {
    Ai,
    /// Teclado 1 (flechas + Enter), 2 (WASD + Tab) o 3 (IJKL + Espacio).
    Keymap(u8),
    RcMouse,
    Joystick(u8),
}

impl ControllerKind {
    fn to_str(self) -> String {
        match self {
            ControllerKind::Ai => "ai".into(),
            ControllerKind::Keymap(n) => format!("keymap{n}"),
            ControllerKind::RcMouse => "rcmouse".into(),
            ControllerKind::Joystick(n) => format!("joystick{n}"),
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "ai" => ControllerKind::Ai,
            // La segunda IA del original ("AI_mass") no se portó: las configuraciones que la
            // nombran pasan a la IA que tenemos.
            "ai_mass" => ControllerKind::Ai,
            "rcmouse" => ControllerKind::RcMouse,
            _ => {
                if let Some(n) = s.strip_prefix("keymap") {
                    ControllerKind::Keymap(n.parse().ok().filter(|n| (1..=3).contains(n))?)
                } else if let Some(n) = s.strip_prefix("joystick") {
                    ControllerKind::Joystick(n.parse().ok()?)
                } else {
                    return None;
                }
            }
        })
    }
}

/// `BomberConfig`: un puesto de la lista de jugadores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BomberConfig {
    pub skin: u8,
    pub name: String,
    /// 0 = sin equipo, 1..=4.
    pub team: u8,
    pub controller: ControllerKind,
    pub enabled: bool,
    /// Resalta su casilla de salida en el mapa.
    pub highlight_maptile: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub max_bombs: i32,
    pub max_power: i32,
    pub max_skateboards: i32,
    pub start_bombs: i32,
    pub start_power: i32,
    pub start_skateboards: i32,
    pub start_kick: bool,
    pub start_glove: bool,
    pub start_map: i32,
    pub points_to_win: i32,
    /// Segundos por ronda.
    pub round_time: i32,
    pub theme: i32,
    pub music: bool,
    pub kids_mode: bool,
    pub corpse_parts: i32,
    pub shaky_explosions: bool,
    pub random_positions: bool,
    pub random_map_order: bool,
    // Extras habilitados.
    pub bombs: bool,
    pub power: bool,
    pub skateboards: bool,
    pub kick: bool,
    pub glove: bool,
    pub joint: bool,
    pub viagra: bool,
    pub koks: bool,
    /// Décimas de segundo hasta explotar.
    pub bomb_countdown: i32,
    /// Centésimas de segundo de retardo en la reacción en cadena.
    pub bomb_delay: i32,
    /// Píxeles por segundo de una bomba pateada.
    pub bomb_speed: i32,
    /// Nombres de los mapas que el jugador ha desmarcado en la selección de mapas.
    pub disabled_maps: Vec<String>,
    pub bombers: Vec<BomberConfig>,
}

impl Default for Config {
    /// Valores iniciales de `Config` en el original. Los nombres son los de `Config::save(true)`.
    /// Con la IA aún sin portar, solo los dos primeros puestos (teclados 1 y 2) vienen activos.
    fn default() -> Self {
        const NAMES: [&str; MAX_PLAYERS] = [
            "Are", "You", "Still", "Watching", "AIs", "Playing", "For", "You",
        ];
        Config {
            max_bombs: 9,
            max_power: 12,
            max_skateboards: 5,
            start_bombs: 1,
            start_power: 1,
            start_skateboards: 0,
            start_kick: false,
            start_glove: false,
            start_map: 0,
            points_to_win: 5,
            round_time: 90,
            theme: 0,
            music: false,
            kids_mode: false,
            corpse_parts: 10,
            shaky_explosions: true,
            random_positions: true,
            random_map_order: false,
            bombs: true,
            power: true,
            skateboards: true,
            kick: true,
            glove: true,
            joint: true,
            viagra: true,
            koks: true,
            bomb_countdown: 25,
            bomb_delay: 10,
            bomb_speed: 160,
            disabled_maps: Vec::new(),
            bombers: (0..MAX_PLAYERS)
                .map(|i| BomberConfig {
                    skin: i as u8,
                    name: NAMES[i].to_owned(),
                    team: 0,
                    controller: match i {
                        0 => ControllerKind::Keymap(1),
                        1 => ControllerKind::Keymap(2),
                        2 => ControllerKind::Keymap(3),
                        _ => ControllerKind::Ai,
                    },
                    enabled: i < 2,
                    highlight_maptile: false,
                })
                .collect(),
        }
    }
}

impl Config {
    /// Qué extras pueden caer de las cajas. Patada y guante no caen si ya se empieza con ellos
    /// (`MapTile::spawn_extra`).
    pub fn extra_config(&self) -> ExtraConfig {
        ExtraConfig {
            power: self.power,
            bombs: self.bombs,
            skateboards: self.skateboards,
            kick: self.kick && !self.start_kick,
            glove: self.glove && !self.start_glove,
            joint: self.joint,
            viagra: self.viagra,
            koks: self.koks,
        }
    }

    pub fn number_of_players(&self) -> usize {
        self.bombers.iter().filter(|b| b.enabled).count()
    }

    /// `Config::get_number_of_opponents`: rivales distintos (un equipo cuenta como uno).
    pub fn number_of_opponents(&self) -> usize {
        let mut teams = [false; 4];
        let mut n = 0;
        for b in self.bombers.iter().filter(|b| b.enabled) {
            match b.team {
                0 => n += 1,
                t => teams[(t as usize - 1).min(3)] = true,
            }
        }
        n + teams.iter().filter(|&&t| t).count()
    }

    /// Los puestos activos, en orden.
    pub fn enabled_bombers(&self) -> impl Iterator<Item = &BomberConfig> {
        self.bombers.iter().filter(|b| b.enabled)
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let mut put = |k: &str, v: String| {
            s.push_str(k);
            s.push('=');
            s.push_str(&v);
            s.push('\n');
        };
        let b = |v: bool| (v as i32).to_string();
        put("version", "1".into());
        put("max_bombs", self.max_bombs.to_string());
        put("max_power", self.max_power.to_string());
        put("max_skateboards", self.max_skateboards.to_string());
        put("start_bombs", self.start_bombs.to_string());
        put("start_power", self.start_power.to_string());
        put("start_skateboards", self.start_skateboards.to_string());
        put("start_kick", b(self.start_kick));
        put("start_glove", b(self.start_glove));
        put("start_map", self.start_map.to_string());
        put("points_to_win", self.points_to_win.to_string());
        put("round_time", self.round_time.to_string());
        put("theme", self.theme.to_string());
        put("music", b(self.music));
        put("kids_mode", b(self.kids_mode));
        put("corpse_parts", self.corpse_parts.to_string());
        put("shaky_explosions", b(self.shaky_explosions));
        put("random_positions", b(self.random_positions));
        put("random_map_order", b(self.random_map_order));
        put("bombs", b(self.bombs));
        put("power", b(self.power));
        put("skateboards", b(self.skateboards));
        put("kick", b(self.kick));
        put("glove", b(self.glove));
        put("joint", b(self.joint));
        put("viagra", b(self.viagra));
        put("koks", b(self.koks));
        put("bomb_countdown", self.bomb_countdown.to_string());
        put("bomb_delay", self.bomb_delay.to_string());
        put("bomb_speed", self.bomb_speed.to_string());
        for name in &self.disabled_maps {
            put("disabled_map", name.clone());
        }
        for (i, p) in self.bombers.iter().enumerate() {
            put(&format!("bomber{i}.skin"), p.skin.to_string());
            put(&format!("bomber{i}.name"), p.name.clone());
            put(&format!("bomber{i}.team"), p.team.to_string());
            put(&format!("bomber{i}.controller"), p.controller.to_str());
            put(&format!("bomber{i}.enabled"), b(p.enabled));
            put(&format!("bomber{i}.highlight"), b(p.highlight_maptile));
        }
        s
    }

    /// Lee `clave=valor`; lo desconocido o inválido se ignora y queda el valor por defecto.
    pub fn from_text(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let (k, v) = (k.trim(), v.trim());
            let int = || v.parse::<i32>().ok();
            let flag = || v.parse::<i32>().ok().map(|n| n != 0);
            macro_rules! set {
                ($field:ident, $val:expr) => {
                    if let Some(x) = $val {
                        c.$field = x;
                    }
                };
            }
            match k {
                "max_bombs" => set!(max_bombs, int()),
                "max_power" => set!(max_power, int()),
                "max_skateboards" => set!(max_skateboards, int()),
                "start_bombs" => set!(start_bombs, int()),
                "start_power" => set!(start_power, int()),
                "start_skateboards" => set!(start_skateboards, int()),
                "start_kick" => set!(start_kick, flag()),
                "start_glove" => set!(start_glove, flag()),
                "start_map" => set!(start_map, int()),
                "points_to_win" => set!(points_to_win, int()),
                "round_time" => set!(round_time, int()),
                "theme" => set!(theme, int()),
                "music" => set!(music, flag()),
                "kids_mode" => set!(kids_mode, flag()),
                "corpse_parts" => set!(corpse_parts, int()),
                "shaky_explosions" => set!(shaky_explosions, flag()),
                "random_positions" => set!(random_positions, flag()),
                "random_map_order" => set!(random_map_order, flag()),
                "bombs" => set!(bombs, flag()),
                "power" => set!(power, flag()),
                "skateboards" => set!(skateboards, flag()),
                "kick" => set!(kick, flag()),
                "glove" => set!(glove, flag()),
                "joint" => set!(joint, flag()),
                "viagra" => set!(viagra, flag()),
                "koks" => set!(koks, flag()),
                "bomb_countdown" => set!(bomb_countdown, int()),
                "bomb_delay" => set!(bomb_delay, int()),
                "bomb_speed" => set!(bomb_speed, int()),
                "disabled_map" if !v.is_empty() => c.disabled_maps.push(v.to_owned()),
                _ => {
                    // bomber{i}.{campo}
                    let Some(rest) = k.strip_prefix("bomber") else {
                        continue;
                    };
                    let Some((idx, field)) = rest.split_once('.') else {
                        continue;
                    };
                    let Some(p) = idx.parse::<usize>().ok().and_then(|i| c.bombers.get_mut(i))
                    else {
                        continue;
                    };
                    match field {
                        "skin" => {
                            if let Some(n) = int() {
                                p.skin = (n.rem_euclid(NR_SKINS as i32)) as u8;
                            }
                        }
                        "name" if !v.is_empty() => p.name = v.to_owned(),
                        "team" => {
                            if let Some(n) = int() {
                                p.team = n.rem_euclid(5) as u8;
                            }
                        }
                        "controller" => {
                            if let Some(k) = ControllerKind::from_str(v) {
                                p.controller = k;
                            }
                        }
                        "enabled" => {
                            if let Some(f) = flag() {
                                p.enabled = f;
                            }
                        }
                        "highlight" => {
                            if let Some(f) = flag() {
                                p.highlight_maptile = f;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        c
    }

    /// Carpeta de configuración del usuario (`$XDG_CONFIG_HOME` o `~/.config`) más el nombre del juego.
    pub fn default_path() -> Option<std::path::PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join(".config"))
            })?;
        Some(base.join("clanbomber-rs").join("config.txt"))
    }

    /// Carga la configuración; si no existe o no se puede leer, la de por defecto.
    pub fn load(path: impl AsRef<std::path::Path>) -> Config {
        std::fs::read_to_string(path)
            .map(|t| Config::from_text(&t))
            .unwrap_or_default()
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_original() {
        let c = Config::default();
        assert_eq!(
            (
                c.round_time,
                c.points_to_win,
                c.bomb_countdown,
                c.bomb_delay,
                c.bomb_speed
            ),
            (90, 5, 25, 10, 160)
        );
        assert_eq!((c.max_bombs, c.max_power, c.max_skateboards), (9, 12, 5));
        assert_eq!(c.number_of_players(), 2);
    }

    #[test]
    fn text_roundtrip_keeps_everything() {
        let mut c = Config::default();
        c.round_time = 30;
        c.koks = false;
        c.start_kick = true;
        c.bombers[3].name = "Zoe = ¡hola!".into();
        c.bombers[3].team = 2;
        c.bombers[3].controller = ControllerKind::Joystick(2);
        c.bombers[4].enabled = true;
        c.disabled_maps = vec!["Arena".into(), "Big Block".into()];
        assert_eq!(Config::from_text(&c.to_text()), c);
    }

    #[test]
    fn garbage_is_ignored() {
        let c = Config::from_text(
            "round_time=abc\nfoo=1\nbomber99.name=x\nbomber1.controller=zzz\n\nsin igual\nkoks=0",
        );
        assert_eq!(c.round_time, 90);
        assert!(!c.koks);
        assert_eq!(c.bombers[1].controller, ControllerKind::Keymap(2));
    }

    #[test]
    fn extras_already_owned_do_not_drop() {
        let mut c = Config::default();
        c.start_kick = true;
        assert!(!c.extra_config().kick);
        assert!(c.extra_config().glove);
    }

    #[test]
    fn opponents_count_teams_once() {
        let mut c = Config::default();
        for i in 0..4 {
            c.bombers[i].enabled = true;
        }
        assert_eq!(c.number_of_opponents(), 4);
        c.bombers[0].team = 1;
        c.bombers[1].team = 1;
        assert_eq!(c.number_of_opponents(), 3);
    }

    #[test]
    fn save_and_load_through_a_file() {
        let dir = std::env::temp_dir().join(format!("cb-config-test-{}", std::process::id()));
        let path = dir.join("sub").join("config.txt");
        let mut c = Config::default();
        c.points_to_win = 7;
        c.save(&path).unwrap();
        assert_eq!(Config::load(&path).points_to_win, 7);
        assert_eq!(Config::load(dir.join("missing")).points_to_win, 5);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn old_configs_naming_the_dropped_second_ai_fall_back_to_the_normal_one() {
        let c = Config::from_text("bomber3.controller=ai_mass");
        assert_eq!(c.bombers[3].controller, ControllerKind::Ai);
    }
}
