//! Movimiento por píxeles compartido por bombers y bombas: port de
//! `GameObject::move()` y `move_left/right/up/down` (ClanBomber2: GameObject.cpp).
//!
//! Las posiciones son píxeles dentro del mapa (0..680 x 0..520): `f32` internamente y
//! enteros truncados (`get_x`) en las comprobaciones, como `x`/`y` del original.

use crate::map::{Dir, Map, TILE_PIXELS};

const T: i32 = TILE_PIXELS as i32;

/// Consultas al mundo que necesita el movimiento (lo implementa quien posee los objetos).
pub trait Query {
    /// Índice (en `World::objects`) de la bomba apoyada en esa casilla, si hay (`tile->bomb`).
    fn bomb_at(&self, tx: i32, ty: i32) -> Option<usize>;
    /// ¿Hay un bomber vivo en esa casilla? (`MapTile::has_bomber`)
    fn has_bomber(&self, tx: i32, ty: i32) -> bool;
    /// ¿La bomba `i` está quieta (`cur_dir == DIR_NONE`)? Solo se puede patear una quieta.
    fn bomb_is_still(&self, i: usize) -> bool;
}

/// Lo que un objeto móvil necesita saber del mundo.
pub struct Env<'a> {
    pub map: &'a Map,
    pub q: &'a dyn Query,
}

/// Propiedades de quien se mueve.
pub struct Mover {
    /// Los bombers se atraviesan entre sí; las bombas no atraviesan bombers.
    pub can_pass_bomber: bool,
    pub can_kick: bool,
    /// Índice de la propia bomba, para no chocar consigo misma (`bomb == this`).
    pub self_bomb: Option<usize>,
}

#[derive(Default)]
pub struct Advance {
    /// `false` si chocó (el original llama a `stop()` y devuelve `false`).
    pub moved: bool,
    /// Bomba a patear: (índice del objeto, dirección).
    pub kick: Option<(usize, Dir)>,
}

pub struct Pos<'a> {
    pub x: &'a mut f32,
    pub y: &'a mut f32,
    /// Fracción de píxel acumulada entre ticks (`remainder`).
    pub remainder: &'a mut f32,
}

/// `GameObject::move()`: avanza `dt * speed` píxeles (con resto acumulado) de uno en uno.
pub fn advance(p: &mut Pos, speed: i32, dt: f32, dir: Dir, m: &Mover, env: &Env) -> Advance {
    let amount = dt * speed as f32;
    let mut span = amount as i32;
    *p.remainder += amount - (amount as i32) as f32;
    span += *p.remainder as i32;
    *p.remainder -= (*p.remainder as i32) as f32;

    for _ in 0..span {
        let step = match dir {
            Dir::Left => step_left(p, m, env),
            Dir::Right => step_right(p, m, env),
            Dir::Up => step_up(p, m, env),
            Dir::Down => step_down(p, m, env),
        };
        if !step.moved {
            return step;
        }
    }
    Advance {
        moved: true,
        kick: None,
    }
}

fn gx(p: &Pos) -> i32 {
    *p.x as i32
}

fn gy(p: &Pos) -> i32 {
    *p.y as i32
}

/// `get_maptile()` del original: la casilla bajo el centro.
fn own_tile(p: &Pos) -> (i32, i32) {
    ((gx(p) + 20) / T, (gy(p) + 20) / T)
}

fn blocked() -> Advance {
    Advance {
        moved: false,
        kick: None,
    }
}

fn ok() -> Advance {
    Advance {
        moved: true,
        kick: None,
    }
}

/// ¿Puede avanzar hacia `ahead` según la bomba que hay allí? `Err` lleva la bomba que la frena.
/// Permite pasar si no hay bomba, si es la propia o si ya se pasó la mitad de la casilla.
fn bomb_gate(
    m: &Mover,
    env: &Env,
    ahead: (i32, i32),
    past_half: bool,
) -> Result<(), Option<usize>> {
    match env.q.bomb_at(ahead.0, ahead.1) {
        None => Ok(()),
        Some(b) if m.self_bomb == Some(b) => Ok(()),
        Some(_) if past_half => Ok(()),
        Some(b) => Err(Some(b)),
    }
}

fn kick_or_block(m: &Mover, env: &Env, bomb: Option<usize>, dir: Dir) -> Advance {
    match bomb {
        Some(b) if m.can_kick && env.q.bomb_is_still(b) => Advance {
            moved: false,
            kick: Some((b, dir)),
        },
        _ => blocked(),
    }
}

