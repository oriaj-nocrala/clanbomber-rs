//! Dibujo de la partida: mapa, objetos, observador, reloj y marcador.
//! Todo en el lienzo virtual de 800x600, con las coordenadas y el orden del original.

use cb_core::{Dir, Map, Object, Phase, Tile, World, MAP_HEIGHT, MAP_WIDTH, TICK_HZ, TILE_PIXELS};
use cb_render::{Align, Renderer, Sheet};

use crate::assets::Assets;

/// `offset_x` de dibujo de todos los bombers (`Bomber::Bomber`); el `offset_y` depende del aspecto.
const BOMBER_OFFSET_X: f32 = 60.0;

/// Quién juega en cada puesto del mundo (mismo orden que `World::bombers`).
pub struct Player {
    pub name: String,
    /// Aspecto (0..8): índice en `Assets::skins` y color de sus bombas.
    pub skin: usize,
}

/// Color de texto del original (0xd3d7cf), pasado a lineal porque la superficie es sRGB.
pub fn text_color() -> [f32; 4] {
    let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
    [lin(0xd3), lin(0xd7), lin(0xcf), 1.0]
}

/// Color RGBA de 8 bits (como `CB_FillRect`) pasado a lineal: el alfa no se convierte.
pub fn srgb_rgba(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    let lin = |c: u8| (c as f32 / 255.0).powf(2.2);
    [lin(r), lin(g), lin(b), a as f32 / 255.0]
}

/// Estado de la interfaz que no vive en el mundo.
pub struct Ui {
    /// `pokal_scroll_in` del marcador: la copa entra desde la derecha (800 → 100).
    pub scroll_in: f32,
    was_scores: bool,
}

impl Ui {
    pub fn new() -> Self {
        Ui {
            scroll_in: 800.0,
            was_scores: false,
        }
    }

    /// Avanza la animación del marcador `dt` segundos.
    pub fn update(&mut self, world: &World, dt: f32) {
        let scores = matches!(world.phase, Phase::Scores { .. });
        if scores && !self.was_scores {
            self.scroll_in = 800.0;
        }
        self.was_scores = scores;
        if let Phase::Scores { end_of_game, .. } = world.phase {
            if self.scroll_in > 100.0 {
                self.scroll_in -= dt * if end_of_game { 333.0 } else { 1000.0 };
            }
        }
    }
}

/// `GameObject::show(x, y, scale)`: sprite escalado alrededor de su centro (se encoge al caer,
/// pulsa al estar enfermo). `pos` es (x + 60, y + 40) del objeto y `offset_y` su `offset_y`.
fn draw_scaled(
    r: &mut Renderer,
    sheet: &Sheet,
    cell: u32,
    pos: (f32, f32),
    offset_y: f32,
    scale: f32,
    alpha: f32,
) {
    let x = (20.0 + pos.0 - 20.0 * scale).trunc();
    let y = (pos.1 + 20.0 + (-40.0 + offset_y) * scale - 20.0 * scale).trunc();
    r.sprite_tinted(
        sheet.tex,
        sheet.cell(cell),
        [x, y, sheet.cell_w * scale, sheet.cell_h * scale],
        [1.0, 1.0, 1.0, alpha],
    );
}

/// Objeto en vuelo: crece con `scale` y se ancla por abajo (fórmula de `GameObject::show` al volar).
fn draw_flying(
    r: &mut Renderer,
    sheet: &Sheet,
    cell: u32,
    pos: (f32, f32),
    scale: f32,
    alpha: f32,
) {
    let x = (pos.0 + 20.0 - scale * 20.0).trunc();
    let y = (pos.1 + 40.0 - scale * 40.0).trunc();
    r.sprite_tinted(
        sheet.tex,
        sheet.cell(cell),
        [x, y, sheet.cell_w * scale, sheet.cell_h * scale],
        [1.0, 1.0, 1.0, alpha],
    );
}

/// Al caer el objeto se encoge y se desvanece con su contador.
fn draw_falling(
    r: &mut Renderer,
    sheet: &Sheet,
    cell: u32,
    pos: (f32, f32),
    offset_y: f32,
    fall: f32,
) {
    let s = fall.clamp(0.0, 1.0);
    draw_scaled(r, sheet, cell, pos, offset_y, s, s);
}

