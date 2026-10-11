// Composición final: escena + bloom, con filtro CRT opcional, aberración cromática mientras
// tiembla la pantalla, ondas expansivas y aire caliente. Un triángulo que cubre el viewport (el letterbox lo pone el viewport).

struct Post {
    bloom: f32,
    crt: f32,
    aberration: f32,
    time: f32,
    waves: u32,
    // Cámara: zoom (1 = sin zoom) alrededor de `focus` (px virtuales).
    zoom: f32,
    focus: vec2<f32>,
    // Cámara lenta: 0..1 de desaturación y viñeta.
    drama: f32,
    // 1 si la máscara de calor tiene algo este fotograma.
    heat: f32,
    _pad1: f32,
    _pad2: f32,
    // Focos de la cámara lenta: centro (px virtuales), radio y fuerza (0 = ninguno).
    spot: array<vec4<f32>, 3>,
    // Por onda: centro (px virtuales), progreso 0..1 y fuerza.
    wave: array<vec4<f32>, 16>,
};

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var linear: sampler;
@group(0) @binding(4) var<uniform> post: Post;
// Máscara de calor a media resolución virtual (`heat.wgsl`).
@group(0) @binding(5) var heat_tex: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    return VsOut(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}

fn sample_scene(uv: vec2<f32>) -> vec3<f32> {
    if post.zoom > 1.001 {
        // Con zoom no entero, el bilineal evita píxeles de ancho desigual.
        return textureSample(scene, linear, uv).rgb;
    }
    if post.aberration <= 0.0 {
        return textureSample(scene, nearest, uv).rgb;
    }
    // Los canales rojo y azul se separan en horizontal.
    let o = vec2<f32>(post.aberration, 0.0);
    return vec3<f32>(
        textureSample(scene, nearest, uv + o).r,
        textureSample(scene, nearest, uv).g,
        textureSample(scene, nearest, uv - o).b,
    );
}

// Ondas expansivas: un anillo que crece desde cada explosión, empuja la imagen hacia fuera y
// aclara un poco su frente. Devuelve el desplazamiento en uv (xy) y el brillo extra (z).
fn shockwaves(uv: vec2<f32>) -> vec3<f32> {
    let virt = vec2<f32>(800.0, 600.0);
    let p = uv * virt;
    var offset = vec2<f32>(0.0);
    var glow = 0.0;
    for (var i = 0u; i < post.waves; i++) {
        let w = post.wave[i];
        let d = p - w.xy;
        let dist = length(d);
        let radius = mix(8.0, 80.0 + 50.0 * w.w, sqrt(w.z));
        let ring = (dist - radius) / 20.0;
        // Perfil: seno dentro de una campana, positivo por fuera y negativo por dentro.
        let bell = exp(-ring * ring * 1.5);
        let fade = 1.0 - w.z;
        offset += d / max(dist, 1.0) * sin(ring * 3.14159) * bell * 8.0 * w.w * fade;
        glow += bell * 0.12 * w.w * fade;
    }
    return vec3<f32>(offset / virt, glow);
}

fn hash21(p: vec2<i32>) -> f32 {
    let q = vec2<u32>(p + 32768);
    var h = q.x * 374761393u + q.y * 668265263u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h ^ (h >> 16u)) / 4294967295.0;
}

// Ruido de valor con interpolación quíntica (sin aristas en el desplazamiento).
fn vnoise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash21(i);
    let b = hash21(i + vec2<i32>(1, 0));
    let c = hash21(i + vec2<i32>(0, 1));
    let d = hash21(i + vec2<i32>(1, 1));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Aire caliente en `p` (px virtuales): desplazamiento (xy, en unidades de ±0.5) y un valor para
