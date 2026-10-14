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
    /// DejaVuSans-Bold a 28 y a 18 px (Resources::Font_big / Font_small).
    pub font_big: Font,
    pub font_small: Font,
}

/// Las hojas redibujadas con IA tienen 4 texels por píxel virtual.
const HD_SCALE: f32 = 4.0;

/// Versión redibujada con IA de `name` (`<nombre>_hd.png`), con filtrado bilineal porque casi
/// siempre se dibuja más pequeña que la textura. `None` si no existe o con CB_SD=1 (originales).
fn load_hd(r: &mut Renderer, dir: &std::path::Path, name: &str) -> Option<TextureId> {
    if std::env::var_os("CB_SD").is_some() {
        return None;
    }
    let path = dir.join("pics").join(name.replace(".png", "_hd.png"));
    r.load_texture_smooth(path).ok()
}

/// Textura de una hoja de sprites y su escala: la HD si la hay, si no la original.
fn load_sheet_tex(r: &mut Renderer, dir: &std::path::Path, name: &str) -> (TextureId, f32) {
    match load_hd(r, dir, name) {
        Some(t) => (t, HD_SCALE),
        None => {
            let path = dir.join("pics").join(name);
            let t = r
                .load_texture(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            (t, 1.0)
        }
    }
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
            .map(|(f, h, o)| {
                let (t, scale) = load_sheet_tex(r, &dir, f);
                (t, *h, *o, scale)
            })
            .collect();
        // Los fondos HD pueden tener cualquier tamaño 4:3 (`Renderer::fullscreen`).
        let title_hd = load_hd(r, &dir, "clanbomber_title_andi.png");
        let cb_logo_hd = load_hd(r, &dir, "cb_logo_small.png");
        let not_available_hd = load_hd(r, &dir, "not_available.png");
        let [status_hd, player_setup_hd, map_select_hd, credits_hd, editor_hd] = [
            "game_status.png",
            "playersetup.png",
            "level_selection.png",
            "horst_evil.png",
            "map_editor.png",
        ]
        .map(|f| load_hd(r, &dir, f));
        let mut sheet = |name: &str| load_sheet_tex(r, &dir, name);
        let maptiles = sheet("maptiles.png");
        let addons = sheet("maptile_addons.png");
        let bombs = sheet("bombs.png");
        let extras = sheet("extras2.png");
        let observer = sheet("observer.png");
        let cup = sheet("cup2.png");
        let corpse_parts = sheet("corpse_parts.png");
        let ps_teams = sheet("ps_teams.png");
        let team_lamps = sheet("ps_teamlamps.png");
        let ps_controls = sheet("ps_controls.png");
        let explosion = sheet("explosion2.png");
        let mut tex = |name: &str| {
            let path = dir.join("pics").join(name);
            r.load_texture(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        };
        let status_bg = status_hd.unwrap_or_else(|| tex("game_status.png"));
        let title_bg = title_hd.unwrap_or_else(|| tex("clanbomber_title_andi.png"));
        let player_setup_bg = player_setup_hd.unwrap_or_else(|| tex("playersetup.png"));
        let map_select_bg = map_select_hd.unwrap_or_else(|| tex("level_selection.png"));
        let credits_bg = credits_hd.unwrap_or_else(|| tex("horst_evil.png"));
        let editor_bg = editor_hd.unwrap_or_else(|| tex("map_editor.png"));
        let (not_available, cb_logo, fl_logo) = (
            not_available_hd.unwrap_or_else(|| tex("not_available.png")),
            cb_logo_hd.unwrap_or_else(|| tex("cb_logo_small.png")),
            tex("fischlustig_logo.png"),
        );

        let font = |r: &mut Renderer, px: f32| {
            let path = dir.join("fonts/DejaVuSans-Bold.ttf");
            r.load_font(&path, px)
                .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
        };
        let (font_big, font_small) = (font(r, 28.0), font(r, 18.0));

        Assets {
            maptiles: r.sheet_scaled(maptiles.0, 40.0, 40.0, maptiles.1),
            addons: r.sheet_scaled(addons.0, 40.0, 40.0, addons.1),
            bombs: r.sheet_scaled(bombs.0, 40.0, 40.0, bombs.1),
            explosion: r.sheet_scaled(explosion.0, 40.0, 40.0, explosion.1),
            extras: r.sheet_scaled(extras.0, 40.0, 40.0, extras.1),
            skins: skin_tex
                .into_iter()
                .map(|(t, h, o, scale)| Skin {
                    sheet: r.sheet_scaled(t, 40.0, h, scale),
                    offset_y: o,
                })
                .collect(),
            observer: r.sheet_scaled(observer.0, 40.0, 40.0, observer.1),
            cup: r.sheet_scaled(cup.0, 40.0, 40.0, cup.1),
            corpse_parts: r.sheet_scaled(corpse_parts.0, 40.0, 40.0, corpse_parts.1),
            status_bg,
            title_bg,
            player_setup_bg,
            map_select_bg,
            credits_bg,
            editor_bg,
            ps_teams: r.sheet_scaled(ps_teams.0, 125.0, 56.0, ps_teams.1),
            ps_controls: r.sheet_scaled(ps_controls.0, 125.0, 56.0, ps_controls.1),
            team_lamps: r.sheet_scaled(team_lamps.0, 30.0, 32.0, team_lamps.1),
            not_available,
            cb_logo,
            fl_logo,
            font_big,
            font_small,
        }
    }
}
