//! Explosiones procedurales (`fire.wgsl`): un quad por explosión que cubre su cruz.

/// Explosiones por fotograma; las que sobren no se dibujan.
pub(crate) const MAX_FIRES: usize = 512;

/// Una explosión, en coordenadas virtuales.
#[derive(Clone, Copy, Debug)]
pub struct Fire {
    /// Centro de la casilla central.
    pub center: [f32; 2],
    /// Casillas de cada brazo: izquierda, derecha, arriba, abajo.
    pub arms: [i32; 4],
    pub tile: f32,
    /// Potencia de la bomba (decide el grosor).
    pub power: f32,
    /// 0 al estallar, 1 al apagarse.
    pub progress: f32,
    /// Distinta por explosión y estable entre fotogramas.
    pub seed: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct FireInstance {
    rect: [f32; 4],
    center: [f32; 2],
    arms: [f32; 4],
    params: [f32; 4],
}

impl Fire {
    pub(crate) fn instance(&self) -> FireInstance {
        // Cada brazo llega al borde de su última casilla.
        let reach = self.arms.map(|n| (n as f32 + 0.5) * self.tile);
        // Margen para la turbulencia del borde.
        let m = 8.0;
        let [cx, cy] = self.center;
        let (x0, x1) = (cx - reach[0] - m, cx + reach[1] + m);
        let (y0, y1) = (cy - reach[2] - m, cy + reach[3] + m);
        FireInstance {
            rect: [x0, y0, x1 - x0, y1 - y0],
            center: self.center,
            arms: reach,
            params: [
                self.power,
                self.progress.clamp(0.0, 1.0),
                (self.seed % 65536) as f32,
                self.tile,
            ],
        }
    }
}

pub(crate) struct FirePipeline {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    buf: wgpu::Buffer,
}

impl FirePipeline {
    pub fn new(device: &wgpu::Device, globals: &wgpu::Buffer, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("fire.wgsl"));
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("explosiones"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FireInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x2, 2 => Float32x4, 3 => Float32x4
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("explosiones"),
            size: (MAX_FIRES * std::mem::size_of::<FireInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        FirePipeline {
            pipeline,
            group,
            buf,
        }
    }

    pub fn upload(&self, queue: &wgpu::Queue, fires: &[FireInstance]) {
        if !fires.is_empty() {
            queue.write_buffer(&self.buf, 0, bytemuck::cast_slice(fires));
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass, first: u32, count: u32) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.buf.slice(..));
        pass.draw(0..4, first..first + count);
    }
}