pub fn draw(r: &mut Renderer, a: &Assets, w: &World, ui: &Ui, players: &[Player], paused: bool) {
    match w.phase {
        Phase::Playing => draw_game(r, a, w, players),
        Phase::Scores {
            winner,
            end_of_game,
        } => draw_scores(r, a, w, ui, players, winner, end_of_game),
    }
    if paused {
        // `- PAUSED -` sobre la partida oscurecida.
        r.quad(0.0, 0.0, 800.0, 600.0, srgb_rgba(0, 0, 0, 123));
        r.text(
            &a.font_big,
            "- PAUSED -",
            400.0,
            300.0,
            text_color(),
            Align::Center,
        );
    }
    r.render();
}

/// Celda de `maptiles.png` (dentro del tema) y, si lo hay, de `maptile_addons.png` de una casilla.
fn tile_cells(map: &Map, x: i32, y: i32) -> Option<(u32, Option<u32>)> {
    Some(match map.get(x, y) {
        Tile::None => return None,
        Tile::Ground => (0, None),
        Tile::Wall => (1, None),
        // Caja alcanzada: sprite_nr++ (MapTile_Box::destroy).
        Tile::Box => (if map.is_crumbling(x, y) { 3 } else { 2 }, None),
        Tile::Ice => (0, Some(0)),
        // Orden del original: DOWN=0, LEFT=1, UP=2, RIGHT=3; los addons de flecha empiezan en 1.
        Tile::Arrow(d) => (
            0,
            Some(
                1 + match d {
                    Dir::Down => 0,
                    Dir::Left => 1,
                    Dir::Up => 2,
                    Dir::Right => 3,
                },
            ),
        ),
        Tile::Trap => (0, Some(7)),
    })
}

