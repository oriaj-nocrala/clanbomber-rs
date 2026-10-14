//! Efectos en la GPU: partículas simuladas por compute, bloom por compute y la pasada final que
//! compone la escena (con filtro CRT opcional) sobre la ventana.

use wgpu::util::DeviceExt;

use crate::heat::{HEAT_FORMAT, HEAT_SIZE};

/// Formato de la escena intermedia: flotante, para que lo que brilla pueda pasar de 1.0 y
/// alimentar el bloom.
pub(crate) const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Partículas vivas a la vez (anillo).
const MAX_PARTICLES: u32 = 16384;
/// Ráfagas por fotograma; las que sobren se descartan.
const MAX_EMITTERS: usize = 256;
/// El bloom trabaja a media resolución virtual, sea cual sea el tamaño de la ventana.
const BLOOM_SIZE: (u32, u32) = (400, 300);
/// Ondas expansivas por fotograma (tamaño del array en `post.wgsl`).
pub(crate) const MAX_WAVES: usize = 16;

/// Qué efectos están activos (opciones gráficas del jugador).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Effects {
    pub bloom: bool,
    pub crt: bool,
    pub particles: bool,
    /// Explosiones procedurales en vez de los sprites.
    pub fire: bool,
    /// Ondas expansivas y calor que deforman la imagen.
    pub shockwaves: bool,
    /// Estelas a mucha velocidad, brillo del hielo y tinte de enfermedad.
    pub sprite_fx: bool,
    /// Iluminación dinámica con sombras.
    pub lighting: bool,
}

impl Default for Effects {
    fn default() -> Self {
        Effects {
            bloom: true,
            crt: false,
            particles: true,
            fire: true,
            shockwaves: true,
            sprite_fx: true,
            lighting: true,
        }
    }
}

/// Focos de luz de la cámara lenta (tamaño del array en `post.wgsl`).
pub const MAX_SPOTS: usize = 3;

/// Cámara de la composición final (cámara lenta).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// 1 = sin zoom.
    pub zoom: f32,
    /// Punto que queda quieto al hacer zoom (px virtuales).
    pub focus: [f32; 2],
    /// 0..1: desaturación y penumbra.
    pub drama: f32,
    /// Focos que se libran de la penumbra: centro (px virtuales), radio y fuerza (0 = ninguno).
    /// Sin focos, la penumbra es una viñeta en los bordes.
    pub spots: [[f32; 4]; MAX_SPOTS],
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            zoom: 1.0,
            focus: [400.0, 300.0],
            drama: 0.0,
            spots: [[0.0; 4]; MAX_SPOTS],
        }
    }
}

/// Aspecto y comportamiento de una partícula (ver `particles.wgsl`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleKind {
    /// Chispa aditiva que brilla y se estira con la velocidad.
    Spark = 0,
    /// Trozo opaco cuadrado (cascotes, sangre).
    Debris = 1,
    /// Humo: disco suave semitransparente que crece y sube.
    Smoke = 2,
    /// Confeti de colores al azar.
    Confetti = 3,
    /// Destello aditivo del color pedido, sin virar al rojo (estelas).
    Glow = 4,
}

/// Ráfaga de partículas, en coordenadas virtuales (800x600).
#[derive(Clone, Copy, Debug)]
pub struct Burst {
    pub kind: ParticleKind,
    pub count: u32,
    pub pos: [f32; 2],
    /// Semiextensión del rectángulo donde nacen.
    pub area: [f32; 2],
    /// Color lineal; las chispas admiten valores > 1 (brillan más).
    pub color: [f32; 4],
    /// Velocidad mínima y máxima (px/s), en dirección aleatoria.
    pub speed: [f32; 2],
    /// Vida mínima y máxima (s).
    pub life: [f32; 2],
    /// Velocidad común que se suma a la aleatoria.
    pub drift: [f32; 2],
    /// Radio en píxeles.
    pub size: f32,
}

