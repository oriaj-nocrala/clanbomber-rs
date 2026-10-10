//! Renderer 2D sobre wgpu: batcher de quads en resolución virtual 800x600
//! con letterboxing. Las texturas y las partículas por compute se añaden aquí.

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

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

struct Batch {
    tex: TextureId,
    first_quad: u32,
    quads: u32,
}

/// Hoja de sprites con celdas de tamaño fijo, numeradas en fila.
#[derive(Clone, Copy)]
pub struct Sheet {
    pub tex: TextureId,
    pub cell_w: f32,
    pub cell_h: f32,
    pub cols: u32,
}

impl Sheet {
    pub fn cell(&self, index: u32) -> [f32; 4] {
        let (cx, cy) = (index % self.cols, index / self.cols);
        [
            cx as f32 * self.cell_w,
            cy as f32 * self.cell_h,
            self.cell_w,
            self.cell_h,
        ]
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: Vec<Texture>,
    batches: Vec<Batch>,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    verts: Vec<Vertex>,
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
            contents: bytemuck::cast_slice(&[VIRTUAL_W, VIRTUAL_H, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
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

        let mut r = Renderer {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            tex_layout,
            sampler,
            textures: Vec::new(),
            batches: Vec::new(),
            vbuf,
            ibuf,
            verts: Vec::with_capacity(MAX_QUADS * 4),
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

    pub fn load_rgba(&mut self, rgba: &[u8], w: u32, h: u32) -> TextureId {
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
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
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
    }

    /// Rectángulo relleno en coordenadas virtuales (800x600).
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.push_quad(TextureId(0), [x, y, w, h], [0.0; 4], color);
    }

    /// Región `src` (x, y, w, h en píxeles de la textura) dibujada en `dst`.
    pub fn sprite(&mut self, tex: TextureId, src: [f32; 4], dst: [f32; 4]) {
        self.push_quad(tex, dst, src, [1.0; 4]);
    }

    /// Celda `index` de una hoja de sprites, en (x, y) a tamaño nativo.
    pub fn sheet_sprite(&mut self, sheet: &Sheet, index: u32, x: f32, y: f32) {
        self.sprite(
            sheet.tex,
            sheet.cell(index),
            [x, y, sheet.cell_w, sheet.cell_h],
        );
    }

    pub fn sheet(&self, tex: TextureId, cell_w: f32, cell_h: f32) -> Sheet {
        Sheet {
            tex,
            cell_w,
            cell_h,
            cols: (self.textures[tex.0].size.0 / cell_w) as u32,
        }
    }

    fn push_quad(&mut self, tex: TextureId, dst: [f32; 4], src: [f32; 4], color: [f32; 4]) {
        let quad_index = (self.verts.len() / 4) as u32;
        if quad_index as usize >= MAX_QUADS {
            return;
        }
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
        };
        self.verts.extend([
            v(x, y, u0, v0),
            v(x + w, y, u1, v0),
            v(x, y + h, u0, v1),
            v(x + w, y + h, u1, v1),
        ]);
        match self.batches.last_mut() {
            Some(b) if b.tex == tex => b.quads += 1,
            _ => self.batches.push(Batch {
                tex,
                first_quad: quad_index,
                quads: 1,
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
                self.verts.clear();
                self.batches.clear();
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());
        self.queue
            .write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&self.verts));

        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            // Letterbox 4:3 centrado.
            let (sw, sh) = (self.config.width as f32, self.config.height as f32);
            let scale = (sw / VIRTUAL_W).min(sh / VIRTUAL_H);
            let (vw, vh) = (VIRTUAL_W * scale, VIRTUAL_H * scale);
            pass.set_viewport((sw - vw) / 2.0, (sh - vh) / 2.0, vw, vh, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vbuf.slice(..));
            pass.set_index_buffer(self.ibuf.slice(..), wgpu::IndexFormat::Uint32);
            for b in &self.batches {
                pass.set_bind_group(1, &self.textures[b.tex.0].bind_group, &[]);
                pass.draw_indexed(b.first_quad * 6..(b.first_quad + b.quads) * 6, 0, 0..1);
            }
        }
        self.queue.submit([enc.finish()]);
        self.queue.present(frame);
        self.verts.clear();
        self.batches.clear();
    }
}
