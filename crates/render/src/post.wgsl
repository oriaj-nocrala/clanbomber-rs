// Composición final: escena + bloom, con filtro CRT opcional y aberración cromática mientras
// tiembla la pantalla. Un triángulo que cubre el viewport (el letterbox lo pone el viewport).

struct Post {
    bloom: f32,
    crt: f32,
    aberration: f32,
    time: f32,
};

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var linear: sampler;
@group(0) @binding(4) var<uniform> post: Post;

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
    var color = sample_scene(uv) + textureSample(bloom_tex, linear, uv).rgb * post.bloom;
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
