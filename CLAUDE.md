# CLAUDE.md

Port de ClanBomber a Rust + wgpu. Primero paridad de juego con el original, después extras
(partículas por compute shader, reescalado/sprites con IA).

## Dependencias del sistema

Solo Linux: `alsa-lib` (sonido, rodio) y `systemd-libs` (libudev, mandos con gilrs) + `pkgconf`. En Arch:
`pacman -S alsa-lib pkgconf systemd-libs`. Windows y macOS no necesitan nada. (La música es `bud.ogg`, `bud.mod`
renderizado una vez con `openmpt123 --render` + `ffmpeg`; ya no hay dependencia de libopenmpt.)

## Comandos

```bash
cargo run -p cb-game                         # el juego (empieza en la intro); el binario se llama clanbomber-rs
cargo run -p cb-game -- --screenshots docs/img   # recorre las pantallas y guarda PNG + GIF (render offscreen)
cargo test                                  # incluye parseo de los 33 mapas
cargo fmt
```

## Estructura

- `crates/core` (`cb-core`): lógica pura, **sin dependencias gráficas ni de ventana**. Determinista, paso
  fijo (`TICK_HZ` = 60). Mapa (`map.rs`), movimiento por píxeles compartido por bombers y bombas
  (`movement.rs`, port de `GameObject::move_*`), IA (`ai.rs`), vuelo en arco (`flight.rs`), bombers, enfermedades,
  bombas/explosiones, extras y `World::step` (`sim.rs`).
- `crates/render` (`cb-render`): wgpu. Batcher de quads en resolución virtual 800x600 con letterbox,
  texturas (`load_texture`), hojas de sprites (`Sheet`), texto TrueType con atlas de glifos (`load_font`,
  `text`, vía `fontdue`), shader en `quad.wgsl`. La escena se dibuja en una textura `Rgba16Float`
  intermedia (al tamaño del letterbox) y `fx.rs` la compone en la ventana: partículas simuladas por compute
  (`particles.wgsl`, anillo de 16384; la CPU solo manda `Burst`s), bloom por compute (`bloom.wgsl`, a 400x300)
  y composición con CRT opcional, aberración al temblar, ondas expansivas, aire caliente y cámara (`Camera`: zoom, foco y desaturación
  dramática; `post.wgsl`).
  `quad.wgsl` tiene modos por vértice: normal, silueta (estelas), brillo de hielo, tono girando (enfermedad),
  reflejo (`sprite_reflection`: invertido y recortado a las casillas de hielo) y quemar/revelar
  (`sprite_burn`/`sprite_reveal`: el avance va en los 16 bits altos del modo y el centro en el color)
  y el cronómetro del HUD dibujado entero por SDF (`Renderer::stopwatch`, con "Sprite Effects").
  `light.rs`/`light.wgsl`: iluminación dinámica multiplicativa con sombras (raymarch sobre la rejilla de
  muros/cajas, `Renderer::light` + `Renderer::lighting`). `fire.rs`/`fire.wgsl`: explosiones
  procedurales (cruz por SDF + fbm, grosor según la potencia; `Renderer::fire`), intercaladas con los sprites. Sin compute shaders (GL antiguo) se
  ve como el original. Un test valida los WGSL con naga (la CI no tiene GPU).
- `crates/game` (`cb-game`): binario winit. Loop con acumulador de tiempo → `World::step` a 60 Hz → dibujo
  interpolado entre el último tick y el anterior (`interp.rs`: copia del mundo con posiciones a medio camino;
  `quad.wgsl` ajusta los vértices a píxeles físicos), para monitores de más de 60 Hz.
  `heat.rs`/`heat.wgsl` (render): máscara de calor de 400x300 que suma rectángulos suaves
  (`Renderer::heat`); `post.wgsl` desplaza la imagen con ruido que sube, sin filtrar y con algo de
  dispersión de color. `haze.rs` (juego) pone el calor: llamas, brasas que suben al apagarse y mechas.
  `dying.rs`: transición de la muerte (destello, el bomber se quema y el cadáver aparece por los mismos
  píxeles, y sube su silueta); va con "Sprite Effects".
  `weather.rs`: clima por tema (nieve, hojas, esporas, ceniza, ascuas, motas), simulado en CPU y dibujado
  antes de la iluminación para que la luz lo alumbre (las ascuas brillan solas, después); va con "Particles".
  `steps.rs`: polvo al frenar o girar y escarcha al patinar en hielo (compara posiciones entre ticks).
  `slowmo.rs`: cámara lenta en la muerte que decide la ronda; solo escala el tiempo que recibe el
  acumulador (los ticks son los mismos, determinista). Encuadra a la víctima, la bomba (la explosión
  que la alcanzó) y al asesino (a quien `kill_bombers` sumó la muerte), con focos de luz
  (`Camera::spots`) y su nombre encima. Si la ronda acaba en plena
  cámara lenta se congela el último fotograma hasta que termina. Va con la opción "Cinematic Effects".
  `assets.rs` carga texturas y fuentes; `draw.rs` dibuja mapa, HUD y marcador; `sparks.rs` compara el
  mundo con el fotograma anterior y lanza ráfagas de partículas (explosiones, cajas, muertes, mechas);
  `video.rs` son las opciones gráficas (bloom, partículas, CRT, explosiones por shader, ondas, calor y cámara lenta, efectos
  de sprites); `Ui` guarda las estelas de los bombers rápidos (crecen con `speed_level`); `lighting.rs` decide las
  luces (fuego, bombas que laten, halos de bombers, extras); `shards.rs` simula en CPU los trozos con textura (cajas y
  extras quemados, con altura falsa y rebote) y el "pop" de los extras recogidos, en `video.txt` aparte de `Config` para no
  entrar en `World::state_hash`; `audio.rs` (efectos y música; el núcleo solo emite `GameEvent`); `menu.rs` es el menú en árbol
  (opciones de `Config`, guardadas al cambiar); `intro.rs`, `player_setup.rs`, `map_select.rs`, `map_editor.rs`, `credits.rs` y `help.rs`
  son las demás pantallas de menú (`screen.rs`: `Flow` y `TextEdit`). Pantallas: menú ⇄ partida (Esc vuelve al menú; Espacio pasa
  del marcador a la siguiente ronda).
