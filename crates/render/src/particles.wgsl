// Partículas: la simulación corre entera en la GPU (`sim`, compute) y se dibujan como quads
// instanciados (`vs`/`fs`) leyendo el mismo búfer como vértices por instancia.
//
// El búfer es un anillo: la CPU solo manda "emisores" (ráfagas) que reservan un tramo
// [first, first + count) del anillo; cada hilo del compute mira si su partícula cae en algún
// tramo y, si es así, la reinicia con números aleatorios propios.

const SPARK: u32 = 0u;   // chispa: aditiva, brilla (alimenta el bloom), se estira con la velocidad
const DEBRIS: u32 = 1u;  // trozo: cuadrado opaco con rozamiento (cajas, sangre)
const SMOKE: u32 = 2u;   // humo: disco suave que crece y sube
const CONFETTI: u32 = 3u; // confeti (modo niños): trozo de color al azar
const GLOW: u32 = 4u;     // destello aditivo del color pedido (estelas)

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    color: vec4<f32>,
    life: f32,
    max_life: f32,
    size: f32,
    kind: u32,
};

struct Emitter {
    color: vec4<f32>,
    pos: vec2<f32>,
    // Semiextensión del rectángulo donde nacen.
    area: vec2<f32>,
    // Velocidad (px/s) mínima y máxima, en dirección aleatoria dentro de `spread`.
    speed: vec2<f32>,
    life: vec2<f32>,
    // Velocidad que se suma a todas (p. ej. a lo largo del brazo de una explosión).
    drift: vec2<f32>,
    size: f32,
    kind: u32,
    first: u32,
    count: u32,
    seed: u32,
    _pad: u32,
};

struct Params {
    dt: f32,
    emitters: u32,
    count: u32,
    _pad: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read> emitters: array<Emitter>;

// PCG hash: un número pseudoaleatorio por (partícula, emisor, componente).
fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand(seed: ptr<function, u32>) -> f32 {
    *seed = pcg(*seed);
    return f32(*seed) / 4294967295.0;
}

fn hue(h: f32) -> vec3<f32> {
    let k = vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0);
    return clamp(abs(fract(h + k) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn spawn(e: Emitter, k: u32) -> Particle {
    var s = pcg(e.seed ^ (k * 2654435761u));
    let angle = rand(&s) * 6.2831853;
    let speed = mix(e.speed.x, e.speed.y, rand(&s));
    let offset = (vec2<f32>(rand(&s), rand(&s)) * 2.0 - 1.0) * e.area;
    let life = mix(e.life.x, e.life.y, rand(&s));
    var color = e.color;
    // Un poco de variación de brillo para que no salgan todas iguales.
    color = vec4<f32>(color.rgb * mix(0.7, 1.15, rand(&s)), color.a);
    if e.kind == CONFETTI {
        color = vec4<f32>(hue(rand(&s)), 1.0);
    }
    var p: Particle;
    p.pos = e.pos + offset;
    p.vel = vec2<f32>(cos(angle), sin(angle)) * speed + e.drift;
    p.color = color;
    p.life = life;
    p.max_life = life;
    p.size = e.size * mix(0.6, 1.3, rand(&s));
    p.kind = e.kind;
    return p;
}

@compute @workgroup_size(64)
fn sim(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.count {
        return;
    }
    for (var n = 0u; n < params.emitters; n++) {
        let e = emitters[n];
        let k = (i + params.count - e.first) % params.count;
        if k < e.count {
            particles[i] = spawn(e, k);
            return;
        }
    }
    var p = particles[i];
    if p.life <= 0.0 {
        return;
    }
    let dt = params.dt;
    switch p.kind {
        case SPARK, GLOW: {
            p.vel *= exp(-3.5 * dt);
        }
        case SMOKE: {
            p.vel = p.vel * exp(-2.0 * dt) + vec2<f32>(0.0, -18.0 * dt);
        }
        default: {
            p.vel *= exp(-6.0 * dt);
        }
    }
    p.pos += p.vel * dt;
    p.life -= dt;
    particles[i] = p;
}

struct Globals { screen: vec2<f32>, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // Coordenadas locales del quad en [-1, 1].
    @location(0) local: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) kind: u32,
};

@vertex
fn vs(
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2<f32>,
    @location(1) vel: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) life: vec3<f32>,
    @location(4) kind: u32,
) -> VsOut {
    var out: VsOut;
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u)) * 2.0 - 1.0;
    out.local = corner;
    out.kind = kind;
    if life.x <= 0.0 {
        // Partícula muerta: triángulo degenerado fuera de la pantalla.
        out.pos = vec4<f32>(2.0, 2.0, 0.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }
    let t = clamp(life.x / life.y, 0.0, 1.0);
    var size = life.z;
    var axis = vec2<f32>(1.0, 0.0);
    var stretch = 1.0;
    // Salida premultiplicada: (rgb * a, a). Con a = 0 la mezcla es puramente aditiva.
    switch kind {
        case SPARK: {
            size *= 0.4 + 0.6 * t;
            let speed = length(vel);
            if speed > 1.0 {
                axis = vel / speed;
                stretch = 1.0 + speed * 0.012;
            }
            // Del blanco amarillento al rojo según se apaga.
            let c = color.rgb * mix(vec3<f32>(1.0, 0.35, 0.15), vec3<f32>(1.0), t * t);
            out.color = vec4<f32>(c * t, 0.0);
        }
        case GLOW: {
            size *= 0.5 + 0.5 * t;
            out.color = vec4<f32>(color.rgb * t, 0.0);
        }
        case SMOKE: {
            size *= 1.0 + 2.0 * (1.0 - t);
            let a = color.a * t * min(1.0, (1.0 - t) * 6.0);
            out.color = vec4<f32>(color.rgb * a, a);
        }
        default: {
            let a = color.a * min(1.0, t * 3.0);
            out.color = vec4<f32>(color.rgb * a, a);
        }
    }
    let perp = vec2<f32>(-axis.y, axis.x);
    let p = pos + axis * corner.x * size * stretch + perp * corner.y * size;
    let ndc = vec2<f32>(p.x / g.screen.x * 2.0 - 1.0, 1.0 - p.y / g.screen.y * 2.0);
    out.pos = vec4<f32>(ndc, 0.0, 1.0);
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    if in.kind == DEBRIS || in.kind == CONFETTI {
        return in.color;
    }
    let r = length(in.local);
    let mask = 1.0 - smoothstep(0.2, 1.0, r);
    return in.color * mask;
}