impl Burst {
    pub fn new(kind: ParticleKind, pos: [f32; 2], count: u32) -> Self {
        Burst {
            kind,
            count,
            pos,
            area: [0.0; 2],
            color: [1.0; 4],
            speed: [20.0, 80.0],
            life: [0.3, 0.8],
            drift: [0.0; 2],
            size: 2.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuEmitter {
    color: [f32; 4],
    pos: [f32; 2],
    area: [f32; 2],
    speed: [f32; 2],
    life: [f32; 2],
    drift: [f32; 2],
    size: f32,
    kind: u32,
    first: u32,
    count: u32,
    seed: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimParams {
    dt: f32,
    emitters: u32,
    count: u32,
    _pad: u32,
}

/// Tamaño de `Particle` en `particles.wgsl`.
const PARTICLE_BYTES: u64 = 48;

pub(crate) struct Particles {
    buf: wgpu::Buffer,
    emitters: wgpu::Buffer,
    params: wgpu::Buffer,
    sim: wgpu::ComputePipeline,
    sim_bg: wgpu::BindGroup,
    draw: wgpu::RenderPipeline,
    draw_bg: wgpu::BindGroup,
    pending: Vec<GpuEmitter>,
    head: u32,
    seed: u32,
    dt: f32,
}

impl Particles {
    pub fn new(device: &wgpu::Device, globals: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("particles.wgsl"));
        // Empiezan todas muertas (vida 0): el búfer se crea a ceros.
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("partículas"),
            size: MAX_PARTICLES as u64 * PARTICLE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });
        let emitters = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("emisores"),
            size: (MAX_EMITTERS * std::mem::size_of::<GpuEmitter>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("parámetros de partículas"),
            size: std::mem::size_of::<SimParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sim = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("simulación de partículas"),
            layout: None,
            module: &shader,
            entry_point: Some("sim"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sim_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &sim.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: emitters.as_entire_binding(),
                },
            ],
        });
        // Alfa premultiplicado: con alfa 0 la partícula se suma (chispas), con alfa 1 tapa (trozos).
        let premultiplied = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dibujo de partículas"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: PARTICLE_BYTES,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2, 1 => Float32x2, 2 => Float32x4, 3 => Float32x3, 4 => Uint32
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: SCENE_FORMAT,
                    blend: Some(premultiplied),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let draw_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &draw.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        Particles {
            buf,
            emitters,
            params,
            sim,
            sim_bg,
            draw,
            draw_bg,
            pending: Vec::new(),
            head: 0,
            seed: 0x9e37_79b9,
            dt: 0.0,
        }
    }

    pub fn emit(&mut self, b: &Burst) {
        if self.pending.len() >= MAX_EMITTERS || b.count == 0 {
            return;
        }
        let count = b.count.min(MAX_PARTICLES);
        // Semilla distinta por ráfaga (Weyl): el aleatorio lo saca la GPU a partir de ella.
        self.seed = self.seed.wrapping_add(0x9e37_79b9);
        self.pending.push(GpuEmitter {
            color: b.color,
            pos: b.pos,
            area: b.area,
            speed: b.speed,
            life: b.life,
            drift: b.drift,
            size: b.size,
            kind: b.kind as u32,
            first: self.head,
            count,
            seed: self.seed,
            _pad: 0,
        });
        self.head = (self.head + count) % MAX_PARTICLES;
    }

    pub fn advance(&mut self, dt: f32) {
        self.dt += dt;
    }

    /// Despacha la simulación con el tiempo acumulado y las ráfagas pendientes.
    pub fn simulate(&mut self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder) {
        if self.pending.is_empty() && self.dt <= 0.0 {
            return;
        }
        // Un fotograma muy largo (ventana arrastrada, depurador) no debe teletransportarlas.
        let dt = self.dt.min(0.1);
        queue.write_buffer(&self.emitters, 0, bytemuck::cast_slice(&self.pending));
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&SimParams {
                dt,
                emitters: self.pending.len() as u32,
                count: MAX_PARTICLES,
                _pad: 0,
            }),
        );
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("partículas"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.sim);
        pass.set_bind_group(0, &self.sim_bg, &[]);
        pass.dispatch_workgroups(MAX_PARTICLES.div_ceil(64), 1, 1);
        drop(pass);
        self.pending.clear();
        self.dt = 0.0;
    }

    /// Olvida las ráfagas pendientes (efecto apagado).
    pub fn discard(&mut self) {
        self.pending.clear();
        self.dt = 0.0;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, &self.draw_bg, &[]);
        pass.set_vertex_buffer(0, self.buf.slice(..));
        pass.draw(0..4, 0..MAX_PARTICLES);
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostParams {
    bloom: f32,
    crt: f32,
    aberration: f32,
    time: f32,
    waves: u32,
    zoom: f32,
    focus: [f32; 2],
    drama: f32,
    /// 1 si hay alguna fuente de calor (si no, la composición no mira la máscara).
    heat: f32,
    _pad: [f32; 2],
    /// Focos de la cámara lenta: centro, radio y fuerza.
    spot: [[f32; 4]; MAX_SPOTS],
    /// Por onda: centro (px virtuales), progreso 0..1 y fuerza.
    wave: [[f32; 4]; MAX_WAVES],
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    (w, h): (u32, u32),
    usage: wgpu::TextureUsages,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SCENE_FORMAT,
            usage,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Bloom por compute (si la GPU lo admite).
struct Bloom {
    prefilter: wgpu::ComputePipeline,
    blur: wgpu::ComputePipeline,
    /// Escena → A, A → B (horizontal), B → A (vertical).
    groups: [wgpu::BindGroup; 3],
}

/// Escena intermedia, bloom y composición final sobre la ventana.
pub(crate) struct Post {
    pub scene: wgpu::TextureView,
    pub scene_size: (u32, u32),
    /// Máscara de calor (`heat.rs`).
    pub heat: wgpu::TextureView,
    bloom_a: wgpu::TextureView,
    bloom_b: wgpu::TextureView,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    params: wgpu::Buffer,
    dirs: [wgpu::Buffer; 2],
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    bloom: Option<Bloom>,
    bloom_shader: Option<wgpu::ShaderModule>,
}

impl Post {
    pub fn new(
        device: &wgpu::Device,
        target: wgpu::TextureFormat,
        scene_size: (u32, u32),
        compute: bool,
    ) -> Self {
        let bloom_usage = if compute {
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING
        } else {
            // Sin compute el bloom se queda a cero y la composición suma negro.
            wgpu::TextureUsages::TEXTURE_BINDING
        };
        let bloom_a = texture(device, "bloom A", BLOOM_SIZE, bloom_usage);
        let bloom_b = texture(device, "bloom B", BLOOM_SIZE, bloom_usage);
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest = device.create_sampler(&Default::default());
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("parámetros de composición"),
            size: std::mem::size_of::<PostParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dir = |d: [i32; 4]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("dirección del desenfoque"),
                contents: bytemuck::cast_slice(&d),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };
        let dirs = [dir([1, 0, 0, 0]), dir([0, 1, 0, 0])];
        let shader = device.create_shader_module(wgpu::include_wgsl!("post.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composición"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(target.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let bloom_shader =
            compute.then(|| device.create_shader_module(wgpu::include_wgsl!("bloom.wgsl")));
        let scene = texture(device, "escena", scene_size, Self::SCENE_USAGE);
        let heat = device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("máscara de calor"),
                size: wgpu::Extent3d {
                    width: HEAT_SIZE.0,
                    height: HEAT_SIZE.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HEAT_FORMAT,
                usage: Self::SCENE_USAGE,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let group = Self::group(
            device, &pipeline, &scene, &bloom_a, &heat, &nearest, &linear, &params,
        );
        let mut post = Post {
            scene,
            scene_size,
            heat,
            bloom_a,
            bloom_b,
            linear,
            nearest,
            params,
            dirs,
            pipeline,
            group,
            bloom: None,
            bloom_shader,
        };
        post.build_bloom(device);
        post
    }

    const SCENE_USAGE: wgpu::TextureUsages =
        wgpu::TextureUsages::RENDER_ATTACHMENT.union(wgpu::TextureUsages::TEXTURE_BINDING);

    #[allow(clippy::too_many_arguments)]
    fn group(
        device: &wgpu::Device,
        pipeline: &wgpu::RenderPipeline,
        scene: &wgpu::TextureView,
        bloom: &wgpu::TextureView,
        heat: &wgpu::TextureView,
        nearest: &wgpu::Sampler,
        linear: &wgpu::Sampler,
        params: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(bloom),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(nearest),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(linear),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(heat),
                },
            ],
        })
    }

    fn build_bloom(&mut self, device: &wgpu::Device) {
        let Some(shader) = &self.bloom_shader else {
            return;
        };
        let pipe = |entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: shader,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (prefilter, blur) = (pipe("prefilter"), pipe("blur"));
        // `prefilter` no usa la dirección y `blur` no usa el sampler: el diseño automático los
        // quita, así que cada grupo lleva solo lo suyo.
        let pre = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &prefilter.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.scene),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.linear),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.bloom_a),
                },
            ],
        });
        let blur_group = |src: &wgpu::TextureView, dst: &wgpu::TextureView, dir: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &blur.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(dst),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: dir.as_entire_binding(),
                    },
                ],
            })
        };
        let h = blur_group(&self.bloom_a, &self.bloom_b, &self.dirs[0]);
        let v = blur_group(&self.bloom_b, &self.bloom_a, &self.dirs[1]);
        self.bloom = Some(Bloom {
            prefilter,
            blur,
            groups: [pre, h, v],
        });
    }

    /// La escena se dibuja al tamaño del rectángulo del letterbox, para no perder resolución.
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        if size == self.scene_size {
            return;
        }
        self.scene_size = size;
        self.scene = texture(device, "escena", size, Self::SCENE_USAGE);
        self.group = Self::group(
            device,
            &self.pipeline,
            &self.scene,
            &self.bloom_a,
            &self.heat,
            &self.nearest,
            &self.linear,
            &self.params,
        );
        self.build_bloom(device);
    }

    pub fn has_compute(&self) -> bool {
        self.bloom.is_some()
    }

    /// Calcula el bloom de la escena actual (dos pasadas de desenfoque separable).
    pub fn bloom(&self, enc: &mut wgpu::CommandEncoder) {
        let Some(b) = &self.bloom else {
            return;
        };
        let groups = (BLOOM_SIZE.0.div_ceil(8), BLOOM_SIZE.1.div_ceil(8));
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("bloom"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&b.prefilter);
        pass.set_bind_group(0, &b.groups[0], &[]);
        pass.dispatch_workgroups(groups.0, groups.1, 1);
        pass.set_pipeline(&b.blur);
        for _ in 0..2 {
            for g in &b.groups[1..] {
                pass.set_bind_group(0, g, &[]);
                pass.dispatch_workgroups(groups.0, groups.1, 1);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_params(
        &self,
        queue: &wgpu::Queue,
        effects: Effects,
        shake: f32,
        time: f32,
        waves: &[[f32; 4]],
        heat: bool,
        camera: Camera,
    ) {
        let mut wave = [[0.0; 4]; MAX_WAVES];
        let n = waves.len().min(MAX_WAVES);
        wave[..n].copy_from_slice(&waves[..n]);
        let bloom = if effects.bloom && self.has_compute() {
            0.9
        } else {
            0.0
        };
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&PostParams {
                bloom,
                crt: if effects.crt { 1.0 } else { 0.0 },
                // Unos 2 px virtuales de separación de canales mientras tiembla.
                aberration: shake * 2.0 / 800.0,
                time,
                waves: n as u32,
                zoom: camera.zoom.max(1.0),
                focus: camera.focus,
                drama: camera.drama.clamp(0.0, 1.0),
                heat: if heat { 1.0 } else { 0.0 },
                _pad: [0.0; 2],
                spot: camera.spots.map(|s| {
                    if s.iter().all(|v| v.is_finite()) {
                        s
                    } else {
                        [0.0; 4]
                    }
                }),
                wave,
            }),
        );
    }

    /// Compone la escena (y el bloom) en `view`, dentro de `viewport`.
    pub fn composite(
        &self,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        viewport: [f32; 4],
    ) {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composición"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(viewport[0], viewport[1], viewport[2], viewport[3], 0.0, 1.0);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use wgpu::naga;

    /// Los shaders se compilan al crear las pipelines, ya en la GPU; esto los valida antes, sin
    /// GPU (la CI no tiene).
    #[test]
    fn shaders_are_valid_wgsl() {
        for (name, src) in [
            ("quad.wgsl", include_str!("quad.wgsl")),
            ("particles.wgsl", include_str!("particles.wgsl")),
            ("bloom.wgsl", include_str!("bloom.wgsl")),
            ("post.wgsl", include_str!("post.wgsl")),
            ("heat.wgsl", include_str!("heat.wgsl")),
            ("fire.wgsl", include_str!("fire.wgsl")),
            ("light.wgsl", include_str!("light.wgsl")),
            ("board3d.wgsl", include_str!("board3d.wgsl")),
        ] {
            let module = naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        }
    }

    /// `PARTICLE_BYTES` y `GpuEmitter` deben coincidir con los `struct` de `particles.wgsl`.
    #[test]
    fn gpu_structs_match_the_shader() {
        let src = include_str!("particles.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap();
        let size = |name: &str| {
            let (h, _) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(name))
                .unwrap();
            module.types[h].inner.size(module.to_ctx()) as usize
        };
        assert_eq!(size("Particle"), super::PARTICLE_BYTES as usize);
        assert_eq!(size("Emitter"), std::mem::size_of::<super::GpuEmitter>());
        assert_eq!(size("Params"), std::mem::size_of::<super::SimParams>());

        let src = include_str!("post.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap();
        let (h, _) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Post"))
            .unwrap();
        assert_eq!(
            module.types[h].inner.size(module.to_ctx()) as usize,
            std::mem::size_of::<super::PostParams>()
        );

        let src = include_str!("light.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap();
        let (h, _) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Lights"))
            .unwrap();
        assert_eq!(
            module.types[h].inner.size(module.to_ctx()) as usize,
            std::mem::size_of::<crate::light::LightUniform>()
        );

        let src = include_str!("board3d.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap();
        let (h, _) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Scene"))
            .unwrap();
        assert_eq!(
            module.types[h].inner.size(module.to_ctx()) as usize,
            std::mem::size_of::<crate::board3d::SceneUniform>()
        );
    }
}
