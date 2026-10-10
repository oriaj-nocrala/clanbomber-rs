//! Pruebas de determinismo: base del juego en red. Una partida entera (IA, humanos con teclas
//! aleatorias, rondas, marcador...) tiene que dar exactamente el mismo estado con la misma semilla
//! y las mismas teclas, se pueda clonar a mitad y continuar igual, y dar el mismo resultado en
//! todos los sistemas (la CI corre estos tests en Linux, Windows y macOS ARM).

use crate::bomber::Input;
use crate::map::Map;
use crate::rng::Rng;
use crate::round::Phase;
use crate::sim::World;

fn arena() -> Map {
    Map::load(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/maps/Arena.map"
    ))
    .unwrap()
}

/// Cuatro bombers: dos con IA y dos "humanos" cuyas teclas salen del generador de entradas.
fn match_world(seed: u64) -> World {
    let mut w = World::with_seed(arena(), seed);
    w.start_match(4);
    w.set_ai(2, true);
    w.set_ai(3, true);
    w
}

/// Teclas de un tick para los dos humanos. Cambian de dirección cada 15 ticks y ponen bombas a
/// ratos; nada de esto toca el `Rng` del mundo.
fn next_inputs(rng: &mut Rng, tick: u64, held: &mut [Input; 2]) -> [Input; 2] {
    if tick.is_multiple_of(15) {
        for h in held.iter_mut() {
            let d = rng.below(6);
            *h = Input {
                left: d == 0,
                right: d == 1,
                up: d == 2,
                down: d == 3,
                bomb: false,
            };
        }
    }
    let mut out = *held;
    for o in out.iter_mut() {
        o.bomb = rng.below(40) == 0;
    }
    out
}

/// Mundo + generador de teclas + teclas mantenidas: todo lo que hace falta para seguir una partida.
#[derive(Clone)]
struct Run {
    world: World,
    keys: Rng,
    held: [Input; 2],
    /// Ticks que lleva el marcador en pantalla.
    on_scores: u32,
}

impl Run {
    fn new(seed: u64) -> Run {
        Run {
            world: match_world(seed),
            keys: Rng::new(seed ^ 0xA5A5_A5A5),
            held: [Input::default(); 2],
            on_scores: 0,
        }
    }

    fn step(&mut self) {
        let inputs = next_inputs(&mut self.keys, self.world.tick, &mut self.held);
        self.world.step(&inputs);
        if matches!(self.world.phase, Phase::Scores { .. }) {
            self.on_scores += 1;
            if self.on_scores > 30 {
                self.world.confirm();
                self.on_scores = 0;
            }
        }
    }

    /// Avanza `ticks` y devuelve la huella del estado cada `every` ticks.
    fn hashes(&mut self, ticks: u32, every: u32) -> Vec<u64> {
        let mut out = Vec::new();
        for t in 1..=ticks {
            self.step();
            if t % every == 0 {
                out.push(self.world.state_hash());
            }
        }
        out
    }
}

#[test]
fn the_same_seed_gives_the_same_state_at_every_checkpoint() {
    let a = Run::new(42).hashes(60 * 120, 300);
    let b = Run::new(42).hashes(60 * 120, 300);
    assert_eq!(a, b);
    assert!(
        a.windows(2).any(|w| w[0] != w[1]),
        "el estado cambia con el tiempo"
    );
}

#[test]
fn a_different_seed_diverges() {
    let a = Run::new(1).hashes(60 * 20, 300);
    let b = Run::new(2).hashes(60 * 20, 300);
    assert_ne!(a.last(), b.last());
}

/// Lo que el rollback necesita: clonar el mundo (y las teclas) en cualquier momento y seguir desde
/// ahí da exactamente lo mismo que no haberlo clonado. Si `World` guardara estado fuera de sí mismo
/// (globales, relojes...) esto fallaría.
#[test]
fn a_cloned_world_continues_exactly_like_the_original() {
    let mut original = Run::new(7);
    original.hashes(60 * 30, 600);
    let mut snapshot = original.clone();
    let a = original.hashes(60 * 60, 300);
    let b = snapshot.hashes(60 * 60, 300);
    assert_eq!(a, b);
}

/// Rebobinar: volver a un instante anterior y repetir las mismas teclas reproduce la partida.
#[test]
fn rewinding_to_an_old_snapshot_and_replaying_reproduces_the_match() {
    let mut run = Run::new(99);
    run.hashes(60 * 10, 600);
    let saved = run.clone();
    let first = run.hashes(60 * 40, 300);
    let mut replay = saved;
    let second = replay.hashes(60 * 40, 300);
    assert_eq!(first, second);
}

/// Huella fija de una partida concreta. Si cambia sin que hayas tocado las reglas a propósito, la
/// simulación dejó de ser reproducible (o difiere entre sistemas: la CI corre esto en Linux, Windows
/// y macOS ARM). Si cambiaste las reglas, copia el valor que imprime el fallo.
const GOLDEN_SEED: u64 = 12345;
const GOLDEN_TICKS: u32 = 60 * 90;
const GOLDEN: u64 = 0x1a7f_eb71_5c64_2d7d;

#[test]
fn golden_hash_is_the_same_on_every_platform() {
    let mut run = Run::new(GOLDEN_SEED);
    run.hashes(GOLDEN_TICKS, GOLDEN_TICKS);
    let h = run.world.state_hash();
    assert_eq!(
        h, GOLDEN,
        "huella nueva: 0x{h:016x} (actualiza GOLDEN si el cambio fue a propósito)"
    );
}
