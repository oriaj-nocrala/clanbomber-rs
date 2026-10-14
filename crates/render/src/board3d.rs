//! Escena 3D sencilla (`board3d.wgsl`): cubos con textura en perspectiva, con depth buffer propio
//! y luces puntuales. Es la base del menú en tablero y no sabe nada del menú: dibuja cubos y caras
//! sueltas en un espacio de casillas (x a la derecha, y hacia arriba, z hacia la cámara).
//! Los sprites (bomber, bomba) y los textos se dibujan después con los quads 2D de siempre,
//! proyectados con [`View3d::project`].

use crate::TextureId;

/// Luces puntuales por fotograma; las que sobren se ignoran.
pub const MAX_LIGHTS_3D: usize = 8;
/// Cuadriláteros 3D por fotograma.
pub(crate) const MAX_QUADS_3D: usize = 16384;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Cámara en perspectiva y luz ambiente de la escena 3D.
#[derive(Clone, Copy, Debug)]
pub struct View3d {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    /// Apertura vertical (radianes).
    pub fov_y: f32,
    pub ambient: [f32; 3],
    /// Luz direccional suave (dirección hacia la que apunta, sin normalizar) y su color.
    pub sun_dir: [f32; 3],
    pub sun: [f32; 3],
}

impl Default for View3d {
    fn default() -> Self {
        View3d {
            eye: [0.0, 10.0, 10.0],
            target: [0.0; 3],
            fov_y: 0.6,
            ambient: [0.3; 3],
            sun_dir: [0.3, -1.0, -0.4],
            sun: [0.4; 3],
        }
    }
}

impl View3d {
    /// Matriz vista-proyección (por columnas, como la espera WGSL) para el lienzo 4:3.
    pub fn view_proj(&self) -> [[f32; 4]; 4] {
        let aspect = crate::VIRTUAL_W / crate::VIRTUAL_H;
        mat_mul(
            perspective(self.fov_y, aspect, 0.1, 200.0),
            look_at(self.eye, self.target, [0.0, 1.0, 0.0]),
        )
    }

    /// Punto del espacio 3D en coordenadas del lienzo virtual (800x600) y su profundidad
    /// (0 cerca, 1 lejos). `None` si queda detrás de la cámara.
    pub fn project(&self, p: [f32; 3]) -> Option<[f32; 3]> {
        let m = self.view_proj();
        let c: [f32; 4] =
            std::array::from_fn(|i| m[0][i] * p[0] + m[1][i] * p[1] + m[2][i] * p[2] + m[3][i]);
        if c[3] <= 1e-4 {
            return None;
        }
        let (x, y, z) = (c[0] / c[3], c[1] / c[3], c[2] / c[3]);
        Some([
            (x + 1.0) * 0.5 * crate::VIRTUAL_W,
            (1.0 - y) * 0.5 * crate::VIRTUAL_H,
            z,
        ])
    }

    /// Píxeles virtuales que mide una casilla (1 unidad) a la altura de `p`, de lado a lado.
    pub fn scale_at(&self, p: [f32; 3]) -> f32 {
        match (self.project(p), self.project([p[0] + 1.0, p[1], p[2]])) {
            (Some(a), Some(b)) => (b[0] - a[0]).hypot(b[1] - a[1]),
            _ => 0.0,
        }
    }
}

/// Luz puntual de la escena 3D: se apaga del todo a `radius` casillas.
#[derive(Clone, Copy, Debug)]
pub struct Light3d {
    pub pos: [f32; 3],
    pub radius: f32,
    /// Color lineal; puede pasar de 1.
    pub color: [f32; 3],
}

/// Caras de un [`Cube`]. `ALL` no lleva la de abajo, que solo se ve si el cubo vuela o gira.
pub mod faces {
    pub const TOP: u8 = 1;
    pub const FRONT: u8 = 2;
    pub const BACK: u8 = 4;
    pub const LEFT: u8 = 8;
    pub const RIGHT: u8 = 16;
    pub const BOTTOM: u8 = 32;
    pub const SIDES: u8 = FRONT | BACK | LEFT | RIGHT;
    pub const ALL: u8 = TOP | SIDES;
}

