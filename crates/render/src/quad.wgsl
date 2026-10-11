// `scale`: píxeles físicos por píxel virtual (tamaño de la escena / 800).
struct Globals { screen: vec2<f32>, time: f32, scale: f32 };
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
// Reflejo en el hielo: el sprite (ya invertido por los UV) con rielado por líneas.
const REFLECT: u32 = 4u;
// Muerte: el sprite se quema desde un punto hacia fuera, con un frente incandescente...
const BURN: u32 = 5u;
// ...y el cadáver aparece por los mismos píxeles. El avance (0..1) va en los 16 bits altos del
// modo y el centro y radio de la quemadura en `color.rgb` (px virtuales), para no tocar el
// formato del vértice.
const REVEAL: u32 = 6u;

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
    // Las posiciones pueden ir entre píxeles virtuales (interpolación entre ticks): se ajustan
    // a la rejilla de píxeles físicos para que el pixel art no tiemble ni pierda filas.
    let snapped = round(p * g.scale) / g.scale;
    // Coordenadas virtuales en píxeles (origen arriba-izquierda) -> clip space.
    let ndc = vec2<f32>(snapped.x / g.screen.x * 2.0 - 1.0, 1.0 - snapped.y / g.screen.y * 2.0);
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

// Patrón de la quemadura (0..1): sobre todo la distancia al centro (un frente que avanza),
// deformada por ruido de valor para que el borde sea irregular.
fn burn_field(world: vec2<f32>, color: vec4<f32>) -> f32 {
    let q = floor(world);
    let radial = clamp(length(q + 0.5 - color.xy) / max(color.z, 1.0), 0.0, 1.0);
    let p = q / 4.0;
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n = mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y,
    );
    // Un poco de grano por píxel: bordes que se desmoronan en vez de curvas lisas.
    let noise = n * 0.9 + hash(q + 911.0) * 0.1;
    return radial * 0.6 + noise * 0.4;
}

// Umbral del patrón según el avance: con 0 no se ha quemado nada y con 1 todo.
fn burn_threshold(mode: u32) -> f32 {
    let progress = f32(mode >> 16u) / 65535.0;
    return mix(-0.2, 1.2, progress);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, samp, in.uv);
    switch in.mode & 0xffffu {
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
        case REFLECT: {
            // Líneas que rielan despacio, como una superficie que no está del todo quieta.
            let row = floor(in.world.y);
            let shimmer = 0.75 + 0.25 * sin(row * 0.9 - g.time * 5.0);
            return vec4<f32>(texel.rgb * in.color.rgb, texel.a * in.color.a * shimmer);
        }
        case BURN: {
            let f = burn_field(in.world, in.color);
            let th = burn_threshold(in.mode);
            // Ya quemado: nada. En el frente, brasa "HDR": blanca justo en el borde, naranja y
            // luego rojo oscuro hacia el sprite que aún no arde.
            let keep = step(th, f);
            let e = 1.0 - smoothstep(0.0, 0.12, f - th);
            let hot = mix(vec3<f32>(1.2, 0.2, 0.03), vec3<f32>(4.5, 2.6, 1.0), e * e);
            return vec4<f32>(mix(texel.rgb, hot, smoothstep(0.0, 0.35, e)), texel.a * keep);
        }
        case REVEAL: {
            let f = burn_field(in.world, in.color);
            let th = burn_threshold(in.mode);
            // Aparece por donde ya se quemó (con un poco de solape para que no quede hueco),
            // chamuscado junto al frente.
            let keep = step(f, th + 0.03);
            let scorch = smoothstep(th - 0.1, th + 0.03, f);
            return vec4<f32>(texel.rgb * mix(1.0, 0.3, scorch), texel.a * in.color.a * keep);
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