fn draw_game(r: &mut Renderer, a: &Assets, w: &World, players: &[Player]) {
    // Tema visual (0..6): cada tema ocupa 4 celdas en maptiles.png.
    let theme = w.config.theme.clamp(0, 5) as u32;
    let t = TILE_PIXELS as f32;
    // El mapa mide 680x520; se centra en el lienzo virtual de 800x600 (offset 60,40 como el original).
    let (ox, oy) = (
        (cb_render::VIRTUAL_W - t * MAP_WIDTH as f32) / 2.0,
        (cb_render::VIRTUAL_H - t * MAP_HEIGHT as f32) / 2.0,
    );

    for y in 0..MAP_HEIGHT as i32 {
        for x in 0..MAP_WIDTH as i32 {
            let Some((base, addon)) = tile_cells(&w.map, x, y) else {
                continue;
            };
            let (mut px, mut py) = (ox + x as f32 * t, oy + y as f32 * t);
            if w.shake > 0 {
                // Temblor: cada casilla se desplaza entre -2 y 2 px al azar en cada fotograma.
                let h = (w.tick.wrapping_mul(2654435761) ^ (y * 31 + x) as u64 * 40503) as u32;
                px += (h % 5) as f32 - 2.0;
                py += ((h / 5) % 5) as f32 - 2.0;
            }
            // Una casilla que desaparece se encoge hacia su centro mientras se desvanece.
            let vanish = w
                .map
                .vanishing
                .iter()
                .find(|v| v.x as i32 == x && v.y as i32 == y);
            match vanish {
                Some(v) => {
                    let cd = v.left.clamp(0.0, 1.0);
                    let (sx, sy) = (
                        (px + 20.0 - 20.0 * cd).trunc(),
                        (py + 20.0 - 20.0 * cd).trunc(),
                    );
                    let size = [t * cd, t * cd];
                    let sheet = &a.maptiles;
                    r.sprite_tinted(
                        sheet.tex,
                        sheet.cell(theme * 4 + base),
                        [sx, sy, size[0], size[1]],
                        [1.0, 1.0, 1.0, cd],
                    );
                    if let Some(ad) = addon {
                        let sheet = &a.addons;
                        r.sprite_tinted(
                            sheet.tex,
                            sheet.cell(ad),
                            [sx, sy, size[0], size[1]],
                            [1.0, 1.0, 1.0, cd],
                        );
                    }
                }
                None => {
                    r.sheet_sprite(&a.maptiles, theme * 4 + base, px, py);
                    if let Some(ad) = addon {
                        r.sheet_sprite(&a.addons, ad, px, py);
                    }
                }
            }
        }
    }

    // Orden de z del original: explosiones (2000) < bombas (3000+y) < bombers (4000+y).
    let at = |tx: i32, ty: i32| (ox + (tx * 40) as f32, oy + (ty * 40) as f32);
    // Lo que cae (Z_FALLING = -1) queda por debajo de todo; luego extras (1000).
    for e in w.extras().filter(|e| e.falling) {
        draw_falling(
            r,
            &a.extras,
            e.kind as u32,
            (ox + e.x, oy + e.y),
            40.0,
            e.fall_countdown,
        );
    }
    for b in w.bombs().filter(|b| b.falling) {
        let cell = players[b.owner].skin as u32 * 4 + b.frame();
        draw_falling(
            r,
            &a.bombs,
            cell,
            (ox + b.x, oy + b.y),
            40.0,
            b.fall_countdown,
        );
    }
    for (b, p) in w.bombers.iter().zip(players) {
        let skin = &a.skins[p.skin];
        if !b.dead && b.is_falling() {
            let pos = (ox + b.get_x() as f32, oy + b.get_y() as f32);
            draw_falling(
                r,
                &skin.sheet,
                b.sprite_nr(),
                pos,
                skin.offset_y,
                b.fall_countdown,
            );
        }
    }
    // Cadáveres (Z_CORPSE = 100 + y): sobre el suelo, bajo los extras.
    for o in &w.objects {
        if let Object::Corpse(c) = o {
            let skin = &a.skins[players[c.owner].skin];
            let pos = (ox + c.get_x() as f32, oy + c.get_y() as f32);
            if c.falling {
                draw_falling(
                    r,
                    &skin.sheet,
                    c.sprite_nr,
                    pos,
                    skin.offset_y,
                    c.fall_countdown,
                );
            } else {
                r.sheet_sprite(
                    &skin.sheet,
                    c.sprite_nr,
                    c.get_x() as f32 + BOMBER_OFFSET_X,
                    c.get_y() as f32 + skin.offset_y,
                );
            }
        }
    }
    for e in w.extras().filter(|e| !e.falling && !e.flight.active) {
        r.sheet_sprite(&a.extras, e.kind as u32, ox + e.x, oy + e.y);
    }
    for e in w.explosions() {
        // Índices de segmento: Explosion.h (EXPLODE_LEFT..EXPLODE_X); +anim (0, 7 o 14).
        let (left, h, right, up, v, down, center) = (0, 1, 2, 3, 4, 5, 6);
        let an = e.anim();
        let (cx, cy) = at(e.tx, e.ty);
        r.sheet_sprite(&a.explosion, center + an, cx, cy);
        for i in 1..=e.left {
            let (px, py) = at(e.tx - i, e.ty);
            r.sheet_sprite(
                &a.explosion,
                if i == e.power { left } else { h } + an,
                px,
                py,
            );
        }
        for i in 1..=e.right {
            let (px, py) = at(e.tx + i, e.ty);
            r.sheet_sprite(
                &a.explosion,
                if i == e.power { right } else { h } + an,
                px,
                py,
            );
        }
        for i in 1..=e.up {
            let (px, py) = at(e.tx, e.ty - i);
            r.sheet_sprite(&a.explosion, if i == e.power { up } else { v } + an, px, py);
        }
        for i in 1..=e.down {
            let (px, py) = at(e.tx, e.ty + i);
            r.sheet_sprite(
                &a.explosion,
                if i == e.power { down } else { v } + an,
                px,
                py,
            );
        }
    }
    for b in w.bombs().filter(|b| !b.falling && !b.flight.active) {
        // sprite_nr = color * 4 + frame (color = skin del bomber que la puso).
        let cell = players[b.owner].skin as u32 * 4 + b.frame();
        r.sheet_sprite(&a.bombs, cell, ox + b.x, oy + b.y);
    }
    // Un bomber enfermo pulsa (Bomber::show): 2π cada ~314 ms (Timer::get_time() / 50).
    let ms = w.tick as f64 * 1000.0 / TICK_HZ as f64;
    let pulse = ((ms / 50.0) % (2.0 * std::f64::consts::PI)).sin() as f32 * 0.15 + 1.0;
    for (b, p) in w
        .bombers
        .iter()
        .zip(players)
        .filter(|(b, _)| !b.dead && !b.is_falling())
    {
        let skin = &a.skins[p.skin];
        if b.flight.active {
            // Entrada de ronda: sale volando desde el centro del mapa.
            let pos = (ox + b.x, b.y + skin.offset_y);
            draw_flying(r, &skin.sheet, b.sprite_nr(), pos, b.flight.scale(), 1.0);
        } else if b.is_diseased() {
            let pos = (ox + b.get_x() as f32, oy + b.get_y() as f32);
            draw_scaled(
                r,
                &skin.sheet,
                b.sprite_nr(),
                pos,
                skin.offset_y,
                pulse,
                1.0,
            );
        } else {
            r.sheet_sprite(
                &skin.sheet,
                b.sprite_nr(),
                b.get_x() as f32 + BOMBER_OFFSET_X,
                b.get_y() as f32 + skin.offset_y,
            );
        }
    }
    // Trozos de cadáver (Z_CORPSE_PART = 7000): por encima de los bombers.
    for o in &w.objects {
        if let Object::CorpsePart(p) = o {
            if !p.flight.active {
                let (x, y) = (p.x.trunc() + 60.0, p.y.trunc() + 40.0);
                let s = &a.corpse_parts;
                r.sprite_tinted(
                    s.tex,
                    s.cell(p.sprite_nr),
                    [x, y, 40.0, 40.0],
                    [1.0, 1.0, 1.0, p.opacity()],
                );
            }
        }
    }
    // Objetos en el aire (Z_FLYING): por encima de todo. Crecen hasta mitad de trayecto.
    for b in w.bombs().filter(|b| b.flight.active) {
        let cell = players[b.owner].skin as u32 * 4 + b.frame();
        draw_flying(
            r,
            &a.bombs,
            cell,
            (ox + b.x, oy + b.y),
            b.flight.scale(),
            1.0,
        );
    }
    for e in w.extras().filter(|e| e.flight.active) {
        // opacity_scaled de los extras es 0xBB.
        draw_flying(
            r,
            &a.extras,
            e.kind as u32,
            (ox + e.x, oy + e.y),
            e.flight.scale(),
            187.0 / 255.0,
        );
    }

    for o in &w.objects {
        if let Object::CorpsePart(p) = o {
            if p.flight.active {
                draw_flying(
                    r,
                    &a.corpse_parts,
                    p.sprite_nr,
                    (ox + p.x, oy + p.y),
                    p.flight.scale(),
                    1.0,
                );
            }
        }
    }

    draw_observer(r, a, w);
}

