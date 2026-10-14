//! Renderer 2D sobre wgpu: batcher de quads en resolución virtual 800x600
//! con letterboxing. La escena se dibuja en una textura flotante intermedia, con partículas
//! simuladas por compute, y una pasada final la compone en la ventana (bloom, CRT).

mod board3d;
mod fire;
mod fx;
mod heat;
mod light;
mod text;

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

pub use board3d::{faces, Cube, Light3d, View3d, MAX_LIGHTS_3D};
pub use fire::Fire;
pub use fx::{Burst, Camera, Effects, ParticleKind, MAX_SPOTS};
pub use light::{Light, Lighting};
pub use text::{Align, Font, FontError, Shadow, TextStyle};

pub const VIRTUAL_W: f32 = 800.0;
pub const VIRTUAL_H: f32 = 600.0;
const MAX_QUADS: usize = 4096;

/// Identificador de una textura cargada con [`Renderer::load_texture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureId(usize);

struct Texture {
    bind_group: wgpu::BindGroup,
    size: (f32, f32),
}

enum Batch {
    Quads {
        tex: TextureId,
        first_quad: u32,
        quads: u32,
    },
    /// Explosiones procedurales (`fire.wgsl`), en su orden de dibujo.
    Fire { first: u32, count: u32 },
    /// Iluminación: multiplica lo dibujado hasta aquí por la luz (`light.wgsl`).
    Light,
    /// Cuadriláteros de la escena 3D (`board3d.wgsl`). Los lotes 3D seguidos comparten una
    /// pasada con su depth buffer.
    Mesh3d {
        tex: TextureId,
        first_quad: u32,
        quads: u32,
    },
}

/// Hoja de sprites con celdas de tamaño fijo, numeradas en fila. `cell_w`/`cell_h` son el
/// tamaño de dibujo (píxeles virtuales); la textura tiene `scale` texels por píxel virtual
/// (1 en las hojas del original, más en las reescaladas).
#[derive(Clone, Copy)]
pub struct Sheet {
    pub tex: TextureId,
    pub cell_w: f32,
    pub cell_h: f32,
    pub cols: u32,
    pub scale: f32,
}

impl Sheet {
    /// Rectángulo de la celda en texels.
    pub fn cell(&self, index: u32) -> [f32; 4] {
        let (cx, cy) = (index % self.cols, index / self.cols);
        let (w, h) = (self.cell_w * self.scale, self.cell_h * self.scale);
        [cx as f32 * w, cy as f32 * h, w, h]
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    /// Modo de dibujo de `quad.wgsl` (NORMAL, SILHOUETTE, ICE, HUE, ...).
    mode: u32,
}

const MODE_NORMAL: u32 = 0;
const MODE_SILHOUETTE: u32 = 1;
const MODE_ICE: u32 = 2;
const MODE_HUE: u32 = 3;
const MODE_REFLECT: u32 = 4;
const MODE_BURN: u32 = 5;
const MODE_REVEAL: u32 = 6;
const MODE_WATCH: u32 = 7;
const MODE_TEXT: u32 = 8;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// Bilineal: para los campos de distancias del texto.
    smooth_sampler: wgpu::Sampler,
    textures: Vec<Texture>,
    batches: Vec<Batch>,
    fire: fire::FirePipeline,
    board3d: board3d::Board3dPipeline,
    scene3d: board3d::Scene3d,
    light: light::LightPipeline,
    heat: heat::HeatPipeline,
    fires: Vec<fire::FireInstance>,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    verts: Vec<Vertex>,
    capture: Option<std::path::PathBuf>,
    post: fx::Post,
    /// `None` si la GPU no tiene compute shaders.
    particles: Option<fx::Particles>,
    effects: Effects,
    /// Lote antes del cual se dibujan las partículas (si se pidió con `particle_layer`).
    particle_layer: Option<usize>,
    shake: f32,
    started: std::time::Instant,
    /// Tamaño virtual y reloj para los shaders (`Globals`).
    globals: wgpu::Buffer,
    waves: Vec<[f32; 4]>,
    camera: Camera,
}

