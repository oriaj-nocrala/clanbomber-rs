//! Dónde están los recursos (mapas, imágenes, sonidos, fuentes) en tiempo de ejecución.
//!
//! Se busca, por este orden: la variable de entorno `CB_ASSETS`; una carpeta `assets` junto al
//! ejecutable (o en su carpeta padre, como en un paquete descomprimido o en `target/release`);
//! `../share/clanbomber-rs/assets` (instalación); y la del repositorio, para `cargo run`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// ¿Parece una carpeta de recursos? (tiene los mapas)
fn looks_valid(dir: &Path) -> bool {
    dir.join("maps").is_dir() && dir.join("pics").is_dir()
}

fn candidates() -> Vec<PathBuf> {
    let mut c = Vec::new();
    if let Some(dir) = std::env::var_os("CB_ASSETS") {
        c.push(PathBuf::from(dir));
    }
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        c.push(exe_dir.join("assets"));
        c.push(exe_dir.join("../assets"));
        c.push(exe_dir.join("../share/clanbomber-rs/assets"));
        // `target/release/` dentro del repositorio.
        c.push(exe_dir.join("../../assets"));
    }
    c.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
    c
}

/// Carpeta de recursos del juego. Si no se encuentra ninguna válida, devuelve la del repositorio
/// (los errores de carga ya dicen qué archivo falta).
pub fn assets_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let all = candidates();
        all.iter()
            .find(|d| looks_valid(d))
            .or(all.last())
            .cloned()
            .unwrap_or_default()
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_repository_assets_when_running_tests() {
        let d = assets_dir();
        assert!(looks_valid(&d), "{d:?}");
        assert!(d.join("wavs/explode.wav").is_file());
        assert!(d.join("fonts/DejaVuSans-Bold.ttf").is_file());
    }
}
