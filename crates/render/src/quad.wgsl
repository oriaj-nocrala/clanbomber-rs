struct Globals { screen: vec2<f32>, time: f32, _pad: f32 };
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

// Modos de dibujo (`Vertex::mode` en lib.rs).
const NORMAL: u32 = 0u;
// Silueta: la forma del sprite (su alfa) rellena del color del vértice (estelas).
const SILHOUETTE: u32 = 1u;
// Brillo de hielo: destello diagonal que barre la casilla, calculado aquí (sin textura).
const ICE: u32 = 2u;
// Tono que gira con el tiempo (bomber "stoned").
const HUE: u32 = 3u;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec2<f32>,
    @location(3) @interpolate(flat) mode: u32,
};

@vertex
fn vs(
    @location(0) p: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) mode: u32,
) -> VsOut {
    // Coordenadas virtuales en píxeles (origen arriba-izquierda) -> clip space.
    let ndc = vec2<f32>(p.x / g.screen.x * 2.0 - 1.0, 1.0 - p.y / g.screen.y * 2.0);
    return VsOut(vec4<f32>(ndc, 0.0, 1.0), uv, color, p, mode);
}

fn hash(p: vec2<f32>) -> f32 {
    let q = vec2<u32>(vec2<i32>(floor(p)) + 4096);
    var h = q.x * 374761393u + q.y * 668265263u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h ^ (h >> 16u)) / 4294967295.0;
}

fn rotate_hue(c: vec3<f32>, angle: f32) -> vec3<f32> {
    // Rotación alrededor del eje gris (fórmula de Rodrigues).
    let k = vec3<f32>(0.57735);
    let cs = cos(angle);
    return c * cs + cross(k, c) * sin(angle) + k * dot(k, c) * (1.0 - cs);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, samp, in.uv);
    switch in.mode {
        case SILHOUETTE: {
            return vec4<f32>(in.color.rgb, texel.a * in.color.a);
        }
        case ICE: {
            // Cada píxel virtual por separado, como el resto del pixel art.
            let q = floor(in.world);
            // Banda diagonal que cruza el mapa cada ~3.5 s.
            let phase = (q.x + q.y) * 0.004 - g.time * 0.28;
            let band = fract(phase);
            let shine = smoothstep(0.0, 0.03, band) * (1.0 - smoothstep(0.03, 0.09, band));
            // Destellos sueltos que parpadean.
            let cell = floor(q / 3.0);
            let tw = hash(cell + floor(g.time * 2.0) * 17.0);
            let sparkle = select(0.0, 1.0, tw > 0.996);
            let a = (shine * 0.35 + sparkle * 0.8) * in.color.a;
            return vec4<f32>(vec3<f32>(1.3, 1.45, 1.6), a);
        }
        case HUE: {
            let c = rotate_hue(texel.rgb, g.time * 4.0);
            return vec4<f32>(mix(texel.rgb, c, in.color.a), texel.a);
        }
        default: {
            return texel * in.color;
        }
    }
}