// Los cuatro `step_*` conservan la estructura del original, incluido el ajuste lateral
// (`y++`/`y--`) que desliza al objeto alrededor de las esquinas.

fn step_right(p: &mut Pos, m: &Mover, env: &Env) -> Advance {
    if env.map.blocking_px(gx(p) + T, gy(p) + 20) {
        return blocked();
    }
    let ahead = ((gx(p) + T) / T, (gy(p) + 20) / T);
    if let Err(b) = bomb_gate(m, env, ahead, gx(p) % T > 19) {
        return kick_or_block(m, env, b, Dir::Right);
    }
    *p.x += 1.0;
    let (x, y) = (gx(p), gy(p));
    if y % T > 19 {
        if env.map.blocking_tile((x + T) / T, (y - 20) / T) {
            *p.y += 1.0;
        }
    } else if y % T > 0 && env.map.blocking_tile((x + T) / T, (y + 60) / T) {
        *p.y -= 1.0;
    }
    if !m.can_pass_bomber && env.q.has_bomber(ahead.0, ahead.1) && ahead != own_tile(p) {
        *p.x -= 1.0;
        return blocked();
    }
    ok()
}

fn step_left(p: &mut Pos, m: &Mover, env: &Env) -> Advance {
    if env.map.blocking_px(gx(p) - 1, gy(p) + 20) {
        return blocked();
    }
    let ahead = ((gx(p) - 1) / T, (gy(p) + 20) / T);
    if let Err(b) = bomb_gate(m, env, ahead, gx(p) % T < 20) {
        return kick_or_block(m, env, b, Dir::Left);
    }
    *p.x -= 1.0;
    let (x, y) = (gx(p), gy(p));
    if y % T > 19 {
        if env.map.blocking_tile((x - 1) / T, (y - 20) / T) {
            *p.y += 1.0;
        }
    } else if y % T > 0 && env.map.blocking_tile((x - 1) / T, (y + 60) / T) {
        *p.y -= 1.0;
    }
    if !m.can_pass_bomber && env.q.has_bomber(ahead.0, ahead.1) && ahead != own_tile(p) {
        *p.x += 1.0;
        return blocked();
    }
    ok()
}

fn step_up(p: &mut Pos, m: &Mover, env: &Env) -> Advance {
    if env.map.blocking_px(gx(p) + 20, gy(p) - 1) {
        return blocked();
    }
    let ahead = ((gx(p) + 20) / T, (gy(p) - 1) / T);
    if let Err(b) = bomb_gate(m, env, ahead, gy(p) % T < 20) {
        return kick_or_block(m, env, b, Dir::Up);
    }
    *p.y -= 1.0;
    let (x, y) = (gx(p), gy(p));
    if x % T > 19 {
        if env.map.blocking_tile((x - 20) / T, (y - 1) / T) {
            *p.x += 1.0;
        }
    } else if x % T > 0 && env.map.blocking_tile((x + 60) / T, (y - 1) / T) {
        *p.x -= 1.0;
    }
    if !m.can_pass_bomber && env.q.has_bomber(ahead.0, ahead.1) && ahead != own_tile(p) {
        *p.y += 1.0;
        return blocked();
    }
    ok()
}

fn step_down(p: &mut Pos, m: &Mover, env: &Env) -> Advance {
    if env.map.blocking_px(gx(p) + 20, gy(p) + T) {
        return blocked();
    }
    let ahead = ((gx(p) + 20) / T, (gy(p) + T) / T);
    if let Err(b) = bomb_gate(m, env, ahead, gy(p) % T > 19) {
        return kick_or_block(m, env, b, Dir::Down);
    }
    *p.y += 1.0;
    let (x, y) = (gx(p), gy(p));
    if x % T > 19 {
        if env.map.blocking_tile((x - 20) / T, (y + T) / T) {
            *p.x += 1.0;
        }
    } else if x % T > 0 && env.map.blocking_tile((x + 60) / T, (y + T) / T) {
        *p.x -= 1.0;
    }
    if !m.can_pass_bomber && env.q.has_bomber(ahead.0, ahead.1) && ahead != own_tile(p) {
        *p.y -= 1.0;
        return blocked();
    }
    ok()
}