- `assets/maps`, `assets/pics`: copiados del fork de referencia (ver abajo).

## Referencia: el original

Especificación ejecutable del comportamiento: el fork C++ `viti95/ClanBomber2` clonado en
`/home/oriaj/c/clanbomber2/src` (compilado en `.../clanbomber2/dist/bin/clanbomber2`).
Ante cualquier duda de reglas (colisión, velocidades, explosiones, extras), **leer ese código y replicarlo**
antes de inventar. Cita el archivo del original en el comentario cuando copies una constante o regla.

Datos útiles del original:
- Mapa 17x13 tiles de 40 px; se dibuja con offset (60, 40) en el lienzo de 800x600.
- Enum de dirección: `DOWN=0, LEFT=1, UP=2, RIGHT=3`. Addons de flecha en `maptile_addons.png` = `dir+1`;
  hielo = 0, trampa = 7. `maptiles.png`: 4 celdas por tema (suelo, muro, caja, caja+1).
- Mapas: los del juego en `assets/maps` (solo lectura) y los del usuario en
  `~/.local/share/clanbomber-rs/maps` (los crea el editor). `MapDoc` es el mapa editable en caracteres.
- Formato `.map`: línea de autor, entero de jugadores, 13 filas de 17 caracteres.
  `' '` suelo, `*` muro, `+` caja, `R` caja con prob. 2/3, `S` hielo, `<>^v` flechas, `o` trampa,
  `0`-`7` spawns, resto = fuera del mapa.

## Decisiones

- **Replicar el movimiento del original tal cual** (píxeles enteros, velocidades enteras, snap a la rejilla)
  para poder comparar contra el fork. Limpiar/generalizar solo después de tener paridad.
- La simulación nunca toca wgpu/winit ni el reloj de pared; el render solo lee `World`.
- El C++ propio anterior (`/home/oriaj/c/clanbomber_sdl3`) se estancó por refactorizar la arquitectura antes de
  tener el contenido del juego. No repetir: paridad de contenido primero.

- **Aleatoriedad:** todo pasa por `cb_core::Rng` (misma recurrencia que `fast_random()`, pero de 32 bits y
  con semilla): `World::with_seed`. Nunca usar `rand` ni el reloj dentro de `core`.

## Notas de wgpu 30

La API difiere de la mayoría de tutoriales: `Queue::present(frame)`, `get_current_texture()` devuelve un
enum (`CurrentSurfaceTexture`), `VertexState.buffers` es `&[Option<_>]`, `bind_group_layouts` es
`&[Option<&_>]`, `Instance::new_without_display_handle()`. Ante errores de compilación, mirar el código en
`~/.cargo/registry/src/*/wgpu-30.*`.

## CI/CD

- `.github/workflows/ci.yml`: formato, Clippy (`-D warnings`), tests y compilación release en cada push y PR.
- `.github/workflows/release.yml`: al subir una etiqueta `vX.Y.Z` compila para Linux, Windows y macOS (arm64 e
  Intel), empaqueta cada binario con `assets/` y publica la versión. Los tests y la compilación de CI también
  corren en los tres sistemas. Los recursos se buscan en tiempo de ejecución (`crates/game/src/paths.rs`: `CB_ASSETS`, junto al
  ejecutable, `../share/...`, o el repo).
- Antes de subir: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test`.
- El README principal está en inglés; el español va en un `<details>` al final del mismo archivo.

## Convenciones

- Código, comentarios y commits en español; identificadores en inglés.
- Commits temáticos y pequeños (un tema por commit). Licencia del proyecto: GPL-2.0-or-later.

## Hoja de ruta

1. ~~Mapa con texturas~~ 2. ~~Bomber~~ 3. ~~Bombas y explosiones~~
4. ~~Extras, enfermedades, hielo/flechas/trampas, ciclo de partida (rondas, reloj, marcador)~~
5. ~~Menús completos~~ (intro, principal, opciones, Player Setup, Map Selection, Map Editor, Credits, Help)
   → IA hecha (`ai.rs`, port de Controller_AI; la segunda IA del original, AI_mass, no se portó a propósito). Sonido hecho (efectos con rodio y
   música Ogg en bucle; F2 la activa, P pausa), cadáveres, trozos, sangre en los créditos y
   temblor de explosiones. Equipos hechos (rondas por equipo, marcador de equipos, la IA no vuela a compañeros). Mandos hechos (gilrs: stick o cruceta + botón A).
   No se portaron a propósito la segunda IA (AI_mass) ni el controlador de ratón (RCMOUSE).
6. ~~Partículas (compute)~~, bloom y CRT hechos (Options → Graphic Options). Pendiente: sprites con IA.