/// Prisma alineado con los ejes, con una región de textura arriba y otra en los lados.
#[derive(Clone, Copy, Debug)]
pub struct Cube {
    pub tex: TextureId,
    /// Esquina de coordenadas mínimas y tamaño.
    pub min: [f32; 3],
    pub size: [f32; 3],
    /// Regiones de la textura (x, y, w, h en texels).
    pub top: [f32; 4],
    pub side: [f32; 4],
    /// Color que multiplica la textura (lineal; puede pasar de 1 para que brille).
    pub tint: [f32; 4],
    /// Oscurecimiento de la base de los lados (oclusión ambiental falsa), 0 = nada.
    pub ao: f32,
    /// Qué caras dibujar (`faces::*`).
    pub faces: u8,
    /// Giro alrededor del centro (radianes, sobre x, luego y, luego z); cero = alineado.
    pub spin: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex3d {
    pos: [f32; 3],
    uv: [f32; 2],
    normal: [f32; 3],
    color: [f32; 4],
}

/// Lo que lee `board3d.wgsl` (`Scene`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    ambient: [f32; 4],
    sun_dir: [f32; 4],
    sun: [f32; 4],
    /// x = luces en uso.
    info: [f32; 4],
    /// xyz y radio.
    light_pos: [[f32; 4]; MAX_LIGHTS_3D],
    light_color: [[f32; 4]; MAX_LIGHTS_3D],
}

/// Geometría acumulada en el fotograma.
#[derive(Default)]
pub(crate) struct Scene3d {
    pub verts: Vec<Vertex3d>,
    pub lights: Vec<Light3d>,
    pub view: View3d,
}

impl Scene3d {
    /// Añade un cuadrilátero (`corners` en orden arriba-izquierda, arriba-derecha,
    /// abajo-izquierda, abajo-derecha de la textura). Devuelve `false` si no cabe.
    pub fn push_face(
        &mut self,
        corners: [[f32; 3]; 4],
        uv: [f32; 4],
        normal: [f32; 3],
        colors: [[f32; 4]; 4],
    ) -> bool {
        if self.verts.len() / 4 >= MAX_QUADS_3D {
            return false;
        }
        let [u0, v0, u1, v1] = uv;
        let uvs = [[u0, v0], [u1, v0], [u0, v1], [u1, v1]];
        for i in 0..4 {
            self.verts.push(Vertex3d {
                pos: corners[i],
                uv: uvs[i],
                normal,
                color: colors[i],
            });
        }
        true
    }

