//! Sucesos del juego que el exterior puede querer oír (el núcleo no sabe de sonido).

/// Equivalen a los `Resources::*()->play()` del original dentro de la lógica de partida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameEvent {
    PutBomb,
    Explode,
    Die,
    DeepFall,
    /// Un extra bueno (bomba, potencia, patín, patada, guante).
    Wow,
    Joint,
    Horny,
    Schnief,
    /// Se acabó el tiempo y empieza a desaparecer el suelo.
    TimeOver,
    HurryUp,
    /// El observador se come una casilla.
    Crunch,
    /// Fin de la partida (marcador del campeón).
    Klatsch,
}
