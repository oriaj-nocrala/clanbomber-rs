//! Opciones gráficas (efectos de la GPU). Van aparte de `Config`: no son reglas de la partida y
//! no deben entrar en la huella del estado del mundo (`World::state_hash`).

use std::path::{Path, PathBuf};

use cb_render::Effects;

use crate::menu::Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Video {
    pub bloom: bool,
    pub particles: bool,
    pub crt: bool,
    pub fire: bool,
    pub shockwaves: bool,
    pub sprite_fx: bool,
    pub lighting: bool,
}

impl Default for Video {
    fn default() -> Self {
        let e = Effects::default();
        Video {
            bloom: e.bloom,
            particles: e.particles,
            crt: e.crt,
            fire: e.fire,
            shockwaves: e.shockwaves,
            sprite_fx: e.sprite_fx,
            lighting: e.lighting,
        }
    }
}

impl Video {
    /// `video.txt`, junto a la configuración de la partida.
    pub fn default_path() -> Option<PathBuf> {
        Some(cb_core::Config::default_path()?.with_file_name("video.txt"))
    }

    /// `clave=valor` por línea; lo desconocido se ignora y queda el valor por defecto.
    pub fn load(path: &Path) -> Video {
        let mut v = Video::default();
        let Ok(text) = std::fs::read_to_string(path) else {
            return v;
        };
        for line in text.lines() {
            let Some((k, val)) = line.split_once('=') else {
                continue;
            };
            let Ok(n) = val.trim().parse::<i32>() else {
                continue;
            };
            match k.trim() {
                "bloom" => v.bloom = n != 0,
                "particles" => v.particles = n != 0,
                "crt" => v.crt = n != 0,
                "fire" => v.fire = n != 0,
                "shockwaves" => v.shockwaves = n != 0,
                "sprite_fx" => v.sprite_fx = n != 0,
                "lighting" => v.lighting = n != 0,
                _ => {}
            }
        }
        v
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let b = |x: bool| x as i32;
        std::fs::write(
            path,
            format!(
                "bloom={}\nparticles={}\ncrt={}\nfire={}\nshockwaves={}\nsprite_fx={}\nlighting={}\n",
                b(self.bloom),
                b(self.particles),
                b(self.crt),
                b(self.fire),
                b(self.shockwaves),
                b(self.sprite_fx),
                b(self.lighting)
            ),
        )
    }

    pub fn effects(&self) -> Effects {
        Effects {
            bloom: self.bloom,
            crt: self.crt,
            particles: self.particles,
            fire: self.fire,
            shockwaves: self.shockwaves,
            sprite_fx: self.sprite_fx,
            lighting: self.lighting,
        }
    }

    /// Valor de una entrada del menú gráfico, o `None` si la entrada no es de vídeo.
    pub fn get(&self, id: Id) -> Option<i32> {
        Some(match id {
            Id::Bloom => self.bloom as i32,
            Id::Particles => self.particles as i32,
            Id::Crt => self.crt as i32,
            Id::FireShader => self.fire as i32,
            Id::Shockwaves => self.shockwaves as i32,
            Id::SpriteFx => self.sprite_fx as i32,
            Id::Lighting => self.lighting as i32,
            _ => return None,
        })
    }

    /// Cambia una entrada del menú; devuelve `false` si no es de vídeo.
    pub fn set(&mut self, id: Id, v: i32) -> bool {
        match id {
            Id::Bloom => self.bloom = v != 0,
            Id::Particles => self.particles = v != 0,
            Id::Crt => self.crt = v != 0,
            Id::FireShader => self.fire = v != 0,
            Id::Shockwaves => self.shockwaves = v != 0,
            Id::SpriteFx => self.sprite_fx = v != 0,
            Id::Lighting => self.lighting = v != 0,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_disk() {
        let dir = std::env::temp_dir().join(format!("cb-video-{}", std::process::id()));
        let path = dir.join("video.txt");
        let v = Video {
            bloom: false,
            particles: true,
            crt: true,
            fire: false,
            shockwaves: true,
            sprite_fx: false,
            lighting: true,
        };
        v.save(&path).unwrap();
        assert_eq!(Video::load(&path), v);
        let _ = std::fs::remove_dir_all(dir);
    }
}
