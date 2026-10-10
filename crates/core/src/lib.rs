//! Lógica de ClanBomber sin dependencias gráficas.
//! Determinista y de paso fijo, pensada para testearse contra el original.

pub mod bomb;
pub mod bomber;
pub mod disease;
pub mod extra;
pub mod flight;
pub mod map;
pub mod movement;
pub mod rng;
pub mod sim;

pub use bomb::{Bomb, Explosion, Object};
pub use bomber::{Bomber, Input};
pub use disease::{Disease, DiseaseKind};
pub use extra::{Extra, ExtraConfig, ExtraKind};
pub use map::{Dir, Map, Tile, MAP_HEIGHT, MAP_WIDTH, TILE_PIXELS};
pub use rng::Rng;
pub use sim::{World, TICK_HZ};