/// El observador, el reloj y el aviso de "HURRY UP" (`Observer::show`).
fn draw_observer(r: &mut Renderer, a: &Assets, w: &World) {
    let o = &w.observer;
    // Antes de agotarse el tiempo vive en la esquina (offset 10,2); después recorre el mapa.
    let (off_x, off_y) = if w.time_left() < 0.0 {
        (60.0, 40.0)
    } else {
        (10.0, 2.0)
    };
    if o.flight.active {
        draw_flying(
            r,
            &a.observer,
            0,
            (o.x + off_x, o.y + off_y),
            o.flight.scale(),
            1.0,
        );
    } else {
        r.sheet_sprite(&a.observer, 0, (o.x + off_x).trunc(), (o.y + off_y).trunc());
    }

    if w.time_left() > 0.0 {
        r.sheet_sprite(&a.observer, 1, 600.0, 1.0);
        let secs = (w.time_left() as i32).to_string();
        r.text(&a.font_big, &secs, 650.0, 3.0, text_color(), Align::Left);
    } else {
        r.sheet_sprite(&a.observer, 2, 600.0, 1.0);
    }
    if w.hurry_up() {
        r.quad(300.0, 250.0, 200.0, 30.0, [0.0, 0.0, 0.0, 0.5]);
        r.text(
            &a.font_small,
            "HURRY UP !!!!",
            400.0,
            255.0,
            text_color(),
            Align::Center,
        );
    }
}

