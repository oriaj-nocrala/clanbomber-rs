//! Lógica de ClanBomber sin dependencias gráficas.
//! Determinista y de paso fijo, pensada para testearse contra el original.

pub mod ai;
pub mod bomb;
pub mod bomber;
pub mod config;
pub mod disease;
pub mod events;
pub mod extra;
pub mod flight;
pub mod map;
pub mod mapdoc;
pub mod movement;
pub mod rng;
pub mod round;
pub mod sim;

pub use ai::Ai;
pub use bomb::{Bomb, Explosion, Object};
pub use bomber::{Bomber, Input};
pub use config::{BomberConfig, Config, ControllerKind, MAX_PLAYERS, NR_SKINS};
pub use disease::{Disease, DiseaseKind};
pub use events::GameEvent;
pub use extra::{Extra, ExtraConfig, ExtraKind};
pub use map::{Dir, Map, MapList, Tile, MAP_HEIGHT, MAP_WIDTH, TILE_PIXELS};
pub use mapdoc::{map_file_name, MapDoc};
pub use rng::Rng;
pub use round::{Observer, Phase};
pub use sim::{World, TICK_HZ};