impl Renderer {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("adapter");
        // Partículas y bloom necesitan compute shaders (los hay en Vulkan, Metal y DX12; no en
        // algunos backends GL antiguos, donde el juego se ve como el original).
        let compute = adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS);
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("device");

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("config");
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("quad.wgsl"));
        let globals = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globals"),
            contents: bytemuck::cast_slice(&[VIRTUAL_W, VIRTUAL_H, 0.0, 1.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // Nearest: pixel art sin suavizado.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let smooth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bgl), Some(&tex_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quads"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Uint32],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fx::SCENE_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertices"),
            size: (MAX_QUADS * 4 * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices: Vec<u32> = (0..MAX_QUADS as u32)
            .flat_map(|q| [0, 1, 2, 2, 1, 3].map(|i| q * 4 + i))
            .collect();
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let post = fx::Post::new(
            &device,
            config.format,
            letterbox(config.width, config.height).1,
            compute,
        );
        let particles = compute.then(|| fx::Particles::new(&device, &globals));
        let fire = fire::FirePipeline::new(&device, &globals, fx::SCENE_FORMAT);
        let board3d =
            board3d::Board3dPipeline::new(&device, &tex_layout, fx::SCENE_FORMAT, post.scene_size);
        let light = light::LightPipeline::new(&device, &globals, fx::SCENE_FORMAT);
        let heat = heat::HeatPipeline::new(&device);
        let mut r = Renderer {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            tex_layout,
            sampler,
            smooth_sampler,
            textures: Vec::new(),
            batches: Vec::new(),
            fire,
            board3d,
            scene3d: Default::default(),
            light,
            heat,
            fires: Vec::new(),
            vbuf,
            ibuf,
            verts: Vec::with_capacity(MAX_QUADS * 4),
            capture: None,
            post,
            particles,
            effects: Effects::default(),
            particle_layer: None,
            shake: 0.0,
            started: std::time::Instant::now(),
            globals,
            waves: Vec::new(),
            camera: Camera::default(),
        };
        // Textura 0: un píxel blanco, para los quads de color plano.
        r.load_rgba(&[255; 4], 1, 1);
        r
    }

    /// Decodifica un PNG y lo sube a la GPU.
    pub fn load_texture(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<TextureId, image::ImageError> {
        let img = image::open(path)?.to_rgba8();
        let (w, h) = img.dimensions();
        Ok(self.load_rgba(&img, w, h))
    }

    /// Como [`load_texture`](Self::load_texture) pero con filtrado bilineal.
    pub fn load_texture_smooth(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<TextureId, image::ImageError> {
        let img = image::open(path)?.to_rgba8();
        let (w, h) = img.dimensions();
        Ok(self.upload(&img, w, h, true))
    }

    pub fn load_rgba(&mut self, rgba: &[u8], w: u32, h: u32) -> TextureId {
        self.upload(rgba, w, h, false)
    }

    /// Como [`load_rgba`](Self::load_rgba) pero con filtrado bilineal (no es pixel art).
    pub fn load_rgba_smooth(&mut self, rgba: &[u8], w: u32, h: u32) -> TextureId {
        self.upload(rgba, w, h, true)
    }

    fn upload(&mut self, rgba: &[u8], w: u32, h: u32, smooth: bool) -> TextureId {
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = tex.create_view(&Default::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.tex_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(if smooth {
                        &self.smooth_sampler
                    } else {
                        &self.sampler
                    }),
                },
            ],
        });
        self.textures.push(Texture {
            bind_group,
            size: (w as f32, h as f32),
        });
        TextureId(self.textures.len() - 1)
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.post.resize(&self.device, letterbox(w, h).1);
        self.board3d.resize(&self.device, self.post.scene_size);
    }

    pub fn set_effects(&mut self, effects: Effects) {
        self.effects = effects;
    }

    pub fn effects(&self) -> Effects {
        self.effects
    }

    /// Si la GPU puede con partículas y bloom (compute shaders).
    pub fn has_compute(&self) -> bool {
        self.particles.is_some()
    }

    /// Lanza una ráfaga de partículas (se ignora si están apagadas o no hay compute).
    pub fn emit(&mut self, burst: &Burst) {
        if let (true, Some(p)) = (self.effects.particles, self.particles.as_mut()) {
            p.emit(burst);
        }
    }

    /// Hace avanzar la simulación de partículas `dt` segundos en el próximo `render()`
    /// (sin llamarlo, como en pausa, se quedan quietas).
    pub fn advance_particles(&mut self, dt: f32) {
        if let Some(p) = self.particles.as_mut() {
            p.advance(dt);
        }
    }

    /// Las partículas se dibujarán aquí, entre lo ya pedido y lo que venga (por defecto no se
    /// dibujan).
    pub fn particle_layer(&mut self) {
        self.particle_layer = Some(self.batches.len());
    }

    /// Zoom, punto de foco y "drama" de la composición final; se queda hasta que se cambie.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }

    /// Temblor de pantalla en [0, 1]: separa los canales de color en la composición.
    pub fn set_shake(&mut self, shake: f32) {
        self.shake = shake;
    }

    /// Rectángulo relleno en coordenadas virtuales (800x600).
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.push_quad(TextureId(0), [x, y, w, h], [0.0; 4], color, MODE_NORMAL);
    }

    /// Región `src` (x, y, w, h en píxeles de la textura) dibujada en `dst`.
    pub fn sprite(&mut self, tex: TextureId, src: [f32; 4], dst: [f32; 4]) {
        self.push_quad(tex, dst, src, [1.0; 4], MODE_NORMAL);
    }

    /// Como [`sprite`](Self::sprite) pero multiplicando el color (para el alfa al desvanecerse).
    pub fn sprite_tinted(&mut self, tex: TextureId, src: [f32; 4], dst: [f32; 4], tint: [f32; 4]) {
        self.push_quad(tex, dst, src, tint, MODE_NORMAL);
    }

    /// La forma del sprite rellena de `color` (el alfa de `color` multiplica el del sprite).
    pub fn sprite_silhouette(
        &mut self,
        tex: TextureId,
        src: [f32; 4],
        dst: [f32; 4],
        color: [f32; 4],
    ) {
        self.push_quad(tex, dst, src, color, MODE_SILHOUETTE);
    }

    /// Sprite que se quema desde `center` hacia fuera hasta `radius` (px virtuales): con
    /// `progress` 0 entero y con 1 desaparecido, con un frente incandescente. El patrón depende
    /// de la posición en pantalla, así que coincide con el de
    /// [`sprite_reveal`](Self::sprite_reveal) con el mismo centro.
    pub fn sprite_burn(
        &mut self,
        tex: TextureId,
        src: [f32; 4],
        dst: [f32; 4],
        progress: f32,
        center: [f32; 2],
        radius: f32,
    ) {
        let mode = MODE_BURN | progress_bits(progress);
        self.push_quad(tex, dst, src, [center[0], center[1], radius, 1.0], mode);
    }

    /// Sprite que aparece por donde [`sprite_burn`](Self::sprite_burn) ya quemó, con el mismo
    /// `progress`, centro y radio.
    #[allow(clippy::too_many_arguments)]
    pub fn sprite_reveal(
        &mut self,
        tex: TextureId,
        src: [f32; 4],
        dst: [f32; 4],
        progress: f32,
        center: [f32; 2],
        radius: f32,
        alpha: f32,
    ) {
        let mode = MODE_REVEAL | progress_bits(progress);
        self.push_quad(tex, dst, src, [center[0], center[1], radius, alpha], mode);
    }

    /// Cronómetro antiguo dibujado por el shader en `rect` (proporción 1 : 1.2, la corona arriba),
    /// girado `angle` radianes sobre su centro. `seconds` y `minutes` son las vueltas (0..1) de
    /// la aguja roja y de la subesfera; `alarm` (0..1) tiñe la esfera de rojo.
    pub fn stopwatch(
        &mut self,
        rect: [f32; 4],
        angle: f32,
        seconds: f32,
        minutes: f32,
        alarm: f32,
    ) {
        let [x, y, w, h] = rect;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let (s, c) = angle.sin_cos();
        let color = [seconds, minutes, alarm.clamp(0.0, 1.0), 1.0];
        let vert = |u: f32, v: f32| {
            let (dx, dy) = ((u - 0.5) * w, (v - 0.5) * h);
            Vertex {
                pos: [cx + dx * c - dy * s, cy + dx * s + dy * c],
                uv: [u, v],
                color,
                mode: MODE_WATCH,
            }
        };
        self.push_vertices(
            TextureId(0),
            [
                vert(0.0, 0.0),
                vert(1.0, 0.0),
                vert(0.0, 1.0),
                vert(1.0, 1.0),
            ],
        );
    }

    /// Sprite con el tono girando en el tiempo; `amount` en [0, 1] mezcla con el original.
    pub fn sprite_hue(&mut self, tex: TextureId, src: [f32; 4], dst: [f32; 4], amount: f32) {
        self.push_quad(tex, dst, src, [1.0, 1.0, 1.0, amount], MODE_HUE);
    }

    /// Reflejo de un sprite dibujado en `dst`: invertido bajo su borde inferior, recortado a
    /// `clip` y desvaneciéndose hacia abajo (`alpha` es la opacidad junto al borde).
    pub fn sprite_reflection(
        &mut self,
        tex: TextureId,
        src: [f32; 4],
        dst: [f32; 4],
        clip: [f32; 4],
        alpha: f32,
    ) {
        let [x, y, w, h] = dst;
        // El reflejo ocupa [x, y + h] .. [x + w, y + 2h]; se recorta a `clip`.
        let top = y + h;
        let x0 = x.max(clip[0]);
        let x1 = (x + w).min(clip[0] + clip[2]);
        let y0 = top.max(clip[1]);
        let y1 = (top + h).min(clip[1] + clip[3]);
        if x0 >= x1 || y0 >= y1 || w <= 0.0 || h <= 0.0 {
            return;
        }
        let (fx0, fx1) = ((x0 - x) / w, (x1 - x) / w);
        let (fy0, fy1) = ((y0 - top) / h, (y1 - top) / h);
        let (tw, th) = self.textures[tex.0].size;
        let u = |f: f32| (src[0] + src[2] * f) / tw;
        // Invertido: la fila f del reflejo es la fila (1 - f) del sprite.
        let v = |f: f32| (src[1] + src[3] * (1.0 - f)) / th;
        let tint = [0.8, 0.9, 1.0];
        let color = |f: f32| [tint[0], tint[1], tint[2], alpha * reflection_fade(f)];
        let vert = |px, py, fx, fy| Vertex {
            pos: [px, py],
            uv: [u(fx), v(fy)],
            color: color(fy),
            mode: MODE_REFLECT,
        };
        self.push_vertices(
            tex,
            [
                vert(x0, y0, fx0, fy0),
                vert(x1, y0, fx1, fy0),
                vert(x0, y1, fx0, fy1),
                vert(x1, y1, fx1, fy1),
            ],
        );
    }

    /// Región `src` de una textura dibujada centrada en `center`, de tamaño `size` y girada
    /// `angle` radianes (trozos que vuelan).
    pub fn sprite_rotated(
        &mut self,
        tex: TextureId,
        src: [f32; 4],
        center: [f32; 2],
        size: [f32; 2],
        angle: f32,
        tint: [f32; 4],
    ) {
        let (tw, th) = self.textures[tex.0].size;
        let (u0, v0) = (src[0] / tw, src[1] / th);
        let (u1, v1) = ((src[0] + src[2]) / tw, (src[1] + src[3]) / th);
        let (s, c) = angle.sin_cos();
        let (hw, hh) = (size[0] / 2.0, size[1] / 2.0);
        let vert = |dx: f32, dy: f32, u: f32, v: f32| Vertex {
            pos: [center[0] + dx * c - dy * s, center[1] + dx * s + dy * c],
            uv: [u, v],
            color: tint,
            mode: MODE_NORMAL,
        };
        self.push_vertices(
            tex,
            [
                vert(-hw, -hh, u0, v0),
                vert(hw, -hh, u1, v0),
                vert(-hw, hh, u0, v1),
                vert(hw, hh, u1, v1),
            ],
        );
    }

    /// Rectángulo de color plano centrado en `center` y girado `angle` radianes (hojas).
    pub fn quad_rotated(&mut self, center: [f32; 2], size: [f32; 2], angle: f32, color: [f32; 4]) {
        self.sprite_rotated(
            TextureId(0),
            [0.0, 0.0, 1.0, 1.0],
            center,
            size,
            angle,
            color,
        );
    }

    /// Destello de hielo animado sobre el rectángulo `dst` (se dibuja encima de la casilla).
    pub fn ice_shine(&mut self, dst: [f32; 4], alpha: f32) {
        self.push_quad(
            TextureId(0),
            dst,
            [0.0; 4],
            [1.0, 1.0, 1.0, alpha],
            MODE_ICE,
        );
    }

    /// Luz puntual para la próxima llamada a [`lighting`](Self::lighting) de este fotograma.
    pub fn light(&mut self, light: Light) {
        // Una luz con NaN rellenaría su círculo entero de NaN en la escena.
        let finite = light.intensity.is_finite() && light.radius.is_finite();
        if finite && self.effects.lighting && self.light.lights.len() < light::MAX_LIGHTS {
            self.light.lights.push(light);
        }
    }

    /// Aplica la iluminación (ambiente, luces pedidas con [`light`](Self::light) y sombras de la
    /// rejilla) a todo lo dibujado hasta ahora. No hace nada si está apagada.
    pub fn lighting(&mut self, lighting: &Lighting) {
        if !self.effects.lighting {
            return;
        }
        self.light.upload(&self.device, &self.queue, lighting);
        self.batches.push(Batch::Light);
    }

    /// Onda expansiva que deforma la imagen en la composición final. `progress` va de 0 a 1 y
    /// `strength` escala radio y amplitud. Solo cuenta para el fotograma actual.
    pub fn shockwave(&mut self, center: [f32; 2], progress: f32, strength: f32) {
        if self.effects.shockwaves && self.waves.len() < fx::MAX_WAVES {
            self.waves
                .push([center[0], center[1], progress.clamp(0.0, 1.0), strength]);
        }
    }

    /// Aire caliente que tiembla sobre `rect` (px virtuales), con `soft` px de borde difuminado.
    /// `intensity` 1 es el calor de una llama; las fuentes que se solapan se suman. Solo cuenta
    /// para el fotograma actual.
    pub fn heat(&mut self, rect: [f32; 4], soft: f32, intensity: f32) {
        let valid = rect.iter().all(|v| v.is_finite()) && intensity.is_finite() && soft.is_finite();
        if self.effects.shockwaves
            && valid
            && intensity > 0.0
            && self.heat.sources.len() < heat::MAX_HEAT
        {
            self.heat.sources.push(heat::HeatInstance {
                rect,
                params: [intensity, soft.max(0.0)],
            });
        }
    }

    /// Celda `index` de una hoja de sprites, en (x, y) a tamaño nativo.
    pub fn sheet_sprite(&mut self, sheet: &Sheet, index: u32, x: f32, y: f32) {
        self.sprite(
            sheet.tex,
            sheet.cell(index),
            [x, y, sheet.cell_w, sheet.cell_h],
        );
    }

    /// Dibuja la textura entera ocupando toda la pantalla virtual (fondos de cualquier tamaño).
    pub fn fullscreen(&mut self, tex: TextureId) {
        self.sprite_whole(tex, [0.0, 0.0, VIRTUAL_W, VIRTUAL_H], [1.0; 4]);
    }

    /// Tamaño en texels de una textura.
    pub fn texture_size(&self, tex: TextureId) -> (f32, f32) {
        self.textures[tex.0].size
    }

    /// La textura entera en `dst`, sea cual sea su tamaño (la original o la HD).
    pub fn sprite_whole(&mut self, tex: TextureId, dst: [f32; 4], tint: [f32; 4]) {
        let (w, h) = self.textures[tex.0].size;
        self.sprite_tinted(tex, [0.0, 0.0, w, h], dst, tint);
    }

    pub fn sheet(&self, tex: TextureId, cell_w: f32, cell_h: f32) -> Sheet {
        self.sheet_scaled(tex, cell_w, cell_h, 1.0)
    }

    /// Hoja cuya textura tiene `scale` texels por píxel virtual (sprites reescalados).
    pub fn sheet_scaled(&self, tex: TextureId, cell_w: f32, cell_h: f32, scale: f32) -> Sheet {
        Sheet {
            tex,
            cell_w,
            cell_h,
            cols: (self.textures[tex.0].size.0 / (cell_w * scale)) as u32,
            scale,
        }
    }

    fn push_quad(
        &mut self,
        tex: TextureId,
        dst: [f32; 4],
        src: [f32; 4],
        color: [f32; 4],
        mode: u32,
    ) {
        // Sin región de origen (quad de color): UV al centro del píxel blanco.
        let (u0, v0, u1, v1) = if src[2] == 0.0 {
            (0.5, 0.5, 0.5, 0.5)
        } else {
            let (tw, th) = self.textures[tex.0].size;
            (
                src[0] / tw,
                src[1] / th,
                (src[0] + src[2]) / tw,
                (src[1] + src[3]) / th,
            )
        };
        let [x, y, w, h] = dst;
        let v = |px, py, u, vv| Vertex {
            pos: [px, py],
            uv: [u, vv],
            color,
            mode,
        };
        self.push_vertices(
            tex,
            [
                v(x, y, u0, v0),
                v(x + w, y, u1, v0),
                v(x, y + h, u0, v1),
                v(x + w, y + h, u1, v1),
            ],
        );
    }

    /// Añade un quad (arriba-izquierda, arriba-derecha, abajo-izquierda, abajo-derecha).
    fn push_vertices(&mut self, tex: TextureId, quad: [Vertex; 4]) {
        let quad_index = (self.verts.len() / 4) as u32;
        if quad_index as usize >= MAX_QUADS {
            return;
        }
        self.verts.extend(quad);
        match self.batches.last_mut() {
            Some(Batch::Quads { tex: t, quads, .. }) if *t == tex => *quads += 1,
            _ => self.batches.push(Batch::Quads {
                tex,
                first_quad: quad_index,
                quads: 1,
            }),
        }
    }

    /// Cámara y luz ambiente de la escena 3D de este fotograma (la última que se pida vale para
    /// todos los cubos).
    pub fn set_view3d(&mut self, view: View3d) {
        self.scene3d.view = view;
    }

    /// Luz puntual de la escena 3D (hasta [`MAX_LIGHTS_3D`] por fotograma).
    pub fn light3d(&mut self, light: Light3d) {
        self.scene3d.lights.push(light);
    }

    /// Prisma con textura en la escena 3D. Los cubos se dibujan con profundidad entre sí, en el
    /// punto del orden de dibujo en que se piden, y lo 2D posterior queda encima.
    pub fn cube(&mut self, cube: &Cube) {
        let first = (self.scene3d.verts.len() / 4) as u32;
        let size = self.textures[cube.tex.0].size;
        let quads = self.scene3d.push_cube(cube, size);
        if quads == 0 {
            return;
        }
        match self.batches.last_mut() {
            Some(Batch::Mesh3d { tex, quads: q, .. }) if *tex == cube.tex => *q += quads,
            _ => self.batches.push(Batch::Mesh3d {
                tex: cube.tex,
                first_quad: first,
                quads,
            }),
        }
    }

    /// Explosión procedural (en lugar de los sprites de `explosion2.png`).
    pub fn fire(&mut self, fire: &Fire) {
        if self.fires.len() >= fire::MAX_FIRES {
            return;
        }
        let index = self.fires.len() as u32;
        self.fires.push(fire.instance());
        match self.batches.last_mut() {
            Some(Batch::Fire { count, .. }) => *count += 1,
            _ => self.batches.push(Batch::Fire {
                first: index,
                count: 1,
            }),
        }
    }

    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            _ => {
                // Superficie perdida/desactualizada: se reconfigura y se reintenta el siguiente frame.
                self.surface.configure(&self.device, &self.config);
                self.clear_frame();
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        self.queue
            .write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&self.verts));
        self.fire.upload(&self.queue, &self.fires);
        self.board3d.upload(&self.queue, &self.scene3d);
        let time = self.started.elapsed().as_secs_f32();
        self.queue.write_buffer(
            &self.globals,
            0,
            bytemuck::cast_slice(&[
                VIRTUAL_W,
                VIRTUAL_H,
                time,
                self.post.scene_size.0 as f32 / VIRTUAL_W,
            ]),
        );

        let mut enc = self.device.create_command_encoder(&Default::default());
        if let Some(p) = self.particles.as_mut() {
            if self.effects.particles {
                p.simulate(&self.queue, &mut enc);
            } else {
                p.discard();
            }
        }
        self.record_scene(&mut enc);
        if self.effects.bloom {
            self.post.bloom(&mut enc);
        }
        self.heat.draw(&self.queue, &mut enc, &self.post.heat);
        self.post.set_params(
            &self.queue,
            self.effects,
            self.shake,
            time,
            &self.waves,
            !self.heat.sources.is_empty(),
            self.camera,
        );
        let (viewport, _) = letterbox(self.config.width, self.config.height);
        self.post.composite(&mut enc, &view, viewport);
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
        if let Some(path) = self.capture.take() {
            if let Err(e) = self.save_capture(&path) {
                eprintln!("no se pudo guardar la captura {path:?}: {e}");
            }
        }
        self.clear_frame();
    }

    fn clear_frame(&mut self) {
        self.verts.clear();
        self.batches.clear();
        self.fires.clear();
        self.scene3d.clear();
        self.waves.clear();
        self.heat.sources.clear();
        self.light.lights.clear();
        self.particle_layer = None;
    }

    /// Dibuja los quads acumulados (y las partículas, en su capa) en la escena intermedia. Cada
    /// tramo de lotes 3D seguidos va en su propia pasada con profundidad; lo 2D de antes y de
    /// después, en pasadas sin ella.
    fn record_scene(&self, enc: &mut wgpu::CommandEncoder) {
        let is_3d = |i: usize| matches!(self.batches.get(i), Some(Batch::Mesh3d { .. }));
        let n = self.batches.len();
        let mut start = 0;
        loop {
            let end = (start..n).find(|&i| is_3d(i)).unwrap_or(n);
            self.record_2d(enc, start..end, start == 0);
            if end == n {
                break;
            }
            let mut next = end;
            let mut meshes = Vec::new();
            while is_3d(next) {
                if let Batch::Mesh3d {
                    tex,
                    first_quad,
                    quads,
                } = self.batches[next]
                {
                    meshes.push((&self.textures[tex.0].bind_group, first_quad, quads));
                }
                next += 1;
            }
            self.board3d.draw(enc, &self.post.scene, &meshes);
            start = next;
        }
    }

    /// Una pasada 2D con los lotes de `range` (la primera borra la escena). Las partículas van
    /// antes del lote de su capa, o al final si la capa cae justo después del tramo.
    fn record_2d(
        &self,
        enc: &mut wgpu::CommandEncoder,
        range: std::ops::Range<usize>,
        clear: bool,
    ) {
        let view = &self.post.scene;
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let particles = self
            .particles
            .as_ref()
            .filter(|_| self.effects.particles)
            .zip(self.particle_layer);
        let bind_quads = |pass: &mut wgpu::RenderPass| {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vbuf.slice(..));
            pass.set_index_buffer(self.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        };
        bind_quads(&mut pass);
        let last = range.end == self.batches.len();
        for i in range.clone() {
            if let Some((p, _)) = particles.filter(|&(_, at)| at == i) {
                p.draw(&mut pass);
                bind_quads(&mut pass);
            }
            match self.batches[i] {
                Batch::Quads {
                    tex,
                    first_quad,
                    quads,
                } => {
                    pass.set_bind_group(1, &self.textures[tex.0].bind_group, &[]);
                    pass.draw_indexed(first_quad * 6..(first_quad + quads) * 6, 0, 0..1);
                }
                Batch::Fire { first, count } => {
                    self.fire.draw(&mut pass, first, count);
                    bind_quads(&mut pass);
                }
                Batch::Light => {
                    self.light.draw(&mut pass);
                    bind_quads(&mut pass);
                }
                Batch::Mesh3d { .. } => {}
            }
        }
        if let Some((p, _)) =
            particles.filter(|&(_, at)| at == range.end || (last && at >= range.end))
        {
            p.draw(&mut pass);
        }
    }

    /// Pide que el próximo `render()` guarde además el lienzo virtual (800x600, sin bandas) como
    /// PNG. No toca la ventana ni captura nada del escritorio.
    pub fn request_capture(&mut self, path: impl Into<std::path::PathBuf>) {
        self.capture = Some(path.into());
    }

    /// Vuelve a componer el frame actual en una textura propia y la lee de la GPU.
    fn save_capture(&self, path: &std::path::Path) -> Result<(), String> {
        let (w, h) = self.post.scene_size;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("captura"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Las filas de la copia se alinean a 256 bytes.
        let row = (w * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("captura"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        let view = tex.create_view(&Default::default());
        self.post
            .composite(&mut enc, &view, [0.0, 0.0, w as f32, h as f32]);
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let (tx, rx) = std::sync::mpsc::channel();
        buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("{e:?}"))?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("{e:?}"))?;

        let data = buf
            .slice(..)
            .get_mapped_range()
            .map_err(|e| format!("{e:?}"))?;
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let line = &data[y * row as usize..][..(w * 4) as usize];
            for px in line.as_chunks::<4>().0 {
                rgba.extend_from_slice(&if bgra {
                    [px[2], px[1], px[0], 255]
                } else {
                    [px[0], px[1], px[2], 255]
                });
            }
        }
        drop(data);
        buf.unmap();
        let mut img = image::RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| "tamaño de imagen inválido".to_string())?;
        // La escena va al tamaño de la ventana; la captura siempre al lienzo virtual.
        let (vw, vh) = (VIRTUAL_W as u32, VIRTUAL_H as u32);
        if (w, h) != (vw, vh) {
            img = image::imageops::resize(&img, vw, vh, image::imageops::FilterType::Triangle);
        }
        img.save(path).map_err(|e| e.to_string())
    }
}