    /// Añade las caras pedidas de `c`; `tex_size` es el tamaño de su textura. Devuelve los
    /// cuadriláteros añadidos.
    pub fn push_cube(&mut self, c: &Cube, tex_size: (f32, f32)) -> u32 {
        let uv = |r: [f32; 4]| {
            [
                r[0] / tex_size.0,
                r[1] / tex_size.1,
                (r[0] + r[2]) / tex_size.0,
                (r[1] + r[3]) / tex_size.1,
            ]
        };
        let [x0, y0, z0] = c.min;
        let [x1, y1, z1] = [x0 + c.size[0], y0 + c.size[1], z0 + c.size[2]];
        let t = c.tint;
        let low = [
            t[0] * (1.0 - c.ao),
            t[1] * (1.0 - c.ao),
            t[2] * (1.0 - c.ao),
            t[3],
        ];
        let side_colors = [t, t, low, low];
        let rot = (c.spin != [0.0; 3]).then(|| rotation(c.spin));
        let center = [(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0];
        let turn = |p: [f32; 3], about: [f32; 3]| match rot {
            Some(m) => {
                let d = [p[0] - about[0], p[1] - about[1], p[2] - about[2]];
                std::array::from_fn(|i| about[i] + m[i][0] * d[0] + m[i][1] * d[1] + m[i][2] * d[2])
            }
            None => p,
        };
        let mut n = 0;
        let mut face = |bit: u8, corners: [[f32; 3]; 4], uvr, normal, colors| {
            let corners = corners.map(|p| turn(p, center));
            let normal = turn(normal, [0.0; 3]);
            if c.faces & bit != 0 && self.push_face(corners, uv(uvr), normal, colors) {
                n += 1;
            }
        };
        face(
            faces::TOP,
            [[x0, y1, z0], [x1, y1, z0], [x0, y1, z1], [x1, y1, z1]],
            c.top,
            [0.0, 1.0, 0.0],
            [t; 4],
        );
        if c.size[1] > 0.0 {
            face(
                faces::FRONT,
                [[x0, y1, z1], [x1, y1, z1], [x0, y0, z1], [x1, y0, z1]],
                c.side,
                [0.0, 0.0, 1.0],
                side_colors,
            );
            face(
                faces::BACK,
                [[x1, y1, z0], [x0, y1, z0], [x1, y0, z0], [x0, y0, z0]],
                c.side,
                [0.0, 0.0, -1.0],
                side_colors,
            );
            face(
                faces::LEFT,
                [[x0, y1, z0], [x0, y1, z1], [x0, y0, z0], [x0, y0, z1]],
                c.side,
                [-1.0, 0.0, 0.0],
                side_colors,
            );
            face(
                faces::RIGHT,
                [[x1, y1, z1], [x1, y1, z0], [x1, y0, z1], [x1, y0, z0]],
                c.side,
                [1.0, 0.0, 0.0],
                side_colors,
            );
            face(
                faces::BOTTOM,
                [[x0, y0, z1], [x1, y0, z1], [x0, y0, z0], [x1, y0, z0]],
                c.top,
                [0.0, -1.0, 0.0],
                [low; 4],
            );
        }
        n
    }

    pub fn clear(&mut self) {
        self.verts.clear();
        self.lights.clear();
    }

    fn uniform(&self) -> SceneUniform {
        let v = &self.view;
        let mut u = SceneUniform {
            view_proj: v.view_proj(),
            ambient: [v.ambient[0], v.ambient[1], v.ambient[2], 0.0],
            sun_dir: {
                let d = normalize(v.sun_dir);
                [d[0], d[1], d[2], 0.0]
            },
            sun: [v.sun[0], v.sun[1], v.sun[2], 0.0],
            info: [self.lights.len().min(MAX_LIGHTS_3D) as f32, 0.0, 0.0, 0.0],
            light_pos: [[0.0; 4]; MAX_LIGHTS_3D],
            light_color: [[0.0; 4]; MAX_LIGHTS_3D],
        };
        for (i, l) in self.lights.iter().take(MAX_LIGHTS_3D).enumerate() {
            u.light_pos[i] = [l.pos[0], l.pos[1], l.pos[2], l.radius.max(0.01)];
            u.light_color[i] = [l.color[0], l.color[1], l.color[2], 0.0];
        }
        u
    }
}

pub(crate) struct Board3dPipeline {
    pipeline: wgpu::RenderPipeline,
    group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    depth: wgpu::TextureView,
}

impl Board3dPipeline {
    pub fn new(
        device: &wgpu::Device,
        tex_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        size: (u32, u32),
    ) -> Self {
        use wgpu::util::DeviceExt;
        let shader = device.create_shader_module(wgpu::include_wgsl!("board3d.wgsl"));
        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("escena 3d"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&scene_layout), Some(tex_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tablero 3d"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3d>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x4
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            // Sin descartar caras traseras: el orden de los vértices no importa.
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("escena 3d"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &scene_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vértices 3d"),
            size: (MAX_QUADS_3D * 4 * std::mem::size_of::<Vertex3d>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices: Vec<u32> = (0..MAX_QUADS_3D as u32)
            .flat_map(|q| [0, 1, 2, 2, 1, 3].map(|i| q * 4 + i))
            .collect();
        let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("índices 3d"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Board3dPipeline {
            pipeline,
            group,
            uniform,
            vbuf,
            ibuf,
            depth: depth_view(device, size),
        }
    }

    /// La profundidad va al tamaño de la escena intermedia.
    pub fn resize(&mut self, device: &wgpu::Device, size: (u32, u32)) {
        self.depth = depth_view(device, size);
    }

    pub fn upload(&self, queue: &wgpu::Queue, scene: &Scene3d) {
        if scene.verts.is_empty() {
            return;
        }
        queue.write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&scene.verts));
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&scene.uniform()));
    }

