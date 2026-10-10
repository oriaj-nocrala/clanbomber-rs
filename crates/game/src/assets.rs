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

impl Assets {
    pub fn load(r: &mut Renderer) -> Assets {
        let dir = crate::paths::assets_dir();
        let mut tex = |name: &str| {
            let path = dir.join("pics").join(name);
            r.load_texture(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        };
        let (maptiles, addons) = (tex("maptiles.png"), tex("maptile_addons.png"));
        let (bombs, explosion, extras) =
            (tex("bombs.png"), tex("explosion2.png"), tex("extras2.png"));
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
            .map(|(f, h, o)| (tex(f), *h, *o))
            .collect();
        let (observer, cup) = (tex("observer.png"), tex("cup2.png"));
        let corpse_parts = tex("corpse_parts.png");
        let (status_bg, title_bg) = (tex("game_status.png"), tex("clanbomber_title_andi.png"));
        let (player_setup_bg, map_select_bg, credits_bg) = (
            tex("playersetup.png"),
            tex("level_selection.png"),
            tex("horst_evil.png"),
        );
        let editor_bg = tex("map_editor.png");
        let (ps_teams, ps_controls) = (tex("ps_teams.png"), tex("ps_controls.png"));
        let team_lamps = tex("ps_teamlamps.png");
        let (not_available, cb_logo, fl_logo) = (
            tex("not_available.png"),
            tex("cb_logo_small.png"),
            tex("fischlustig_logo.png"),
        );

        let font = |r: &mut Renderer, px: f32| {
            let path = dir.join("fonts/DejaVuSans-Bold.ttf");
            r.load_font(&path, px)
                .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
        };
        let (font_big, font_small) = (font(r, 28.0), font(r, 18.0));

        Assets {
            maptiles: r.sheet(maptiles, 40.0, 40.0),
            addons: r.sheet(addons, 40.0, 40.0),
            bombs: r.sheet(bombs, 40.0, 40.0),
            explosion: r.sheet(explosion, 40.0, 40.0),
            extras: r.sheet(extras, 40.0, 40.0),
            skins: skin_tex
                .into_iter()
                .map(|(t, h, o)| Skin {
                    sheet: r.sheet(t, 40.0, h),
                    offset_y: o,
                })
                .collect(),
            observer: r.sheet(observer, 40.0, 40.0),
            cup: r.sheet(cup, 40.0, 40.0),
            corpse_parts: r.sheet(corpse_parts, 40.0, 40.0),
            status_bg,
            title_bg,
            player_setup_bg,
            map_select_bg,
            credits_bg,
            editor_bg,
            ps_teams: r.sheet(ps_teams, 125.0, 56.0),
            ps_controls: r.sheet(ps_controls, 125.0, 56.0),
            team_lamps: r.sheet(team_lamps, 30.0, 32.0),
            not_available,
            cb_logo,
            fl_logo,
            font_big,
            font_small,
        }
    }
}
