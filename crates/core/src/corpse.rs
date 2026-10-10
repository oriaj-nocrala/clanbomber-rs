//! Cadáveres y trozos (ClanBomber2: Bomber_Corpse.cpp, Corpse_Part.cpp).

use crate::flight::Flight;
use crate::map::Map;
use crate::rng::Rng;

/// El cadáver que deja un bomber: se arrastra al azar a 30 px/s y reacciona a quien pasa cerca.
#[derive(Clone, Debug)]
pub struct Corpse {
    pub x: f32,
    pub y: f32,
    /// Bomber del que viene (de él salen el aspecto y el desplazamiento de dibujo).
    pub owner: usize,
    /// Celda del aspecto: `cur_dir * 10 + 9`.
    pub sprite_nr: u32,
    pub speed: i32,
    pub remainder: f32,
    pub ready_to_splash: bool,
    /// Salpicaduras ya hechas; a las 5 deja de moverse y de salpicar (-1).
    pub splash_counter: i32,
    /// Hasta que pasa este tiempo no revienta con una explosión.
    pub explode_delay: f32,
    pub falling: bool,
    pub fall_countdown: f32,
    pub fallen_down: bool,
    pub delete_me: bool,
}

impl Corpse {
    pub fn new(x: i32, y: i32, owner: usize, sprite_nr: u32) -> Self {
        Corpse {
            x: x as f32,
            y: y as f32,
            owner,
            sprite_nr,
            speed: 30,
            remainder: 0.0,
            ready_to_splash: true,
            splash_counter: 0,
            explode_delay: 0.8,
            falling: false,
            fall_countdown: 0.0,
            fallen_down: false,
            delete_me: false,
        }
    }

    pub fn get_x(&self) -> i32 {
        self.x as i32
    }

    pub fn get_y(&self) -> i32 {
        self.y as i32
    }

    /// Casilla bajo el centro.
    pub fn tile(&self) -> (i32, i32) {
        ((self.get_x() + 20) / 40, (self.get_y() + 20) / 40)
    }

    pub fn fall(&mut self) {
        if !self.falling {
            self.falling = true;
            self.fall_countdown = 1.0;
        }
    }

    pub fn continue_falling(&mut self, dt: f32) {
        self.speed = (self.fall_countdown * 60.0) as i32;
        self.fall_countdown -= dt;
        if self.fall_countdown < 0.0 {
            self.fallen_down = true;
            self.fall_countdown = 0.0;
        }
    }
}

/// Un trozo que sale volando al reventar un cadáver y se desvanece al aterrizar.
#[derive(Clone, Debug)]
pub struct CorpsePart {
    pub x: f32,
    pub y: f32,
    /// Celda 0..4 de `corpse_parts.png`.
    pub sprite_nr: u32,
    /// Segundos que quedan tras aterrizar; durante el último se desvanece.
    pub countdown: f32,
    pub flight: Flight,
    pub delete_me: bool,
}

/// `Corpse_Part::speed`.
const PART_SPEED: i32 = 800;

impl CorpsePart {
    pub fn new(x: i32, y: i32, rng: &mut Rng) -> Self {
        CorpsePart {
            x: x as f32,
            y: y as f32,
            sprite_nr: rng.below(4),
            countdown: 2.0,
            flight: Flight::default(),
            delete_me: false,
        }
    }

    /// `GameObject::fly_to(x, y)` a 800 px/s.
    pub fn fly_to(&mut self, dest: (i32, i32)) {
        self.flight.start((self.x, self.y), dest, PART_SPEED);
    }

    /// Opacidad 0..1 (`min(255, 255 * countdown)`).
    pub fn opacity(&self) -> f32 {
        if self.flight.active {
            1.0
        } else {
            self.countdown.clamp(0.0, 1.0)
        }
    }

    /// `Corpse_Part::act`. Con mapa, las casillas de muro frenan el vuelo (los créditos no tienen).
    pub fn act(&mut self, dt: f32, map: Option<&Map>) {
        if !self.flight.active {
            self.countdown -= dt;
            if self.countdown < 0.0 {
                self.countdown = 0.0;
                self.delete_me = true;
            }
        }
        if self.flight.active {
            let mut pos = (self.x, self.y);
            match map {
                Some(m) => self.flight.advance_center(&mut pos, dt, m),
                None => self.flight.advance(&mut pos, dt, None),
            }
            (self.x, self.y) = pos;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_part_flies_lands_and_fades_out_after_two_seconds() {
        let mut rng = Rng::new(1);
        let mut p = CorpsePart::new(100, 100, &mut rng);
        p.fly_to((300, 100));
        let mut t = 0.0f32;
        while p.flight.active {
            p.act(1.0 / 60.0, None);
            t += 1.0 / 60.0;
            assert!(t < 2.0);
        }
        assert!(
            (t - 200.0 / 800.0).abs() < 0.05,
            "vuelo de 200 px a 800 px/s: {t}"
        );
        assert_eq!((p.x as i32, p.y as i32), (300, 100));
        assert_eq!(p.opacity(), 1.0);
        for _ in 0..(1.5 * 60.0) as usize {
            p.act(1.0 / 60.0, None);
        }
        assert!(p.opacity() < 1.0 && !p.delete_me, "ya se desvanece");
        for _ in 0..60 {
            p.act(1.0 / 60.0, None);
        }
        assert!(p.delete_me);
    }

    #[test]
    fn walls_stop_a_flying_part() {
        let src =
            "x\n2\n".to_owned() + &(" ".repeat(7) + "*" + &" ".repeat(9) + &" ".repeat(17 * 12));
        let map = Map::parse(&src).unwrap();
        let mut rng = Rng::new(2);
        let mut p = CorpsePart::new(1 * 40, 0, &mut rng);
        p.fly_to((15 * 40, 0));
        for _ in 0..200 {
            p.act(1.0 / 60.0, Some(&map));
        }
        assert!(p.x < 7.0 * 40.0, "no atraviesa el muro: x={}", p.x);
    }
}
