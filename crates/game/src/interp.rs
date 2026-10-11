//! Interpolación al dibujar. La simulación avanza a 60 ticks fijos por segundo (como el
//! original, y determinista); en un monitor de 120/144/180 Hz cada estado se vería varios
//! fotogramas seguidos y el movimiento iría a saltos. Aquí se guardan las posiciones de antes del
//! último tick y se dibuja una copia del mundo con las posiciones a medio camino, según cuánto
//! tiempo sobra en el acumulador. El mundo real no se toca.

use cb_core::{Object, World};

/// Más que esto en un tick no es movimiento sino un salto (nueva ronda, reaparición...): no se
/// interpola.
const MAX_STEP: f32 = 48.0;

/// Posiciones de lo que se mueve, tomadas justo antes de un tick.
#[derive(Default)]
pub struct Snapshot {
    bombers: Vec<(f32, f32)>,
    /// Objetos que se mueven, con una clave para emparejarlos (tipo y dueño/clase).
    objects: Vec<(u32, f32, f32)>,
}

/// Clave de emparejamiento de un objeto: los del mismo tipo y dueño se emparejan por cercanía.
fn key(o: &Object) -> Option<(u32, f32, f32)> {
    Some(match o {
        Object::Bomb(b) => (b.owner as u32, b.x, b.y),
        Object::Extra(e) => (100 + e.kind as u32, e.x, e.y),
        Object::Corpse(c) => (200 + c.owner as u32, c.x, c.y),
        Object::CorpsePart(p) => (300, p.x, p.y),
        Object::Explosion(_) => return None,
    })
}

impl Snapshot {
    pub fn of(w: &World) -> Self {
        Snapshot {
            bombers: w.bombers.iter().map(|b| (b.x, b.y)).collect(),
            objects: w.objects.iter().filter_map(key).collect(),
        }
    }

    /// La posición anterior más cercana con la misma clave, si no está demasiado lejos.
    fn previous(&self, k: u32, x: f32, y: f32) -> Option<(f32, f32)> {
        self.objects
            .iter()
            .filter(|o| o.0 == k)
            .map(|o| (o.1, o.2))
            .min_by(|a, b| {
                let da = (a.0 - x).powi(2) + (a.1 - y).powi(2);
                let db = (b.0 - x).powi(2) + (b.1 - y).powi(2);
                da.total_cmp(&db)
            })
            .filter(|p| near(*p, (x, y)))
    }
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() <= MAX_STEP && (a.1 - b.1).abs() <= MAX_STEP
}

fn lerp(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// Copia del mundo para dibujar, con las posiciones entre `prev` (t = 0) y `w` (t = 1).
pub fn view(w: &World, prev: &Snapshot, t: f32) -> World {
    let mut v = w.clone();
    let t = t.clamp(0.0, 1.0);
    for (b, &p) in v.bombers.iter_mut().zip(&prev.bombers) {
        if near(p, (b.x, b.y)) {
            (b.x, b.y) = lerp(p, (b.x, b.y), t);
        }
    }
    for o in &mut v.objects {
        let Some((k, x, y)) = key(o) else {
            continue;
        };
        let Some(p) = prev.previous(k, x, y) else {
            continue;
        };
        let (nx, ny) = lerp(p, (x, y), t);
        match o {
            Object::Bomb(b) => (b.x, b.y) = (nx, ny),
            Object::Extra(e) => (e.x, e.y) = (nx, ny),
            Object::Corpse(c) => (c.x, c.y) = (nx, ny),
            Object::CorpsePart(c) => (c.x, c.y) = (nx, ny),
            Object::Explosion(_) => {}
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use cb_core::{Input, Map};

    fn world() -> World {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps");
        // El orden de `read_dir` depende del sistema de archivos: se ordena para que la prueba
        // use el mismo mapa en todas partes.
        let mut maps: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "map"))
            .collect();
        maps.sort();
        let path = maps.into_iter().next().unwrap();
        let mut w = World::with_seed(Map::load(path).unwrap(), 1);
        w.start_match(2);
        w
    }

    #[test]
    fn halfway_between_ticks() {
        let mut w = world();
        // Que terminen de aterrizar y se puedan mover.
        for _ in 0..300 {
            w.step(&[Input::default(), Input::default()]);
        }
        // La primera dirección en la que el bomber 0 puede andar desde donde aparece.
        let dirs = [
            Input {
                right: true,
                ..Input::default()
            },
            Input {
                left: true,
                ..Input::default()
            },
            Input {
                down: true,
                ..Input::default()
            },
            Input {
                up: true,
                ..Input::default()
            },
        ];
        let (before, p0, w) = dirs
            .iter()
            .find_map(|&dir| {
                let mut w = w.clone();
                let before = Snapshot::of(&w);
                let p0 = (w.bombers[0].x, w.bombers[0].y);
                w.step(&[dir, Input::default()]);
                let moved = (w.bombers[0].x, w.bombers[0].y) != p0;
                moved.then_some((before, p0, w))
            })
            .expect("el bomber debe poder moverse en alguna dirección");
        let p1 = (w.bombers[0].x, w.bombers[0].y);
        let v = view(&w, &before, 0.5);
        assert!((v.bombers[0].x - (p0.0 + p1.0) / 2.0).abs() < 1e-4);
        assert!((v.bombers[0].y - (p0.1 + p1.1) / 2.0).abs() < 1e-4);
        // El mundo real no cambia.
        assert_eq!((w.bombers[0].x, w.bombers[0].y), p1);
        let end = view(&w, &before, 1.0);
        assert_eq!((end.bombers[0].x, end.bombers[0].y), p1);
    }

    #[test]
    fn jumps_are_not_interpolated() {
        let w = world();
        let mut before = Snapshot::of(&w);
        before.bombers[0].0 += 200.0;
        let v = view(&w, &before, 0.5);
        assert_eq!(v.bombers[0].x, w.bombers[0].x);
    }
}