    /// Abre una pasada sobre `target` (sin borrarlo) con la profundidad limpia y dibuja los
    /// lotes `(textura, primer cuadrilátero, cuadriláteros)`.
    pub fn draw(
        &self,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        batches: &[(&wgpu::BindGroup, u32, u32)],
    ) {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tablero 3d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        pass.set_index_buffer(self.ibuf.slice(..), wgpu::IndexFormat::Uint32);
        for &(tex, first, count) in batches {
            pass.set_bind_group(1, tex, &[]);
            pass.draw_indexed(first * 6..(first + count) * 6, 0, 0..1);
        }
    }
}

fn depth_view(device: &wgpu::Device, (w, h): (u32, u32)) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("profundidad 3d"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Matriz de giro (por filas) sobre x, luego y, luego z.
fn rotation([ax, ay, az]: [f32; 3]) -> [[f32; 3]; 3] {
    let (sx, cx) = ax.sin_cos();
    let (sy, cy) = ay.sin_cos();
    let (sz, cz) = az.sin_cos();
    let rx = [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]];
    let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
    let rz = [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]];
    let mul = |a: [[f32; 3]; 3], b: [[f32; 3]; 3]| -> [[f32; 3]; 3] {
        std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
    };
    mul(rz, mul(ry, rx))
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Perspectiva de mano derecha con profundidad 0..1 (la de wgpu). Por columnas.
fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y / 2.0).tan();
    let r = far / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, r, -1.0],
        [0.0, 0.0, r * near, 0.0],
    ]
}

/// Matriz de vista de mano derecha (la cámara mira hacia -z). Por columnas.
fn look_at(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
    let s = normalize(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

/// `a * b` con matrices por columnas.
fn mat_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_projects_to_the_center() {
        let v = View3d {
            eye: [3.0, 8.0, 9.0],
            target: [3.0, 0.0, 2.0],
            ..Default::default()
        };
        let p = v.project(v.target).unwrap();
        assert!(
            (p[0] - 400.0).abs() < 1e-3 && (p[1] - 300.0).abs() < 1e-3,
            "{p:?}"
        );
        assert!((0.0..1.0).contains(&p[2]));
        // Más cerca de la cámara, más abajo en pantalla y menos profundo.
        let q = v.project([3.0, 0.0, 4.0]).unwrap();
        assert!(q[1] > p[1] && q[2] < p[2]);
        // A la derecha en el mundo, a la derecha en pantalla.
        assert!(v.project([4.0, 0.0, 2.0]).unwrap()[0] > 400.0);
        assert!(v.project([3.0, 0.0, 20.0]).is_none(), "detrás de la cámara");
    }

    #[test]
    fn cube_faces_follow_the_mask() {
        let mut s = Scene3d::default();
        let mut c = Cube {
            tex: TextureId(0),
            min: [0.0; 3],
            size: [1.0; 3],
            top: [0.0, 0.0, 1.0, 1.0],
            side: [0.0, 0.0, 1.0, 1.0],
            tint: [1.0; 4],
            ao: 0.0,
            faces: faces::ALL,
            spin: [0.0; 3],
        };
        assert_eq!(s.push_cube(&c, (1.0, 1.0)), 5);
        c.faces = faces::TOP | faces::FRONT;
        assert_eq!(s.push_cube(&c, (1.0, 1.0)), 2);
        // Sin altura solo queda la cara de arriba.
        c.size[1] = 0.0;
        c.faces = faces::ALL;
        assert_eq!(s.push_cube(&c, (1.0, 1.0)), 1);
    }

    #[test]
    fn spin_turns_around_the_center() {
        let mut s = Scene3d::default();
        let c = Cube {
            tex: TextureId(0),
            min: [0.0; 3],
            size: [2.0; 3],
            top: [0.0, 0.0, 1.0, 1.0],
            side: [0.0, 0.0, 1.0, 1.0],
            tint: [1.0; 4],
            ao: 0.0,
            faces: faces::TOP,
            spin: [std::f32::consts::FRAC_PI_2, 0.0, 0.0],
        };
        s.push_cube(&c, (1.0, 1.0));
        // Un cuarto de vuelta sobre x: la cara de arriba pasa a mirar hacia la cámara (+z).
        let v = &s.verts[0];
        assert!((v.normal[2] - 1.0).abs() < 1e-5, "{:?}", v.normal);
        assert!((v.pos[2] - 2.0).abs() < 1e-5, "{:?}", v.pos);
    }
}