/// Pantalla de marcador entre rondas (`GameStatus::draw`).
fn draw_scores(
    r: &mut Renderer,
    a: &Assets,
    w: &World,
    ui: &Ui,
    players: &[Player],
    winner: Option<usize>,
    end_of_game: bool,
) {
    let col = text_color();
    r.sprite(
        a.status_bg,
        [0.0, 0.0, 800.0, 600.0],
        [0.0, 0.0, 800.0, 600.0],
    );
    r.text(
        &a.font_small,
        "PRESS SPACE TO CONTINUE",
        400.0,
        570.0,
        col,
        Align::Center,
    );
    let title = match winner {
        // En una partida por equipos gana el equipo del último vivo.
        Some(i) if w.bombers[i].team != 0 => {
            format!("{} won", TEAM_NAMES[w.bombers[i].team as usize - 1])
        }
        Some(i) => format!("{} won", players[i].name),
        None => "Draw Game".to_owned(),
    };
    r.text(&a.font_big, &title, 500.0, 40.0, col, Align::Center);

    if w.is_team_game() {
        draw_team_rows(r, a, w, players, winner, end_of_game);
    } else {
        let ms = w.tick as f64 * 1000.0 / TICK_HZ as f64;
        for (i, (b, p)) in w.bombers.iter().zip(players).enumerate() {
            let (sheet, off) = (&a.skins[p.skin].sheet, a.skins[p.skin].offset_y);
            let nr = (i + 1) as f32;
            let row_y = 150.0 + nr * 43.0;
            r.text(
                &a.font_big,
                p.name.as_str(),
                70.0,
                157.0 + nr * 43.0,
                col,
                Align::Left,
            );
            let cup = |r: &mut Renderer, x: f32, y: f32, s: f32| {
                r.sprite_tinted(
                    a.cup.tex,
                    a.cup.cell(0),
                    [x, y, 40.0 * s, 40.0 * s],
                    [1.0; 4],
                );
            };
            if winner == Some(i) && !end_of_game {
                let x = 5.0 + ((i + 1) % 2) as f32 * 20.0;
                r.sheet_sprite(sheet, b.sprite_nr(), x, row_y - 40.0 + off);
                for k in 0..(b.points - 1).max(0) {
                    cup(r, 300.0 + k as f32 * 43.0, row_y, 1.0);
                }
                cup(
                    r,
                    (257.0 + b.points as f32 * 43.0).max(ui.scroll_in),
                    row_y,
                    1.0,
                );
            } else if winner == Some(i) {
                // El campeón de la partida: todo pulsa y se desliza hacia dentro.
                let sf = (((ms / 100.0).sin() + 2.0) / 2.0) as f32;
                let pos = (5.0 + (800.0 - ui.scroll_in) / 3.0, row_y);
                draw_scaled(r, sheet, b.sprite_nr(), pos, off, sf, 1.0);
                for k in 0..(b.points - 1).max(0) {
                    cup(
                        r,
                        320.0 + k as f32 * 43.0 - 20.0 * sf,
                        170.0 + nr * 43.0 - 20.0 * sf,
                        sf,
                    );
                }
                let x = (277.0 + b.points as f32 * 43.0 - 20.0 * sf).max(ui.scroll_in);
                cup(r, x, 170.0 + nr * 43.0 - 20.0 * sf, sf);
            } else {
                let x = 5.0 + ((i + 1) % 2) as f32 * 20.0;
                r.sheet_sprite(sheet, b.sprite_nr(), x, row_y - 40.0 + off);
                for k in 0..b.points {
                    cup(r, 300.0 + k as f32 * 43.0, row_y, 1.0);
                }
            }
        }
    }

    if end_of_game {
        r.text(&a.font_big, "the Match", 500.0, 80.0, col, Align::Center);
    } else {
        r.text(&a.font_small, "NEXT LEVEL", 785.0, 10.0, col, Align::Right);
        if let Some(name) = w.next_map_name() {
            r.text(&a.font_small, name, 785.0, 125.0, col, Align::Right);
        }
    }
}

/// `Map::show_preview`: el mapa entero en miniatura con esquina en `(x, y)` y casillas de
/// `40 * factor` píxeles. Las casillas `R` se enseñan como cajas.
pub fn draw_map_preview(
    r: &mut Renderer,
    a: &Assets,
    map: &Map,
    theme: u32,
    x: f32,
    y: f32,
    factor: f32,
    random_as_box: bool,
) {
    let tile = (40.0 * factor).trunc();
    for ty in 0..MAP_HEIGHT as i32 {
        for tx in 0..MAP_WIDTH as i32 {
            let cells = if random_as_box && map.random_boxes.contains(&(tx as usize, ty as usize)) {
                Some((2, None))
            } else {
                tile_cells(map, tx, ty)
            };
            let Some((base, addon)) = cells else { continue };
            let dst = [x + tx as f32 * tile, y + ty as f32 * tile, tile, tile];
            let s = &a.maptiles;
            r.sprite_tinted(s.tex, s.cell(theme.min(5) * 4 + base), dst, [1.0; 4]);
            if let Some(ad) = addon {
                let s = &a.addons;
                r.sprite_tinted(s.tex, s.cell(ad), dst, [1.0; 4]);
            }
        }
    }
}

const TEAM_NAMES: [&str; 4] = ["Blood Team", "Gold Team", "Water Team", "Leaf Team"];
const TEAM_COLORS: [(u8, u8, u8); 4] = [
    (255, 128, 128),
    (255, 255, 0),
    (128, 128, 255),
    (128, 255, 128),
];

