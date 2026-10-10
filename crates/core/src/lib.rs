//! Lógica de ClanBomber sin dependencias gráficas.
//! Determinista y de paso fijo, pensada para testearse contra el original.

// Es un port fiel: se conserva la estructura de bucles del original (índices sobre la cuadrícula,
// `loop` con `break` al final de la cola...) aunque clippy sugiera otra forma.
#![allow(clippy::needless_range_loop, clippy::while_let_loop)]
// Las pruebas escriben posiciones como `1 * 40` (casilla 1) para que se lean como en el mapa.
#![cfg_attr(
    test,
    allow(
        clippy::identity_op,
        clippy::field_reassign_with_default,
        clippy::bool_comparison,
        clippy::nonminimal_bool
    )
)]

pub mod ai;
pub mod bomb;
pub mod bomber;
pub mod config;
pub mod corpse;
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
pub use corpse::{Corpse, CorpsePart};
pub use disease::{Disease, DiseaseKind};
pub use events::GameEvent;
pub use extra::{Extra, ExtraConfig, ExtraKind};
pub use map::{Dir, Map, MapList, Tile, MAP_HEIGHT, MAP_WIDTH, TILE_PIXELS};
pub use mapdoc::{map_file_name, MapDoc};
pub use rng::Rng;
pub use round::{Observer, Phase};
pub use sim::{World, TICK_HZ};
