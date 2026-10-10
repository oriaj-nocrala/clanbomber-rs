use crate::map::Map;

/// Frecuencia fija de la simulación. El render interpola entre ticks.
pub const TICK_HZ: u32 = 60;

pub struct World {
    pub map: Map,
    pub tick: u64,
}

impl World {
    pub fn new(map: Map) -> Self {
        World { map, tick: 0 }
    }

    /// Avanza exactamente un tick. Aquí irán bombas, bombers y explosiones.
    pub fn step(&mut self) {
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
            a.step();
        }
        assert_eq!(a.tick, 100);
    }
}