/// Cuerpo del marcador por equipos (`GameStatus_Team::draw`): una ventana por equipo (de 380 px,
/// dos por fila) con sus miembros y las copas medias, y al final los jugadores sin equipo.
fn draw_team_rows(
    r: &mut Renderer,
    a: &Assets,
    w: &World,
    players: &[Player],
    winner: Option<usize>,
    end_of_game: bool,
) {
    let col = text_color();
    let ms = w.tick as f64 * 1000.0 / TICK_HZ as f64;
    // Las copas del campeón crecen y encogen; las demás son de media escala.
    let pulse = ((ms / 100.0).sin() + 2.0) as f32;
    let cup = |r: &mut Renderer, x: f32, y: f32, scale: f32| {
        r.sprite_tinted(
            a.cup.tex,
            a.cup.cell(0),
            [x, y, 40.0 * scale, 40.0 * scale],
            [1.0; 4],
        );
    };
    let member = |r: &mut Renderer, i: usize, x: f32, y: f32| {
        let skin = &a.skins[players[i].skin];
        draw_scaled(
            r,
            &skin.sheet,
            w.bombers[i].sprite_nr(),
            (x, y),
            skin.offset_y,
            0.5,
            1.0,
        );
    };

    let (mut wx, mut wy) = (0.0f32, 170.0f32);
    for team in 0..4u8 {
        let members: Vec<usize> = (0..w.bombers.len())
            .filter(|&i| w.bombers[i].team == team + 1)
            .collect();
        if members.is_empty() {
            continue;
        }
        let (tr, tg, tb) = TEAM_COLORS[team as usize];
        for (k, &i) in members.iter().enumerate() {
            let y = wy + 30.0 + k as f32 * 30.0;
            r.quad(wx, y, 380.0, 30.0, srgb_rgba(tr, tg, tb, 128));
            member(r, i, wx, wy + 25.0 + k as f32 * 30.0);
            r.text(
                &a.font_small,
                &players[i].name,
                wx + 35.0,
                wy + 35.0 + k as f32 * 30.0,
                col,
                Align::Left,
            );
        }
        // Lámpara del equipo, nombre y media de copas por jugador.
        r.sprite_tinted(
            a.team_lamps.tex,
            a.team_lamps.cell(team as u32),
            [wx, wy, 30.0, 32.0],
            [1.0; 4],
        );
        r.text(
            &a.font_small,
            TEAM_NAMES[team as usize],
            wx + 35.0,
            wy + 6.0,
            col,
            Align::Left,
        );
        let total: i32 = members.iter().map(|&i| w.bombers[i].points).sum();
        for k in 0..total / members.len() as i32 {
            cup(r, wx + 180.0 + k as f32 * 20.0, wy + 4.0, 0.5);
        }
        if end_of_game && winner.is_some_and(|i| w.bombers[i].team == team + 1) {
            cup(
                r,
                wx + 200.0 - 20.0 * pulse,
                wy + 70.0 - 20.0 * pulse,
                pulse,
            );
        }
        wx += 400.0;
        if wx == 800.0 {
            wx = 0.0;
            wy += 200.0;
        }
    }

    // Los que no tienen equipo, cada uno con sus propias copas.
    let mut row = 0.0f32;
    for i in (0..w.bombers.len()).filter(|&i| w.bombers[i].team == 0) {
        if row == 0.0 {
            r.text(
                &a.font_small,
                "Teamless Heroes",
                wx + 35.0,
                wy + 6.0,
                col,
                Align::Left,
            );
        }
        r.quad(
            wx,
            wy + 30.0 + row * 30.0,
            380.0,
            30.0,
            srgb_rgba(140, 140, 140, 128),
        );
        member(r, i, wx, wy + 25.0 + row * 30.0);
        r.text(
            &a.font_small,
            &players[i].name,
            wx + 32.0,
            wy + 35.0 + row * 30.0,
            col,
            Align::Left,
        );
        for j in 0..w.bombers[i].points {
            cup(r, wx + 180.0 + j as f32 * 20.0, wy + 34.0 + row * 30.0, 0.5);
        }
        if end_of_game && winner == Some(i) {
            cup(
                r,
                wx + 200.0 - 20.0 * pulse,
                wy + 34.0 + row * 30.0 - 20.0 * pulse,
                pulse,
            );
        }
        row += 1.0;
    }
}
