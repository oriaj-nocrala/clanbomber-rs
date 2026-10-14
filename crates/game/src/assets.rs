//! Carga de texturas, hojas de sprites y fuentes.

use cb_render::{Font, Renderer, Sheet, TextureId};

/// Aspecto de bomber: hoja de sprites y desplazamiento vertical de dibujo (`Bomber::Bomber`).
pub struct Skin {
    pub sheet: Sheet,
    pub offset_y: f32,
}

pub struct Assets {
    pub maptiles: Sheet,
    pub addons: Sheet,
    /// Los 8 aspectos, en el orden de `Resources::Bombers`.
    pub skins: Vec<Skin>,
    /// bombs.png: 4 frames por color de bomber (celdas de 40x40, 8 columnas).
    pub bombs: Sheet,
    /// explosion2.png: 7 columnas (tipo de segmento) x 3 filas (frame de animación).
    pub explosion: Sheet,
    /// extras2.png: celdas de 40x40, índice = ExtraKind.
    pub extras: Sheet,
    /// observer.png: 0 = observador, 1 = reloj, 2 = reloj agotado.
    pub observer: Sheet,
    /// cup2.png: la copa del marcador.
    pub cup: Sheet,
    /// corpse_parts.png: 4 trozos de 40x40.
    pub corpse_parts: Sheet,
    pub status_bg: TextureId,
    pub title_bg: TextureId,
    /// Fondos de las pantallas de menú (800x600).
    pub player_setup_bg: TextureId,
    pub map_select_bg: TextureId,
    pub credits_bg: TextureId,
    pub editor_bg: TextureId,
    /// ps_teams.png y ps_controls.png: 5 celdas de 125x56 en vertical.
    pub ps_teams: Sheet,
    pub ps_controls: Sheet,
    /// ps_teamlamps.png: la lámpara de cada equipo (4 celdas de 30x32).
    pub team_lamps: Sheet,
    pub not_available: TextureId,
    pub cb_logo: TextureId,
    pub fl_logo: TextureId,
    /// Disco blanco que se desvanece hacia el borde (halos de luz, llamas).
    pub glow: TextureId,
    /// DejaVuSans-Bold a 28 y a 18 px (Resources::Font_big / Font_small).
    pub font_big: Font,
    pub font_small: Font,
}

/// Las hojas están redibujadas con IA a 4 texels por píxel virtual (`tools/sprites`).
const SCALE: f32 = 4.0;

/// Una imagen de `assets/pics`, con filtrado bilineal porque casi siempre se dibuja más pequeña
/// que la textura.
fn load_pic(r: &mut Renderer, dir: &std::path::Path, name: &str) -> TextureId {
    let path = dir.join("pics").join(name);
    r.load_texture_smooth(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

impl Assets {
    pub fn load(r: &mut Renderer) -> Assets {
        let dir = crate::paths::assets_dir();
        // (archivo, alto de celda, offset_y): los de 60 px de alto se dibujan a y + 11.
        let skin_files = [
            ("bomber_snake.png", 60.0, 11.0),
            ("bomber_tux.png", 60.0, 11.0),
            ("bomber_spider.png", 40.0, 40.0),
            ("bomber_bsd.png", 60.0, 11.0),
            ("bomber_dull_red.png", 40.0, 40.0),
            ("bomber_dull_blue.png", 40.0, 40.0),
            ("bomber_dull_yellow.png", 40.0, 40.0),
            ("bomber_dull_green.png", 40.0, 40.0),
        ];
        let skin_tex: Vec<_> = skin_files
            .iter()
            .map(|(f, h, o)| (load_pic(r, &dir, f), *h, *o))
            .collect();
        let mut pic = |name: &str| load_pic(r, &dir, name);
        let maptiles = pic("maptiles.png");
        let addons = pic("maptile_addons.png");
        let bombs = pic("bombs.png");
        let extras = pic("extras2.png");
        let observer = pic("observer.png");
        let cup = pic("cup2.png");
        let corpse_parts = pic("corpse_parts.png");
        let ps_teams = pic("ps_teams.png");
        let team_lamps = pic("ps_teamlamps.png");
        let ps_controls = pic("ps_controls.png");
        let explosion = pic("explosion2.png");
        // Los fondos pueden tener cualquier tamaño 4:3 (`Renderer::fullscreen`).
        let status_bg = pic("game_status.png");
        let title_bg = pic("clanbomber_title_andi.png");
        let player_setup_bg = pic("playersetup.png");
        let map_select_bg = pic("level_selection.png");
        let credits_bg = pic("horst_evil.png");
        let editor_bg = pic("map_editor.png");
        let not_available = pic("not_available.png");
        let cb_logo = pic("cb_logo_small.png");
        // El logo de Fischlustig (los autores del original) se queda como era, sin filtrar.
        let fl_logo = {
            let path = dir.join("pics/fischlustig_logo.png");
            r.load_texture(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        };

        let glow = {
            const N: u32 = 64;
            let rgba: Vec<u8> = (0..N * N)
                .flat_map(|i| {
                    let (x, y) = ((i % N) as f32 + 0.5, (i / N) as f32 + 0.5);
                    let d = ((x - N as f32 / 2.0).hypot(y - N as f32 / 2.0) / (N as f32 / 2.0))
                        .min(1.0);
                    let a = (1.0 - d * d).powi(2);
                    [255, 255, 255, (a * 255.0).round() as u8]
                })
                .collect();
            r.load_rgba_smooth(&rgba, N, N)
        };

        let font = |r: &mut Renderer, px: f32| {
            let path = dir.join("fonts/DejaVuSans-Bold.ttf");
            r.load_font(&path, px)
                .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
        };
        let (font_big, font_small) = (font(r, 28.0), font(r, 18.0));

        Assets {
            maptiles: r.sheet_scaled(maptiles, 40.0, 40.0, SCALE),
            addons: r.sheet_scaled(addons, 40.0, 40.0, SCALE),
            bombs: r.sheet_scaled(bombs, 40.0, 40.0, SCALE),
            explosion: r.sheet_scaled(explosion, 40.0, 40.0, SCALE),
            extras: r.sheet_scaled(extras, 40.0, 40.0, SCALE),
            skins: skin_tex
                .into_iter()
                .map(|(t, h, o)| Skin {
                    sheet: r.sheet_scaled(t, 40.0, h, SCALE),
                    offset_y: o,
                })
                .collect(),
            observer: r.sheet_scaled(observer, 40.0, 40.0, SCALE),
            cup: r.sheet_scaled(cup, 40.0, 40.0, SCALE),
            corpse_parts: r.sheet_scaled(corpse_parts, 40.0, 40.0, SCALE),
            status_bg,
            title_bg,
            player_setup_bg,
            map_select_bg,
            credits_bg,
            editor_bg,
            ps_teams: r.sheet_scaled(ps_teams, 125.0, 56.0, SCALE),
            ps_controls: r.sheet_scaled(ps_controls, 125.0, 56.0, SCALE),
            team_lamps: r.sheet_scaled(team_lamps, 30.0, 32.0, SCALE),
            not_available,
            cb_logo,
            fl_logo,
            glow,
            font_big,
            font_small,
        }
    }
}
