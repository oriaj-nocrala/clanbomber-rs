// Máscara de calor: cada fuente es un rectángulo de bordes suaves que se suma (en px virtuales)
// sobre una textura de 400x300. La composición final (`post.wgsl`) deforma la imagen donde hay calor.

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // Posición respecto al centro del rectángulo (px virtuales).
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half_size: vec2<f32>,
    // Intensidad y ancho del borde suave.
    @location(2) @interpolate(flat) params: vec2<f32>,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) params: vec2<f32>,
) -> VsOut {
    let soft = params.y;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    // El quad se agranda con el borde suave para que el degradado no se corte.
    let p = rect.xy - soft + corner * (rect.zw + 2.0 * soft);
    let ndc = vec2<f32>(p.x / 800.0 * 2.0 - 1.0, 1.0 - p.y / 600.0 * 2.0);
    let half_size = rect.zw * 0.5;
    return VsOut(vec4<f32>(ndc, 0.0, 1.0), p - (rect.xy + half_size), half_size, params);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Distancia con signo a un rectángulo de esquinas redondeadas (negativa dentro).
    let r = min(in.params.y, min(in.half_size.x, in.half_size.y));
    let q = abs(in.local) - in.half_size + r;
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
    let soft = max(in.params.y, 1.0);
    let h = in.params.x * smoothstep(soft, -soft * 0.5, d);
    return vec4<f32>(h, 0.0, 0.0, 1.0);
}
