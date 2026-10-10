//! Efectos de sonido (ClanBomber2: Resources::*() y AudioSimpleSDL). Los WAV se decodifican una
//! vez al arrancar; si no hay dispositivo de audio el juego sigue en silencio.

use std::collections::HashMap;
use std::path::PathBuf;

use cb_core::GameEvent;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, MixerDeviceSink, Player, Source};

/// Cada sonido del original.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    /// Pulsar una opción o entrar en un submenú (`Menu_clear`).
    Clear,
    /// Moverse por un menú (`Menu_break`).
    Break,
    CreditsStop,
    MenuBack,
    WinLevel,
    Typewriter,
    PutBomb,
    Explode,
    Die,
    DeepFall,
    Wow,
    Joint,
    Horny,
    Schnief,
    TimeOver,
    HurryUp,
    Crunch,
    Klatsch,
    Splash1,
    Splash2,
    CorpseExplode,
}

impl Sfx {
    const ALL: [Sfx; 21] = [
        Sfx::Clear,
        Sfx::Break,
        Sfx::CreditsStop,
        Sfx::MenuBack,
        Sfx::WinLevel,
        Sfx::Typewriter,
        Sfx::PutBomb,
        Sfx::Explode,
        Sfx::Die,
        Sfx::DeepFall,
        Sfx::Wow,
        Sfx::Joint,
        Sfx::Horny,
        Sfx::Schnief,
        Sfx::TimeOver,
        Sfx::HurryUp,
        Sfx::Crunch,
        Sfx::Klatsch,
        Sfx::Splash1,
        Sfx::Splash2,
        Sfx::CorpseExplode,
    ];

    pub fn file(self) -> &'static str {
        match self {
            Sfx::Clear => "clear.wav",
            Sfx::Break => "break.wav",
            Sfx::CreditsStop => "stop.wav",
            Sfx::MenuBack => "menu_back.wav",
            Sfx::WinLevel => "winlevel.wav",
            Sfx::Typewriter => "typewriter.wav",
            Sfx::PutBomb => "putbomb.wav",
            Sfx::Explode => "explode.wav",
            Sfx::Die => "die.wav",
            Sfx::DeepFall => "deepfall.wav",
            Sfx::Wow => "wow.wav",
            Sfx::Joint => "joint.wav",
            Sfx::Horny => "horny.wav",
            Sfx::Schnief => "schnief.wav",
            Sfx::TimeOver => "time_over.wav",
            Sfx::HurryUp => "hurry_up.wav",
            Sfx::Crunch => "crunch.wav",
            Sfx::Klatsch => "klatsch.wav",
            Sfx::Splash1 => "splash1a.wav",
            Sfx::Splash2 => "splash2a.wav",
            Sfx::CorpseExplode => "corpse_explode.wav",
        }
    }
}

impl From<GameEvent> for Sfx {
    fn from(e: GameEvent) -> Sfx {
        match e {
            GameEvent::PutBomb => Sfx::PutBomb,
            GameEvent::Explode => Sfx::Explode,
            GameEvent::Die => Sfx::Die,
            GameEvent::DeepFall => Sfx::DeepFall,
            GameEvent::Wow => Sfx::Wow,
            GameEvent::Joint => Sfx::Joint,
            GameEvent::Horny => Sfx::Horny,
            GameEvent::Schnief => Sfx::Schnief,
            GameEvent::TimeOver => Sfx::TimeOver,
            GameEvent::HurryUp => Sfx::HurryUp,
            GameEvent::Crunch => Sfx::Crunch,
            GameEvent::Klatsch => Sfx::Klatsch,
            GameEvent::Splash(0) => Sfx::Splash1,
            GameEvent::Splash(_) => Sfx::Splash2,
            GameEvent::CorpseExplode => Sfx::CorpseExplode,
        }
    }
}

fn wavs_dir() -> PathBuf {
    crate::paths::assets_dir().join("wavs")
}

