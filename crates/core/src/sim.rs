use crate::bomber::{Bomber, Input};
use crate::map::{Map, TILE_PIXELS};

/// Frecuencia fija de la simulación. El render interpola entre ticks.
pub const TICK_HZ: u32 = 60;

/// Duración de un tick en segundos (el original usa `Timer::time_elapsed()` variable).
pub const TICK_DT: f32 = 1.0 / TICK_HZ as f32;

pub struct World {
    pub map: Map,
    pub bombers: Vec<Bomber>,
    pub tick: u64,
}

impl World {
    pub fn new(map: Map) -> Self {
        World {
            map,
            bombers: Vec::new(),
            tick: 0,
        }
    }

    /// Crea un bomber por cada una de las primeras `n` posiciones de salida del mapa.
    pub fn spawn_bombers(&mut self, n: usize) {
        let t = TILE_PIXELS as i32;
        self.bombers = self
            .map
            .spawns
            .iter()
            .take(n)
            .map(|&(_, x, y)| Bomber::new(x as i32 * t, y as i32 * t))
            .collect();
    }

    /// Avanza exactamente un tick. `inputs[i]` es la entrada del bomber `i`.
    /// Aquí irán también bombas y explosiones.
    pub fn step(&mut self, inputs: &[Input]) {
        for (i, b) in self.bombers.iter_mut().enumerate() {
            b.act(
                TICK_DT,
                inputs.get(i).copied().unwrap_or_default(),
                &self.map,
            );
        }
        self.tick += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_is_deterministic() {
        let src = "x\n1\n".to_owned() + &"                 \n".repeat(13);
        let mut a = World::new(Map::parse(&src).unwrap());
        for _ in 0..100 {
            a.step(&[]);
        }
        assert_eq!(a.tick, 100);
    }
}
