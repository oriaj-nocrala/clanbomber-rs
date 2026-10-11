//! Iluminación dinámica con sombras (`light.wgsl`).

/// Luces por fotograma (tamaño del array en `light.wgsl`); las que sobren se ignoran.
pub(crate) const MAX_LIGHTS: usize = 96;

/// Luz puntual en coordenadas virtuales. `color` se multiplica por `intensity`.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub pos: [f32; 2],
    pub radius: f32,
    pub intensity: f32,
    pub color: [f32; 3],
}

/// Rejilla que tapa la luz y luz ambiente de un fotograma.
pub struct Lighting<'a> {
    pub ambient: [f32; 3],
    /// Esquina superior izquierda de la rejilla y tamaño de casilla (px virtuales).
    pub origin: [f32; 2],
    pub cell: f32,
    /// Columnas y filas.
    pub grid: (u32, u32),
    /// Opacidad por casilla, fila a fila (255 = muro o caja).
    pub occluders: &'a [u8],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    pos: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LightUniform {
    ambient: [f32; 4],
    origin: [f32; 2],
    cell: f32,
    count: u32,
    grid: [f32; 2],
    _pad: [f32; 2],
    lights: [GpuLight; MAX_LIGHTS],
}

pub(crate) struct LightPipeline {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    sampler: wgpu::Sampler,
    /// Textura de opacidad y su grupo (se rehacen si cambia el tamaño de la rejilla).
    occ: Option<(wgpu::Texture, wgpu::BindGroup, (u32, u32))>,
    globals: wgpu::Buffer,
    pub lights: Vec<Light>,
}

impl LightPipeline {
    pub fn new(device: &wgpu::Device, globals: &wgpu::Buffer, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("light.wgsl"));
        // Multiplicativa: destino * luz. El alfa del destino se queda como está.
        let multiply = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("iluminación"),
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
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(multiply),
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
        let layout = pipeline.get_bind_group_layout(0);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("luces"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        LightPipeline {
            pipeline,
            layout,
            uniform,
            sampler,
            occ: None,
            globals: globals.clone(),
            lights: Vec::new(),
        }
    }

    /// Sube la rejilla y las luces del fotograma.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, l: &Lighting) {
        let (w, h) = l.grid;
        if self.occ.as_ref().is_none_or(|o| o.2 != l.grid) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("opacidad"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.globals.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.occ = Some((tex, group, l.grid));
        }
        let (tex, _, _) = self.occ.as_ref().expect("textura de opacidad");
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &l.occluders[..(w * h) as usize],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let mut u = LightUniform {
            ambient: [l.ambient[0], l.ambient[1], l.ambient[2], 0.0],
            origin: l.origin,
            cell: l.cell,
            count: 0,
            grid: [w as f32, h as f32],
            _pad: [0.0; 2],
            lights: [GpuLight {
                pos: [0.0; 4],
                color: [0.0; 4],
            }; MAX_LIGHTS],
        };
        for (dst, src) in u.lights.iter_mut().zip(&self.lights) {
            *dst = GpuLight {
                pos: [src.pos[0], src.pos[1], src.radius, src.intensity],
                color: [src.color[0], src.color[1], src.color[2], 0.0],
            };
        }
        u.count = self.lights.len().min(MAX_LIGHTS) as u32;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass) {
        if let Some((_, group, _)) = &self.occ {
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..4, 0..1);
        }
    }
}