/// Decodifica todos los efectos a memoria.
fn load_buffers() -> HashMap<Sfx, SamplesBuffer> {
    let mut map = HashMap::new();
    for sfx in Sfx::ALL {
        let path = wavs_dir().join(sfx.file());
        let decoded = std::fs::File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|f| Decoder::try_from(f).map_err(|e| e.to_string()));
        match decoded {
            Ok(dec) => {
                let (channels, rate) = (dec.channels(), dec.sample_rate());
                let samples: Vec<f32> = dec.collect();
                map.insert(sfx, SamplesBuffer::new(channels, rate, samples));
            }
            Err(e) => eprintln!("no se pudo cargar {path:?}: {e}"),
        }
    }
    map
}

pub struct Audio {
    /// Mientras viva, suena; sin dispositivo es `None`.
    device: Option<MixerDeviceSink>,
    buffers: HashMap<Sfx, SamplesBuffer>,
    music: Option<Player>,
}

impl Audio {
    pub fn new() -> Audio {
        let device = rodio::DeviceSinkBuilder::open_default_sink()
            .map_err(|e| eprintln!("sin audio: {e}"))
            .ok();
        let buffers = if device.is_some() {
            load_buffers()
        } else {
            HashMap::new()
        };
        Audio {
            device,
            buffers,
            music: None,
        }
    }

    /// Empieza o para la música de fondo.
    pub fn set_music(&mut self, on: bool) {
        if !on {
            if let Some(p) = self.music.take() {
                p.stop();
            }
            return;
        }
        if self.music.is_some() {
            return;
        }
        let Some(dev) = &self.device else { return };
        // bud.mod renderizado a Ogg Vorbis (assets/wavs/bud.ogg): suena igual en todos los sistemas
        // sin necesitar un reproductor de módulos. Se repite hasta que se apaga (`playLoop`).
        let path = wavs_dir().join("bud.ogg");
        match std::fs::File::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|f| {
                Decoder::new_looped(std::io::BufReader::new(f)).map_err(|e| e.to_string())
            }) {
            Ok(src) => {
                let player = Player::connect_new(dev.mixer());
                player.append(src);
                self.music = Some(player);
            }
            Err(e) => eprintln!("no se pudo cargar la música {path:?}: {e}"),
        }
    }

    pub fn play(&self, sfx: Sfx) {
        if let (Some(dev), Some(buf)) = (&self.device, self.buffers.get(&sfx)) {
            dev.mixer().add(buf.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_decodes_and_is_not_empty() {
        let buffers = load_buffers();
        for sfx in Sfx::ALL {
            let b = buffers
                .get(&sfx)
                .unwrap_or_else(|| panic!("{sfx:?} no carga"));
            assert!(b.clone().count() > 100, "{sfx:?} vacío");
        }
    }

    #[test]
    fn the_music_decodes_to_audible_stereo_audio_and_loops() {
        let path = wavs_dir().join("bud.ogg");
        let open = || std::io::BufReader::new(std::fs::File::open(&path).expect("bud.ogg"));
        let plain = Decoder::new(open()).expect("decodifica");
        assert_eq!(plain.channels().get(), 2);
        let first_secs: Vec<f32> = plain.take(44_100 * 2 * 5).collect();
        let peak = first_secs.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "silencio: pico {peak}");
        assert!(first_secs.iter().all(|s| s.is_finite()));

        // La canción dura 156 s: pedir bastante más obliga a volver al principio.
        let looped = Decoder::new_looped(open()).expect("en bucle");
        let n = 44_100 * 2 * 170;
        assert_eq!(looped.take(n).count(), n, "no se acaba");
    }

    #[test]
    fn every_game_event_maps_to_a_sound() {
        use GameEvent::*;
        for e in [
            PutBomb, Explode, Die, DeepFall, Wow, Joint, Horny, Schnief, TimeOver, HurryUp, Crunch,
            Klatsch,
        ] {
            assert!(Sfx::ALL.contains(&Sfx::from(e)));
        }
    }
}
