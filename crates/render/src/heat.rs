//! Calor que deforma el aire (`heat.wgsl` + `post.wgsl`). Las fuentes se suman en una máscara
//! pequeña (el calor no tiene detalle fino) y la composición final desplaza la imagen con un ruido
//! que sube allí donde la máscara no es cero.

/// Fuentes por fotograma; las que sobren se ignoran.
pub(crate) const MAX_HEAT: usize = 256;
/// La máscara va a media resolución virtual y se lee con filtro bilineal.
pub(crate) const HEAT_SIZE: (u32, u32) = (400, 300);
pub(crate) const HEAT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct HeatInstance {
    /// x, y, ancho y alto en px virtuales (sin el borde suave).
    pub rect: [f32; 4],
    /// Intensidad y ancho del borde suave.
    pub params: [f32; 2],
}

pub(crate) struct HeatPipeline {
    pipeline: wgpu::RenderPipeline,
    buf: wgpu::Buffer,
    pub sources: Vec<HeatInstance>,
}

impl HeatPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("heat.wgsl"));
        // Se suman: dos llamas que se cruzan calientan más.
        let add = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("calor"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HeatInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x2],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HEAT_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: add,
                        alpha: add,
                    }),
                    write_mask: wgpu::ColorWrites::RED,
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
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("calor"),
            size: (MAX_HEAT * std::mem::size_of::<HeatInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        HeatPipeline {
            pipeline,
            buf,
            sources: Vec::new(),
        }
    }

    /// Rehace la máscara con las fuentes de este fotograma (o la deja a cero).
    pub fn draw(
        &self,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        mask: &wgpu::TextureView,
    ) {
        if !self.sources.is_empty() {
            queue.write_buffer(&self.buf, 0, bytemuck::cast_slice(&self.sources));
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("calor"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: mask,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if !self.sources.is_empty() {
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.buf.slice(..));
            pass.draw(0..4, 0..self.sources.len() as u32);
        }
    }
}
