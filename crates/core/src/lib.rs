//! Lógica de ClanBomber sin dependencias gráficas.
//! Determinista y de paso fijo, pensada para testearse contra el original.

pub mod map;
pub mod sim;

pub use map::{Dir, Map, Tile, MAP_HEIGHT, MAP_WIDTH, TILE_PIXELS};
pub use sim::{World, TICK_HZ};
