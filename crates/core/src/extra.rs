//! Extras que sueltan las cajas (ClanBomber2: Extra*.cpp, MapTile::spawn_extra).

use crate::flight::Flight;
use crate::rng::Rng;

/// Duración del parpadeo antes de desaparecer cuando una explosión lo alcanza.
pub const EXTRA_DESTROY_TIME: f32 = 0.5;

/// El valor es el índice de celda en `extras2.png` (`sprite_nr` del original).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtraKind {
    Bomb = 0,
    Power = 1,
    Skateboard = 2,
    Kick = 3,
    Glove = 4,
    Joint = 5,
    Viagra = 6,
    Koks = 7,
}

impl ExtraKind {
    /// Joint, Viagra y Koks son enfermedades y resisten las explosiones.
    pub fn destroyable(self) -> bool {
        !matches!(self, ExtraKind::Joint | ExtraKind::Viagra | ExtraKind::Koks)
    }
}

/// Qué extras pueden aparecer (`Config::get_*` del original).
#[derive(Clone, Copy, Debug)]
pub struct ExtraConfig {
    pub power: bool,
    pub bombs: bool,
    pub skateboards: bool,
    pub kick: bool,
    pub glove: bool,
    pub joint: bool,
    pub viagra: bool,
    pub koks: bool,
}

impl Default for ExtraConfig {
    fn default() -> Self {
        ExtraConfig {
            power: true,
            bombs: true,
            skateboards: true,
            kick: true,
            glove: true,
            joint: true,
            viagra: true,
            koks: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Extra {
    /// Posición en píxeles (esquina de la casilla; fraccionaria durante el vuelo).
    pub x: f32,
    pub y: f32,
    pub kind: ExtraKind,
    pub destroyed: bool,
    pub destroy_countdown: f32,
    pub delete_me: bool,
    pub falling: bool,
    pub fall_countdown: f32,
    pub flight: Flight,
}

impl Extra {
    pub fn new(x: i32, y: i32, kind: ExtraKind) -> Self {
        Extra {
            x: x as f32,
            y: y as f32,
            kind,
            destroyed: false,
            destroy_countdown: 0.0,
            delete_me: false,
            falling: false,
            fall_countdown: 0.0,
            flight: Flight::default(),
        }
    }

    pub fn tile(&self) -> (i32, i32) {
        (((self.x as i32) + 20) / 40, ((self.y as i32) + 20) / 40)
    }

    /// `GameObject::fall()`: cae por un agujero y desaparece tras 1 s.
    pub fn fall(&mut self) {
        if !self.falling {
            self.falling = true;
            self.fall_countdown = 1.0;
        }
    }

    /// ¿Cuenta como presente en la casilla? Los que vuelan no (`maptilize_objects`).
    pub fn rests_on_tile(&self) -> bool {
        !self.flight.active
    }

    /// `Extra::destroy()` para los destruibles: parpadean 0.5 s y desaparecen. Los demás
    /// (enfermedades) salen volando: lo resuelve el `World`, que sortea el destino.
    pub fn destroy(&mut self) {
        if !self.destroyed && self.kind.destroyable() {
            self.destroyed = true;
            self.destroy_countdown = EXTRA_DESTROY_TIME;
        }
    }

    /// `GameObject::fly_to(maptile)`: sale despedido hacia la esquina de la casilla.
    /// Velocidad base 240 px/s (la de cualquier `GameObject`).
    pub fn fly_to(&mut self, tile: (usize, usize)) {
        if !self.falling {
            let dest = (tile.0 as i32 * 40, tile.1 as i32 * 40);
            self.flight.start((self.x, self.y), dest, 240);
        }
    }

    pub fn continue_flying(&mut self, dt: f32) {
        let mut pos = (self.x, self.y);
        self.flight.advance(&mut pos, dt, None);
        (self.x, self.y) = pos;
    }
}

/// `MapTile::spawn_extra()`: decide qué extra (si alguno) suelta una caja.
/// Misma tabla de probabilidades que el original; un tipo deshabilitado vuelve a sortear.
pub fn roll_extra(rng: &mut Rng, cfg: &ExtraConfig) -> Option<ExtraKind> {
    // Sin ningún extra habilitado el original recursaría sin fin.
    if !(cfg.power
        || cfg.bombs
        || cfg.skateboards
        || cfg.kick
        || cfg.glove
        || cfg.joint
        || cfg.viagra
        || cfg.koks)
    {
        return None;
    }
    loop {
        let pick = match rng.below(8) {
            0 => cfg.power.then_some(ExtraKind::Power),
            1 => cfg.bombs.then_some(ExtraKind::Bomb),
            2 => cfg.skateboards.then_some(ExtraKind::Skateboard),
            3 => match rng.below(2) {
                0 => cfg.kick.then_some(ExtraKind::Kick),
                _ => cfg.glove.then_some(ExtraKind::Glove),
            },
            4 => match rng.below(16) {
                0 => cfg.joint.then_some(ExtraKind::Joint),
                1 => cfg.viagra.then_some(ExtraKind::Viagra),
                2 => cfg.koks.then_some(ExtraKind::Koks),
                // El resto: no cae nada.
                _ => return None,
            },
            // 5, 6 y 7: no cae nada.
            _ => return None,
        };
        if pick.is_some() {
            return pick;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_rates_match_the_original_table() {
        let mut rng = Rng::new(3);
        let cfg = ExtraConfig::default();
        let n = 160_000;
        let mut count = std::collections::HashMap::new();
        for _ in 0..n {
            *count.entry(roll_extra(&mut rng, &cfg)).or_insert(0u32) += 1;
        }
        let rate = |k| count.get(&k).copied().unwrap_or(0) as f64 / n as f64;
        // 1/8 cada uno: Power, Bomb, Skateboard; 1/16: Kick y Glove; 1/128: cada enfermedad.
        assert!((rate(Some(ExtraKind::Power)) - 0.125).abs() < 0.01);
        assert!((rate(Some(ExtraKind::Bomb)) - 0.125).abs() < 0.01);
        assert!((rate(Some(ExtraKind::Kick)) - 0.0625).abs() < 0.01);
        assert!((rate(Some(ExtraKind::Joint)) - 1.0 / 128.0).abs() < 0.004);
        assert!((rate(None) - (3.0 / 8.0 + 13.0 / 128.0)).abs() < 0.01);
    }

    #[test]
    fn disabled_kinds_are_rerolled() {
        let mut rng = Rng::new(9);
        let cfg = ExtraConfig {
            power: false,
            kick: false,
            ..ExtraConfig::default()
        };
        for _ in 0..10_000 {
            let k = roll_extra(&mut rng, &cfg);
            assert!(!matches!(k, Some(ExtraKind::Power) | Some(ExtraKind::Kick)));
        }
    }
}
