# Handoff: menú principal como tablero 3D ("el menú es una partida")

Estado: fases 1, 2 y 3 hechas en la rama `menu-3d` (`cb-render/board3d.rs`, `cb-game/menu_board.rs`,
caras de los cubos en `assets/pics/maptiles_cubes.png`). Player Setup y Map Selection
siguen como pantallas propias. Maqueta:
`docs/menu-3d-maqueta.png`.

## La idea

El menú principal deja de ser el rectángulo azul translúcido sobre el logo (`menu.rs::draw`) y
pasa a ser **un nivel de ClanBomber en miniatura, en 3D de verdad**: una isla/diorama flotando en
la oscuridad, vista desde arriba a unos 45° en perspectiva, con el logo en llamas
(`clanbomber_title_andi.png`) brillando detrás.

De la maqueta, lo que hay que conservar:

- Muros del borde y **cajas como cubos con altura** (caras de arriba y laterales), suelo de
  baldosas, la isla con un "espesor" de tierra/piedra y raíces colgando por debajo. Tema jungla
  en la maqueta; debería poder ser cualquiera de los 6 (idea: el último mapa jugado).
- **Seis cajas en fila horizontal**, una por opción del menú raíz: New Game, Options, Map
  Editor, Show Credits, Help Screen, Quit. Encima de cada una, su nombre en texto SDF derecho y
  de frente a la cámara (no en perspectiva). La seleccionada brilla en cálido, las demás más
  apagadas.
- **El cursor es un bomber** (skin del jugador 1; en la maqueta el diablito BSD) parado junto a
  la caja seleccionada, con una bomba con la mecha encendida que alumbra la caja y el suelo.
  Izquierda/derecha lo hace caminar a la caja vecina. Los sprites (bomber, bomba) son
  billboards planos de frente a la cámara, como en la maqueta.
- **Antorchas en las esquinas** del tablero (luces puntuales cálidas), ascuas flotando.
- `v0.2.0` chico en una esquina.

## Fases

1. **(Empezar por aquí)** Renderer 3D + tablero del menú raíz: cubos con textura, cámara en
   perspectiva que respira apenas, luces puntuales (antorchas + mecha), cajas con sus nombres,
   bomber que camina entre cajas con ←/→ (Enter sigue entrando al submenú como hoy, sin
   animación todavía). Los submenús siguen con el panel actual por ahora.
2. Al elegir: el bomber pone la bomba, la mecha chisporrotea ~0,5 s, la caja explota (fuego por
   shader, trozos con `shards.rs`, bloom) y la cámara se zambulle en el destello hacia el
   submenú. Esc hace el camino inverso (las piezas vuelven a su sitio). **Todo saltable**:
   Enter otra vez la hace explotar al instante; nunca bloquear la navegación.
3. Menús largos (opciones con valores, Player Setup, lista de mapas) como letreros/paneles pixel
   art clavados al tablero que entran girando como una carta. No intentar meter 33 mapas como
   cajas.

Idea suelta para después: si el observador ("chupa poto", `observer.png`) cruza volando cuando
uno se queda quieto en el menú, mejor aún. Y el usuario quiere que este renderer de cubos sea la
base de un posible modo 2.5D/voxel del juego algún día: diseñarlo para dibujar un `Map`
cualquiera, no solo el tablero del menú (la simulación no cambia; el 3D es otra forma de dibujar).

## Cómo encaja en el renderer actual (leído en el código)

- `cb-render` (`crates/render/src/lib.rs`): todo es un batcher 2D de quads en lienzo virtual
  800x600. `Renderer::render` → `record_scene` abre **un** render pass sobre la textura de escena
  `post.scene` (`fx.rs`, `SCENE_FORMAT = Rgba16Float`, sin depth) y recorre `self.batches`
  (`Batch::Quads`, `Batch::Fire`, `Batch::Light`; las partículas se intercalan por índice).
  Después `fx.rs` hace bloom/CRT/ondas/calor/cámara (`post.wgsl`) y compone en la ventana.
- Para el 3D hace falta: un pipeline nuevo (p. ej. `board3d.rs` + `board3d.wgsl`) con vértices
  pos3/uv/normal (o cubos instanciados), **depth buffer** propio (hoy no hay ninguno; el pass
  usa `depth_stencil_attachment: None`), cámara con matriz de perspectiva, y luces puntuales en
  el fragment shader (pocas, en un uniform). Lo natural es un nuevo `Batch::Board3d` que se
  dibuje al principio del pass de escena (o en un pass previo con su depth) para que el logo de
  fondo, los nombres, los sprites, el bloom y las partículas sigan funcionando encima/después.
  El fondo (`fullscreen(title_bg)`) va antes del tablero.
- Billboards y nombres: proyectar en CPU la posición 3D a coordenadas del lienzo 800x600 con la
  misma matriz y dibujar con los quads/texto de siempre (`sheet_sprite`, `text_styled`), así el
  bomber usa su hoja (`Assets::skins`, celda `dir*10 + anim`) y la bomba su latido (`draw.rs`).
  Ojo con el orden de dibujo respecto de los cubos (el bomber delante de la caja).
- Texturas: `Assets::maptiles` es una `Sheet` de 6 temas x 4 celdas (suelo, muro, caja, caja
  dañada) a 4 texels por píxel (`SCALE`, celdas de 160 texels). Caras de arriba = esas celdas.
  Caras laterales: al principio la misma celda oscurecida; después se le pueden pedir a ChatGPT
  texturas laterales por tema (flujo en `tools/sprites/README.md` y la memoria
  `sprites-ia-chatgpt`).
- Luces 2D actuales (`light.rs`/`light.wgsl`) son un raymarch sobre la rejilla, no sirven para el
  3D: las luces del tablero van en su propio shader.
- Sin compute shaders (GL antiguo) el juego cae a dibujo simple; el pipeline 3D solo usa
  vértice/fragmento, así que debería funcionar igual, pero hay que tener un camino de respaldo
  (si falla crear el pipeline, dibujar el menú viejo).
- Hay un test que valida todos los WGSL con naga (`fx.rs`, ~línea 776; la CI no tiene GPU):
  agregar ahí el shader nuevo.
- wgpu 30: la API difiere de los tutoriales (ver "Notas de wgpu 30" en `CLAUDE.md`).

## El menú hoy (`crates/game/src/menu.rs`)

Árbol de `Item`s (`Id`, `Kind::Value {min,max,value}` para opciones), `current` = nodo abierto,
`selection` = fila; ↑/↓ mueven, ←/→ cambian valores, Enter entra, Esc sube. `draw` pinta
`fullscreen(title_bg)`, el rectángulo azul y las filas. Para la fase 1, el tablero reemplaza el
dibujo solo cuando `current` es la raíz; en el tablero ←/→ (y también ↑/↓) mueven la selección.

## Verificación

`cargo run -q -p cb-game -- --screenshots <dir>` guarda `menu.png` (render offscreen, sin
ventana): sirve para mirar el resultado sin abrir el juego. Antes de commitear:
`cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test`.
Commits chicos y temáticos, en español.
