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
// Cronómetro dibujado entero aquí (sin textura): `uv` va de 0 a 1 sobre el quad y `color` lleva
// la vuelta de la aguja de segundos (r), la de la subesfera (g), la alarma (b) y el alfa (a).
const WATCH: u32 = 7u;
// Texto en campo de distancias (`text.rs`): el alfa de la textura es la distancia al borde de la
// letra (0.5 en el borde). Bits 16-23 del modo: cuánto engordar la letra (contorno y sombra);
// bits 24-31: cuánto difuminar el borde (sombra suave). Ambos en 0..0.5 unidades de distancia.
const TEXT: u32 = 8u;

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
    // El texto no: su borde ya se recorta por píxel físico y ajustar cada esquina deformaría
    // las letras pequeñas.
    let snapped = select(round(p * g.scale) / g.scale, p, (mode & 0xffffu) == TEXT);
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

// Cobertura de una forma con distancia con signo `d` (negativa dentro), suavizada en `aa`.
fn fill(d: f32, aa: f32) -> f32 {
    return clamp(0.5 - d / aa, 0.0, 1.0);
}

fn sd_box(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn sd_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Gira `p` para que el eje -y apunte a `angle` (0 arriba, en sentido horario).
fn to_local(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * p.x + s * p.y, -s * p.x + c * p.y);
}

// Metal cromado: `shade` es cuánto mira la pieza hacia la luz (arriba a la izquierda).
fn chrome(shade: f32) -> vec3<f32> {
    let base = mix(vec3<f32>(0.22, 0.23, 0.26), vec3<f32>(0.86, 0.87, 0.9), clamp(shade, 0.0, 1.0));
    let glint = pow(clamp(shade, 0.0, 1.0), 12.0) * 0.9;
    return base + glint;
}

// Capa sobre lo acumulado (premultiplicado): `acc.rgb` color por alfa, `acc.a` alfa.
fn over(acc: vec4<f32>, color: vec3<f32>, alpha: f32) -> vec4<f32> {
    return vec4<f32>(acc.rgb * (1.0 - alpha) + color * alpha, acc.a * (1.0 - alpha) + alpha);
}

