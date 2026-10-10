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

/// Música de módulo (`bud.mod`) renderizada con libopenmpt, en bucle (`AudioSimpleSDL::playLoop`).
struct ModSource {
    module: openmpt::module::Module,
    buf: Vec<f32>,
    pos: usize,
}

const MUSIC_RATE: u32 = 44_100;
/// Fotogramas por bloque de render.
const MUSIC_CHUNK: usize = 4096;

// SAFETY: el módulo de libopenmpt se usa siempre desde un único hilo a la vez (el del mezclador);
// no comparte estado mutable con ningún otro objeto.
unsafe impl Send for ModSource {}

impl ModSource {
    fn open(path: &std::path::Path) -> Option<Self> {
        let mut data = std::fs::read(path).ok()?;
        let module = openmpt::module::Module::create_from_memory(
            &mut data,
            openmpt::module::Logger::None,
            &[],
        )
        .ok()?;
        Some(ModSource {
            module,
            buf: Vec::with_capacity(MUSIC_CHUNK * 2),
            pos: 0,
        })
    }

    /// Renderiza el siguiente bloque; al terminar la canción vuelve al principio.
    fn refill(&mut self) -> Option<()> {
        for _ in 0..2 {
            self.buf.clear();
            let frames = self
                .module
                .read_interleaved_float_stereo(MUSIC_RATE as i32, &mut self.buf);
            if frames > 0 {
                // SAFETY: libopenmpt ha escrito `frames * 2` muestras en la capacidad reservada.
                unsafe { self.buf.set_len(frames * 2) };
                self.pos = 0;
                return Some(());
            }
            self.module.set_position_seconds(0.0);
        }
        None
    }
}

impl Iterator for ModSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pos >= self.buf.len() {
            self.refill()?;
        }
        let s = self.buf[self.pos];
        self.pos += 1;
        Some(s)
    }
}

impl Source for ModSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        rodio::nz!(2)
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        std::num::NonZero::new(MUSIC_RATE).expect("frecuencia no nula")
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
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
        let path = wavs_dir().join("bud.mod");
        match ModSource::open(&path) {
            Some(src) => {
                let player = Player::connect_new(dev.mixer());
                player.append(src);
                self.music = Some(player);
            }
            None => eprintln!("no se pudo cargar la música {path:?}"),
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
    fn the_module_renders_audible_stereo_audio_and_loops() {
        let mut src = ModSource::open(&wavs_dir().join("bud.mod")).expect("bud.mod");
        // Unos segundos de audio: tiene que haber señal y nunca terminarse.
        let n = MUSIC_RATE as usize * 2 * 5;
        let samples: Vec<f32> = (&mut src).take(n).collect();
        assert_eq!(samples.len(), n);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "silencio: pico {peak}");
        assert!(samples.iter().all(|s| s.is_finite()));
        // Forzar el final de la canción: debe reiniciarse en vez de acabar.
        src.module.set_position_seconds(1e6);
        assert!(src.next().is_some(), "en bucle");
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
