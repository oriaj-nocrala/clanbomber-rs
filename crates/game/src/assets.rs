//! Carga de texturas, hojas de sprites y fuentes.

use cb_render::{Font, Renderer, Sheet, TextureId};

pub struct Assets {
    pub maptiles: Sheet,
    pub addons: Sheet,
    /// Skins 0 (snake) y 1 (tux): celdas de 40x60 (Resources::Bombers).
    pub bombers: [Sheet; 2],
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
    pub status_bg: TextureId,
    /// DejaVuSans-Bold a 28 y a 18 px (Resources::Font_big / Font_small).
    pub font_big: Font,
    pub font_small: Font,
}

fn assets_dir() -> String {
    format!("{}/../../assets", env!("CARGO_MANIFEST_DIR"))
}

impl Assets {
    pub fn load(r: &mut Renderer) -> Assets {
        let dir = assets_dir();
        let mut tex = |name: &str| {
            let path = format!("{dir}/pics/{name}");
            r.load_texture(&path)
                .unwrap_or_else(|e| panic!("{path}: {e}"))
        };
        let (maptiles, addons) = (tex("maptiles.png"), tex("maptile_addons.png"));
        let (bombs, explosion, extras) =
            (tex("bombs.png"), tex("explosion2.png"), tex("extras2.png"));
        let (snake, tux) = (tex("bomber_snake.png"), tex("bomber_tux.png"));
        let (observer, cup, status_bg) =
            (tex("observer.png"), tex("cup2.png"), tex("game_status.png"));

        let font = |r: &mut Renderer, px: f32| {
            let path = format!("{dir}/fonts/DejaVuSans-Bold.ttf");
            r.load_font(&path, px)
                .unwrap_or_else(|e| panic!("{path}: {e:?}"))
        };
        let (font_big, font_small) = (font(r, 28.0), font(r, 18.0));

        Assets {
            maptiles: r.sheet(maptiles, 40.0, 40.0),
            addons: r.sheet(addons, 40.0, 40.0),
            bombs: r.sheet(bombs, 40.0, 40.0),
            explosion: r.sheet(explosion, 40.0, 40.0),
            extras: r.sheet(extras, 40.0, 40.0),
            bombers: [r.sheet(snake, 40.0, 60.0), r.sheet(tux, 40.0, 60.0)],
            observer: r.sheet(observer, 40.0, 40.0),
            cup: r.sheet(cup, 40.0, 40.0),
            status_bg,
            font_big,
            font_small,
        }
    }
}
