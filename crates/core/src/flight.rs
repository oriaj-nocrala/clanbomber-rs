//! Vuelo en arco de objetos (`GameObject::fly_to` / `continue_flying`, ClanBomber2:
//! GameObject.cpp): lo usan las bombas lanzadas y los extras que salen despedidos.

use crate::map::Map;

#[derive(Clone, Copy, Debug, Default)]
pub struct Flight {
    pub active: bool,
    dest: (f32, f32),
    dist: (f32, f32),
    /// Fracción del trayecto por segundo (`fly_speed`): la velocidad se normaliza por la
    /// distancia, así que un vuelo de `d` píxeles a `v` px/s dura `d / v` segundos.
    pub speed: f32,
    /// 0..1 durante el vuelo; queda en exactamente 1 al aterrizar (indica vuelo terminado).
    pub progress: f32,
}

impl Flight {
    /// `GameObject::fly_to(x, y, speed)`. Si el destino es la propia posición no hay trayecto
    /// que recorrer (el original dividiría entre cero) y se queda donde está.
    pub fn start(&mut self, pos: (f32, f32), dest: (i32, i32), base_speed: i32) {
        if self.active {
            return;
        }
        let dist = (dest.0 as f32 - pos.0, dest.1 as f32 - pos.1);
        let len = (dist.0 * dist.0 + dist.1 * dist.1).sqrt();
        if len == 0.0 {
            return;
        }
        self.active = true;
        self.dest = (dest.0 as f32, dest.1 as f32);
        self.dist = dist;
        self.speed = base_speed as f32 / len;
        self.progress = 0.0;
    }

    /// `continue_flying`. Con `walls = Some(mapa)` (`can_fly_over_walls == false`) los muros y
    /// el borde frenan el vuelo (se comprueban las cuatro esquinas de la casilla del objeto);
    /// las cajas no.
    pub fn advance(&mut self, pos: &mut (f32, f32), dt: f32, walls: Option<&Map>) {
        self.advance_checked(pos, dt, |p| {
            walls.is_some_and(|map| {
                let (x, y) = (p.0 as i32, p.1 as i32);
                map.is_wall_px(x, y)
                    || map.is_wall_px(x + 39, y)
                    || map.is_wall_px(x, y + 39)
                    || map.is_wall_px(x + 39, y + 39)
            })
        });
    }

    /// Variante de las partes de cadáver (`continue_flying` con `CORPSE_PART`): solo cuenta la
    /// casilla bajo el centro (`get_maptile()`).
    pub fn advance_center(&mut self, pos: &mut (f32, f32), dt: f32, map: &Map) {
        self.advance_checked(pos, dt, |p| {
            map.is_wall_px(p.0 as i32 + 20, p.1 as i32 + 20)
        });
    }

    /// Un paso del vuelo; si `blocked` dice que la nueva posición choca, se deshace el paso y el
    /// destino pasa a ser donde se está.
    fn advance_checked(
        &mut self,
        pos: &mut (f32, f32),
        dt: f32,
        blocked: impl Fn((f32, f32)) -> bool,
    ) {
        if dt == 0.0 {
            return;
        }
        let mut time_step = dt;
        let mut steps = 1;
        while ((time_step * self.speed * self.dist.0) as i32).abs() > 5
            || ((time_step * self.speed * self.dist.1) as i32).abs() > 5
        {
            time_step /= 2.0;
            steps *= 2;
        }

        for _ in 0..steps {
            let (dx, dy) = (
                time_step * self.speed * self.dist.0,
                time_step * self.speed * self.dist.1,
            );
            pos.0 += dx;
            pos.1 += dy;
            if blocked(*pos) {
                pos.0 -= dx;
                pos.1 -= dy;
                self.dest = *pos;
            }
            self.progress += time_step * self.speed;
            if self.progress >= 1.0 {
                self.active = false;
                self.progress = 1.0;
                *pos = self.dest;
            }
        }
    }

    /// Escala de dibujo durante el vuelo (`GameObject::show`): crece hasta mitad de trayecto.
    /// El original usa `3.14f` y no π: se conserva para que las animaciones coincidan.
    #[allow(clippy::approx_constant)]
    pub fn scale(&self) -> f32 {
        (self.progress * 3.14).sin() * (1.3 / self.speed) + 1.0
    }
}
