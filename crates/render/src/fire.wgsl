// Explosión procedural: una cruz de fuego cuyo grosor depende de la potencia de la bomba.
// Idea tomada del shader de `clanbomber_sdl3` (cruz por SDF + turbulencia fbm + paleta de fuego),
// pero dibujada en la rejilla de píxeles virtuales y con borde granulado, como el sprite original.

struct Globals { screen: vec2<f32>, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    // Posición en píxeles virtuales.
    @location(0) world: vec2<f32>,
    @location(1) @interpolate(flat) center: vec2<f32>,
    // Alcance de cada brazo en píxeles desde el centro: izquierda, derecha, arriba, abajo.
    @location(2) @interpolate(flat) arms: vec4<f32>,
    // potencia, progreso (0..1), semilla, tamaño de casilla.
    @location(3) @interpolate(flat) params: vec4<f32>,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) rect: vec4<f32>,
    @location(1) center: vec2<f32>,
    @location(2) arms: vec4<f32>,
    @location(3) params: vec4<f32>,
) -> VsOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let p = rect.xy + corner * rect.zw;
    let ndc = vec2<f32>(p.x / g.screen.x * 2.0 - 1.0, 1.0 - p.y / g.screen.y * 2.0);
    return VsOut(vec4<f32>(ndc, 0.0, 1.0), p, center, arms, params);
}

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn hash2(p: vec2<f32>, seed: f32) -> f32 {
    let q = vec2<i32>(floor(p));
    return f32(pcg(u32(q.x) ^ pcg(u32(q.y) ^ pcg(u32(seed))))) / 4294967295.0;
}

fn noise(p: vec2<f32>, seed: f32) -> f32 {
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    let a = hash2(i, seed);
    let b = hash2(i + vec2<f32>(1.0, 0.0), seed);
    let c = hash2(i + vec2<f32>(0.0, 1.0), seed);
    let d = hash2(i + vec2<f32>(1.0, 1.0), seed);
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn fbm(p0: vec2<f32>, seed: f32) -> f32 {
    var p = p0;
    var v = 0.0;
    var a = 0.5;
    for (var i = 0; i < 3; i++) {
        v += a * noise(p, seed);
        p *= 2.03;
        a *= 0.5;
    }
    return v / 0.875;
}

// Del rojo oscuro del borde al blanco que brilla (> 1: lo recoge el bloom) del centro.
fn palette(heat: f32) -> vec3<f32> {
    let red = vec3<f32>(0.85, 0.16, 0.02);
    let orange = vec3<f32>(1.0, 0.45, 0.04);
    let yellow = vec3<f32>(1.0, 0.85, 0.25);
    let white = vec3<f32>(1.9, 1.75, 1.3);
    if heat < 0.3 {
        return mix(red, orange, heat / 0.3);
    }
    if heat < 0.75 {
        return mix(orange, yellow, (heat - 0.3) / 0.45);
    }
    return mix(yellow, white, smoothstep(0.82, 1.0, heat));
}

// Brazo recto a lo largo de x (positivo y negativo con alcances distintos), con punta redonda
// y estrechándose hacia la punta.
fn arm(d: vec2<f32>, reach_neg: f32, reach_pos: f32, width: f32) -> f32 {
    let reach = select(reach_neg, reach_pos, d.x > 0.0);
    let along = clamp(abs(d.x) / max(reach, 1.0), 0.0, 1.0);
    let w = width * (1.0 - 0.3 * along * along);
    return length(vec2<f32>(max(abs(d.x) - (reach - w), 0.0), d.y)) - w;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let power = in.params.x;
    let t = in.params.y;
    let seed = in.params.z;
    let tile = in.params.w;
    // Un píxel virtual por fragmento lógico: se ve tan "gordo" como los sprites.
    let q = floor(in.world) + 0.5;
    let d = q - in.center;

    // Los brazos salen disparados del centro en el primer quinto de la explosión.
    let grow = smoothstep(0.0, 0.2, t);
    let half = tile * 0.5;
    let reach = mix(vec4<f32>(half * 0.6), in.arms, grow);

    // Grosor: fino con potencia 1 y ancho con potencia alta. Como el sprite (Explosion::show:
    // fino, medio, grueso, medio, fino), engorda hasta la mitad y luego adelgaza.
    let pw = clamp((power - 1.0) / 9.0, 0.0, 1.0);
    let base = mix(5.0, 13.0, sqrt(pw));
    let envelope = 0.4 + 0.6 * sin(3.14159 * clamp(t * 1.05, 0.0, 1.0));
    let width = base * envelope;

    var sd = min(
        arm(d, reach.x, reach.y, width),
        arm(d.yx, reach.z, reach.w, width),
    );
    // Bola central algo más gorda que los brazos.
    sd = min(sd, length(d) - width * 1.2);

    // Turbulencia que fluye hacia fuera del centro y grano por píxel en el borde (como el
    // punteado del sprite). El grano cambia a ~30 fps.
    let outward = normalize(d + vec2<f32>(1e-3));
    let flow = fbm(q * 0.12 - outward * t * 6.0, seed);
    let frame = floor(t * 15.0);
    let grain = hash2(q, seed + frame * 7.0);
    sd += (flow - 0.5) * width * 0.9 + (grain - 0.5) * 3.0;
    if sd > 0.0 {
        return vec4<f32>(0.0);
    }

    // Calor: máximo en el eje, con un fogonazo blanco al principio y enfriándose al final.
    var heat = clamp(-sd / max(width * 0.9, 1.0), 0.0, 1.0);
    heat = clamp(heat * 0.9 + 0.15 * (1.0 - smoothstep(0.0, 0.2, t)) - 0.25 * smoothstep(0.6, 1.0, t)
        + (flow - 0.5) * 0.3, 0.0, 1.0);
    // Bandas de color: paleta reducida, más cerca del pixel art.
    heat = floor(heat * 6.0 + 0.5) / 6.0;
    return vec4<f32>(palette(heat), 1.0);
}