// Cronómetro antiguo. Unidades: radio de la caja = 1, centro de la esfera en el origen, y
// hacia abajo; el quad mide 2 x 2.4 (la corona sobresale por arriba).
fn stopwatch(uv: vec2<f32>, aa: f32, params: vec4<f32>) -> vec4<f32> {
    let p = vec2<f32>((uv.x - 0.5) * 2.0, uv.y * 2.4 - 1.4);
    let light = normalize(vec2<f32>(-0.55, -0.85));
    let r = length(p);
    var acc = vec4<f32>(0.0);

    // Sombra sobre el fondo.
    let shadow = 1.0 - smoothstep(0.9, 1.12, length(p - vec2<f32>(0.07, 0.09)));
    acc = over(acc, vec3<f32>(0.0), shadow * 0.45);

    // Argolla, vástago y botón de la corona.
    let bow = abs(length(p - vec2<f32>(0.0, -1.27)) - 0.12) - 0.032;
    let bow_shade = 0.55 + 0.45 * dot(normalize(p - vec2<f32>(0.0, -1.27)), light);
    acc = over(acc, chrome(bow_shade), fill(bow, aa));
    let stem = sd_box(p - vec2<f32>(0.0, -1.04), vec2<f32>(0.07, 0.1), 0.01);
    acc = over(acc, chrome(0.75 - abs(p.x) * 6.0), fill(stem, aa));
    let crown = sd_box(p - vec2<f32>(0.0, -1.14), vec2<f32>(0.115, 0.055), 0.03);
    acc = over(acc, chrome(0.9 - abs(p.x) * 4.0 - (p.y + 1.14) * 3.0), fill(crown, aa));

    // Botones laterales, a 40 grados de la corona.
    for (var side = -1.0; side <= 1.0; side += 2.0) {
        let q = to_local(p, side * 0.7);
        let post = sd_box(q - vec2<f32>(0.0, -1.06), vec2<f32>(0.055, 0.09), 0.01);
        acc = over(acc, chrome(0.7 - abs(q.x) * 7.0), fill(post, aa));
        let cap = sd_box(q - vec2<f32>(0.0, -1.15), vec2<f32>(0.09, 0.045), 0.025);
        acc = over(acc, chrome(0.85 - abs(q.x) * 5.0), fill(cap, aa));
    }

    // Caja: anillo cromado, más claro hacia la luz y con una banda de brillo a lo largo.
    let n = p / max(r, 1e-4);
    let t = clamp((1.0 - r) / 0.16, 0.0, 1.0);
    let facing = dot(n, light) * (1.0 - 2.0 * t);
    let band = 0.12 * sin(atan2(p.y, p.x) * 3.0 + 1.3);
    // El filo exterior, más oscuro, le da volumen y la separa del fondo.
    let case_color = chrome(0.5 + 0.45 * facing + band) * (1.0 - 0.35 * smoothstep(0.93, 1.0, r));
    acc = over(acc, case_color, fill(r - 1.0, aa));

    // Esfera, algo hundida (sombra junto al bisel). Con la alarma se tiñe de rojo.
    let alarm = params.z;
    var dial = vec3<f32>(0.8, 0.8, 0.77) * (1.0 - 0.1 * r * r);
    dial *= 1.0 - 0.45 * smoothstep(0.7, 0.84, r) * (0.5 + 0.5 * dot(n, -light));
    dial = mix(dial, vec3<f32>(0.85, 0.16, 0.12), alarm * 0.55);
    acc = over(acc, dial, fill(r - 0.84, aa));

    // Marcas: 60 finas y 12 largas.
    let angle = atan2(p.x, -p.y);
    let step60 = 6.2831853 / 60.0;
    let idx = round(angle / step60);
    let major = abs(idx % 5.0) < 0.5;
    let tick = to_local(p, idx * step60);
    let inner = select(0.73, 0.62, major);
    let width = select(0.011, 0.028, major);
    let mark = sd_box(tick - vec2<f32>(0.0, -(inner + 0.79) * 0.5), vec2<f32>(width, (0.79 - inner) * 0.5), 0.0);
    acc = over(acc, vec3<f32>(0.08), fill(mark, aa) * 0.9);

    // Subesfera de los minutos.
    let sub_center = vec2<f32>(0.0, -0.36);
    let sp = p - sub_center;
    acc = over(acc, vec3<f32>(0.1), fill(abs(length(sp) - 0.2) - 0.008, aa) * 0.8);
    let sub_dir = to_local(vec2<f32>(0.0, -1.0), -params.y * 6.2831853);
    acc = over(acc, vec3<f32>(0.08), fill(sd_segment(sp, vec2<f32>(0.0), sub_dir * 0.16) - 0.012, aa));
    acc = over(acc, vec3<f32>(0.08), fill(length(sp) - 0.03, aa));

    // Aguja de los segundos (con su sombra), contrapeso y tapa central.
    let dir = to_local(vec2<f32>(0.0, -1.0), -params.x * 6.2831853);
    let shadow_hand = sd_segment(p - vec2<f32>(0.035, 0.05), -dir * 0.2, dir * 0.78) - 0.02;
    acc = over(acc, vec3<f32>(0.0), (1.0 - smoothstep(-0.02, 0.06, shadow_hand)) * 0.3 * (1.0 - smoothstep(0.84, 0.86, r)));
    let red = vec3<f32>(0.85, 0.08, 0.06) * (1.0 + alarm * 0.8);
    let hand = sd_segment(p, -dir * 0.2, dir * 0.78) - mix(0.024, 0.012, clamp(dot(p, dir) / 0.78, 0.0, 1.0));
    acc = over(acc, red, fill(hand, aa));
    acc = over(acc, red, fill(length(p + dir * 0.17) - 0.05, aa));
    acc = over(acc, red, fill(r - 0.075, aa));
    acc = over(acc, vec3<f32>(0.25, 0.02, 0.02), fill(r - 0.028, aa));

    // Cristal: reflejo suave arriba a la izquierda y un filo de luz junto al bisel.
    let gloss = (1.0 - smoothstep(0.0, 0.5, length((p - vec2<f32>(-0.28, -0.38)) * vec2<f32>(1.0, 1.6))));
    let rim = smoothstep(0.7, 0.8, r) * (1.0 - smoothstep(0.8, 0.84, r)) * max(dot(n, light), 0.0);
    acc = over(acc, vec3<f32>(1.0), (gloss * 0.16 + rim * 0.45) * fill(r - 0.84, aa));

    return vec4<f32>(acc.rgb / max(acc.a, 1e-4), acc.a * params.w);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, samp, in.uv);
    // Tamaño de un píxel en las unidades del cronómetro (las derivadas, fuera del `switch`).
    let duv = fwidth(in.uv);
    let watch_aa = max(duv.x * 2.0, duv.y * 2.4) * 1.2;
    // Lo que cambia la distancia del texto de un píxel físico al siguiente.
    let text_aa = length(vec2<f32>(dpdx(texel.a), dpdy(texel.a)));
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
        case WATCH: {
            return stopwatch(in.uv, watch_aa, in.color);
        }
        case TEXT: {
            let dilate = f32((in.mode >> 16u) & 0xffu) / 510.0;
            let soft = f32(in.mode >> 24u) / 510.0;
            let edge = 0.5 - dilate;
            // Rampa de un píxel físico centrada en el borde (más ancha si se difumina).
            let a = clamp((texel.a - edge) / (max(text_aa, 1e-4) + soft) + 0.5, 0.0, 1.0);
            return vec4<f32>(in.color.rgb, in.color.a * a);
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
