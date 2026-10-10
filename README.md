<div align="center">

# 💣 ClanBomber RS

**El clásico Bomberman de ClanBomber 2, reescrito en Rust con wgpu.**
Hasta 8 jugadores, IA, mandos, 33 mapas, extras, enfermedades, hielo, trampas… y mucha sangre de píxeles.

[![Licencia: GPL v2+](https://img.shields.io/badge/licencia-GPL%20v2%2B-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)
![wgpu](https://img.shields.io/badge/gráficos-wgpu-5c2d91)
![Tests](https://img.shields.io/badge/tests-130-brightgreen)

<img src="docs/img/gameplay.gif" alt="Cuatro IAs jugando una ronda en FireWheels" width="640">

<sub>Cuatro IAs en <i>FireWheels</i>, con flechas que empujan bombas. Grabado con el propio juego (<code>--screenshots</code>).</sub>

</div>

---

## ✨ Qué es

ClanBomber es un Bomberman de pantalla compartida: laberintos de cajas y muros, bombas que explotan en cruz, extras
que te hacen más fuerte… o te ponen a tope de cocaína. Este proyecto es **un port fiel a Rust** del juego original
(el fork SDL de [viti95](https://github.com/viti95/ClanBomber2) fue la referencia ejecutable): mismas reglas, mismos
mapas, mismos gráficos y sonidos, pero con un núcleo de simulación limpio y testeado y un renderer moderno sobre
**wgpu** (Vulkan, Metal, DX12). Probado en Linux; macOS y Windows deberían funcionar, pero no se han probado.

<table>
<tr>
<td width="50%"><img src="docs/img/menu.png" alt="Menú principal"></td>
<td width="50%"><img src="docs/img/gameplay-ice.png" alt="Partida con tema de hielo"></td>
</tr>
<tr>
<td align="center"><sub>Menú principal, con la pantalla de título original</sub></td>
<td align="center"><sub>Arena helada: IAs, bombas, extras y fuego en cruz</sub></td>
</tr>
<tr>
<td width="50%"><img src="docs/img/gameplay-jungle.png" alt="Partida con tema de jungla"></td>
<td width="50%"><img src="docs/img/team-scoreboard.png" alt="Marcador por equipos"></td>
</tr>
<tr>
<td align="center"><sub>6 temas gráficos, aquí «Jungle Arena»</sub></td>
<td align="center"><sub>Marcador por equipos entre rondas</sub></td>
</tr>
<tr>
<td width="50%"><img src="docs/img/player-setup.png" alt="Configuración de jugadores"></td>
<td width="50%"><img src="docs/img/map-select.png" alt="Selección de mapa"></td>
</tr>
<tr>
<td align="center"><sub>8 puestos: aspecto, nombre, equipo y controlador</sub></td>
<td align="center"><sub>Selección de mapas con vista previa</sub></td>
</tr>
<tr>
<td width="50%"><img src="docs/img/map-editor.png" alt="Editor de mapas"></td>
<td width="50%"><img src="docs/img/scoreboard.png" alt="Marcador con copas"></td>
</tr>
<tr>
<td align="center"><sub>Editor de mapas integrado</sub></td>
<td align="center"><sub>Marcador con copas</sub></td>
</tr>
</table>

## 🎮 Características

- **Hasta 8 jugadores**: teclados (3 esquemas), mandos (stick o cruceta + botón A) e **IA** que huye del fuego, recoge
  extras, evita las enfermedades y abre camino a bombazos sin suicidarse.
- **Todos los extras**: más bombas, más alcance, patines, **patada** (empuja bombas), **guante** (lanza bombas por el
  aire) y las enfermedades **Joint** (controles invertidos), **Viagra** (bombas sin parar) y **Koks** (velocidad
  absurda), que se contagian por contacto.
- **Terreno con trucos**: hielo que te hace resbalar, flechas que empujan bombas, trampas que las disparan por encima
  de los muros, huecos en los que caes.
- **Rondas completas**: entrada volando al mapa, reloj de ronda, aviso *HURRY UP* y, al acabarse el tiempo, un
  observador que se va comiendo el suelo casilla a casilla. Marcador con copas y modo por **equipos**.
- **33 mapas** del original, **editor de mapas** propio, selección con vista previa y 6 temas.
- **Cadáveres** que se arrastran y revientan en trozos, temblor de explosiones, sangre en los créditos y modo
  infantil (*Kidz Mode*) para apagarlo todo.
- **Sonido y música**: efectos del original y el módulo `bud.mod` (F2) renderizado con libopenmpt.
- **Todo configurable** desde el menú: extras iniciales y máximos, tiempos de bomba, qué extras caen, puntos para
  ganar, tiempo de ronda…
- **Determinista**: la simulación es de paso fijo (60 Hz) y toda la aleatoriedad pasa por un generador con semilla.
  Misma semilla, misma partida.

## ⌨️ Controles

| | Izquierda / Derecha / Arriba / Abajo | Bomba |
|---|---|---|
| **Teclado 1** | ← → ↑ ↓ | Enter |
| **Teclado 2** | A D W S | Tab |
| **Teclado 3** | J L I K | Espacio |
| **Mando** | Stick izquierdo o cruceta | A |

**Bomba doble sin moverte** (con guante): lanza la bomba. **Caminar contra una bomba** (con patada): la empuja.

| Tecla | Acción |
|---|---|
| Esc | Salir de la partida al menú / subir un nivel |
| Espacio | Pasar del marcador a la siguiente ronda |
| P / F3 | Pausa |
| F2 | Música de fondo |

## 🚀 Compilar y jugar

Necesitas [Rust](https://rustup.rs) (edición 2021) y, para el sonido y los mandos, las librerías del sistema:

```bash
# Arch Linux
sudo pacman -S alsa-lib libopenmpt pkgconf systemd-libs
# Debian / Ubuntu
sudo apt install libasound2-dev libopenmpt-dev pkg-config libudev-dev
```

```bash
git clone https://github.com/oriaj-nocrala/clanbomber-rs.git
cd clanbomber-rs
cargo run --release -p cb-game
```

La configuración se guarda en `~/.config/clanbomber-rs/config.txt` y tus mapas en
`~/.local/share/clanbomber-rs/maps/`.

### Sacar capturas

```bash
cargo run --release -p cb-game -- --screenshots docs/img
```

Recorre las pantallas del juego, guarda un PNG de cada una y monta un GIF de una partida entre IAs (necesita
ImageMagick). Renderiza fuera de pantalla: no captura tu escritorio.

## 🧱 Arquitectura

```
crates/
├── core/     cb-core    Simulación pura: mapa, bombers, bombas, extras, IA, rondas… Sin gráficos ni audio.
├── render/   cb-render  Renderer 2D sobre wgpu: lotes de quads, atlas de texto, captura a PNG.
└── game/     cb-game    Ventana (winit), menús, dibujo, audio, mandos y el bucle de juego.
```

El núcleo no sabe de ventanas ni de sonido: emite `GameEvent`s que el binario convierte en audio, y el renderer solo
lee el estado del `World`. Eso permite probarlo todo sin GPU:

```bash
cargo test        # 130 tests: reglas, IA, rondas, equipos, menús, mandos…
```

Cada regla del juego cita en un comentario el archivo del original del que se portó.

## 🛣️ Siguientes pasos

- [ ] Partículas por *compute shader* (explosiones, humo, sangre).
- [ ] Reescalado de sprites con IA y sprites 3D.
- [ ] Juego en red / en línea con *rollback* (la simulación ya es determinista).
- [ ] Versión web (wgpu → WebGPU / WASM).

## 📜 Licencia y créditos

Este proyecto es software libre bajo la **GNU GPL v2 o posterior** (ver [`LICENSE`](LICENSE)), igual que el juego
original.

**ClanBomber** es obra de **Andreas Hundt** y **Denis Oliver Kropp**, con la IA «mass», y muchos colaboradores
(ver `AUTHORS` del proyecto original). La versión SDL es de Rene Lopez y viti95. Los **gráficos, sonidos, música y
mapas** de este repositorio provienen de ClanBomber 2 y se distribuyen bajo sus mismos términos. La tipografía
*DejaVu Sans Bold* se incluye con su propia licencia ([`assets/fonts/LICENSE.DEJAVU`](assets/fonts/LICENSE.DEJAVU)).

Este es un port independiente y no oficial. Todo el código Rust es nuevo.