// el rielado del brillo (z). Bandas alargadas en horizontal que suben, deformadas por un
// torbellino lento para que no se vean como un dibujo que se desliza.
fn haze(p: vec2<f32>, t: f32) -> vec3<f32> {
    let swirl = vnoise(vec2<f32>(p.x / 34.0, p.y / 22.0 + t * 0.6)) * 2.2;
    let q1 = vec2<f32>(p.x / 15.0 + swirl, p.y / 6.0 + t * 2.3);
    let q2 = vec2<f32>(p.x / 6.5, p.y / 2.8 + t * 4.4 + swirl);
    let a = vnoise(q1) * 0.7 + vnoise(q2) * 0.3;
    let b = vnoise(q1 + vec2<f32>(7.3, 3.1)) * 0.7 + vnoise(q2 + vec2<f32>(11.9, 5.7)) * 0.3;
    return vec3<f32>(a - 0.5, (b - 0.5) * 0.45, a);
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    var uv = in.uv;
    if post.crt > 0.0 {
        // Curvatura de tubo: los bordes se doblan hacia fuera.
        let c = uv * 2.0 - 1.0;
        uv = (c * (1.0 + 0.035 * dot(c, c) * vec2<f32>(0.75, 1.0))) * 0.5 + 0.5;
    }
    // Fuera del tubo, negro (sin `return` temprano: textureSample exige control uniforme).
    let inside = f32(all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)));
    // Cámara: acercarse a `focus` deja ese punto quieto en pantalla.
    let focus = post.focus / vec2<f32>(800.0, 600.0);
    var suv = focus + (uv - focus) / post.zoom;
    let wave = shockwaves(suv);
    suv -= wave.xy;
    var color = sample_scene(suv) + textureSample(bloom_tex, linear, suv).rgb * post.bloom;
    let heat = textureSampleLevel(heat_tex, linear, suv, 0.0).r * post.heat;
    if heat > 0.003 {
        // Refracción: la imagen se mueve unos píxeles con el aire. Sin filtrar, para que el
        // pixel art se ondule nítido en vez de emborronarse. Cada canal se dobla un poco distinto
        // (el rojo más que el azul): donde más tiembla queda un filo de arcoíris.
        let virt = vec2<f32>(800.0, 600.0);
        let k = min(heat, 1.6);
        let hz = haze(suv * virt, post.time % 600.0);
        let o = hz.xy * 6.0 * k / virt;
        let hot = vec3<f32>(
            textureSampleLevel(scene, nearest, suv + o * 1.12, 0.0).r,
            textureSampleLevel(scene, nearest, suv + o, 0.0).g,
            textureSampleLevel(scene, nearest, suv + o * 0.88, 0.0).b,
        ) + textureSampleLevel(bloom_tex, linear, suv + o, 0.0).rgb * post.bloom;
        // La luz que atraviesa el aire caliente riela.
        let shimmer = 1.0 + (hz.z - 0.5) * 0.14 * k;
        color = mix(color, hot * shimmer, smoothstep(0.003, 0.12, heat));
    }
    color *= 1.0 + wave.z;
    if post.drama > 0.0 {
        // Cámara lenta: penumbra sin color salvo en los focos (quien cae, la bomba, quien la
        // puso); sin focos, una viñeta en los bordes.
        let p = suv * vec2<f32>(800.0, 600.0);
        var lit = 0.0;
        var any_spot = 0.0;
        for (var i = 0u; i < 3u; i++) {
            let s = post.spot[i];
            if s.w > 0.0 {
                any_spot = 1.0;
                let d = length(p - s.xy);
                lit = max(lit, s.w * (1.0 - smoothstep(s.z, s.z * 2.2, d)));
            }
        }
        let v = uv * (1.0 - uv);
        let vignette = pow(16.0 * v.x * v.y, 0.35);
        let light = mix(vignette, mix(0.42, 1.0, lit), any_spot);
        let lum = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
        color = mix(color, vec3<f32>(lum), 0.6 * post.drama * (1.0 - lit));
        color *= mix(1.0, light, post.drama);
    }
    if post.crt > 0.0 {
        // Líneas de barrido a la resolución del original (600 líneas) y viñeta.
        let line = 0.5 + 0.5 * cos(uv.y * 600.0 * 6.2831853);
        color *= mix(1.0, 0.72 + 0.28 * line, post.crt);
        // Máscara de apertura: realza un canal por columna de 3 píxeles virtuales.
        let col = u32(uv.x * 800.0 * 3.0) % 3u;
        var mask = vec3<f32>(0.9);
        mask[col] = 1.1;
        color *= mix(vec3<f32>(1.0), mask, post.crt);
        let v = uv * (1.0 - uv);
        color *= mix(1.0, pow(16.0 * v.x * v.y, 0.18), post.crt);
        color *= 1.0 + 0.12 * post.crt;
    }
    return vec4<f32>(color * inside, 1.0);
}
