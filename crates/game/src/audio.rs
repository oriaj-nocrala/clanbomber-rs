//! Efectos de sonido (ClanBomber2: Resources::*() y AudioSimpleSDL). Los WAV se decodifican una
//! vez al arrancar; si no hay dispositivo de audio el juego sigue en silencio.

use std::collections::HashMap;
use std::path::PathBuf;

use cb_core::GameEvent;
use rodio::buffer::SamplesBuffer;
use rodio::{Decoder, MixerDeviceSink, Source};

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
}

impl Sfx {
    const ALL: [Sfx; 18] = [
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
        }
    }
}

fn wavs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/wavs")
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
        Audio { device, buffers }
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