/// Avance 0..1 en los 16 bits altos del modo del vértice (`quad.wgsl`).
fn progress_bits(progress: f32) -> u32 {
    let p = if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    };
    ((p * 65535.0).round() as u32) << 16
}

/// Opacidad del reflejo a la altura `f` (0 junto a los pies, 1 al final). Con posiciones
/// fraccionarias `f` puede pasar de 1 por redondeo, y una base negativa en `powf` da NaN: un
/// píxel NaN que el bloom convertía en un cuadrado negro.
fn reflection_fade(f: f32) -> f32 {
    (1.0 - f).clamp(0.0, 1.0).powf(1.3)
}

/// Rectángulo 4:3 centrado en una ventana de `w`x`h` (x, y, ancho, alto) y su tamaño en píxeles.
fn letterbox(w: u32, h: u32) -> ([f32; 4], (u32, u32)) {
    let (sw, sh) = (w as f32, h as f32);
    let scale = (sw / VIRTUAL_W).min(sh / VIRTUAL_H);
    let (vw, vh) = ((VIRTUAL_W * scale).round(), (VIRTUAL_H * scale).round());
    (
        [((sw - vw) / 2.0).floor(), ((sh - vh) / 2.0).floor(), vw, vh],
        (vw.max(1.0) as u32, vh.max(1.0) as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::reflection_fade;

    #[test]
    fn reflection_fade_never_nan() {
        // `top + h - top` puede dar algo más que `h` con posiciones fraccionarias.
        for f in [-0.001, 0.0, 0.5, 1.0, 1.000_000_1, 1.5] {
            let a = reflection_fade(f);
            assert!(a.is_finite() && (0.0..=1.0).contains(&a), "f = {f}: {a}");
        }
        assert_eq!(reflection_fade(1.000_000_1), 0.0);
    }
}
