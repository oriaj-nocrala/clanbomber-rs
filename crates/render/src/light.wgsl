// Iluminación dinámica: un quad sobre el mapa que multiplica lo ya dibujado por la luz que le
// llega a cada píxel (ambiente + luces puntuales). Muros y cajas tapan la luz: para cada luz se
// recorre el segmento píxel → luz sobre la rejilla de casillas (`occ`, 1 = opaco) y se atenúa.
// El resultado puede pasar de 1 (la escena es flotante): cerca de una explosión, aclara.

struct Light {
    // x, y (px virtuales), radio, intensidad.
    pos: vec4<f32>,
    color: vec4<f32>,
};

struct Lights {
    ambient: vec4<f32>,
    // Esquina del mapa (px), tamaño de casilla y cuántas luces hay.
    origin: vec2<f32>,
    cell: f32,
    count: u32,
    grid: vec2<f32>,
    _pad: vec2<f32>,
    lights: array<Light, 96>,
};

struct Globals { screen: vec2<f32>, time: f32, _pad: f32 };
@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var<uniform> L: Lights;
@group(0) @binding(2) var occ: texture_2d<f32>;
@group(0) @binding(3) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32) -> VsOut {
    // Quad que cubre el mapa.
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let p = L.origin + corner * L.grid * L.cell;
    let ndc = vec2<f32>(p.x / g.screen.x * 2.0 - 1.0, 1.0 - p.y / g.screen.y * 2.0);
    return VsOut(vec4<f32>(ndc, 0.0, 1.0), p);
}

fn tile_of(p: vec2<f32>) -> vec2<f32> {
    return floor((p - L.origin) / L.cell);
}

// Cuánta luz llega de `src_pos` a `dst_pos` (1 = nada en medio). La casilla del píxel y la de la luz no
// cuentan: así la cara de un muro que mira a la luz se ilumina.
fn visibility(src_pos: vec2<f32>, dst_pos: vec2<f32>) -> f32 {
    let dist = distance(src_pos, dst_pos);
    let steps = i32(clamp(dist / 7.0, 1.0, 28.0));
    let own = tile_of(dst_pos);
    let src = tile_of(src_pos);
    var vis = 1.0;
    for (var i = 1; i < steps; i++) {
        let p = mix(src_pos, dst_pos, f32(i) / f32(steps));
        let t = tile_of(p);
        if all(t == own) || all(t == src) {
            continue;
        }
        // Bilineal: bordes de sombra suaves.
        let uv = (p - L.origin) / (L.cell * L.grid);
        let o = textureSampleLevel(occ, samp, uv, 0.0).r;
        vis *= 1.0 - 0.85 * o;
        if vis < 0.02 {
            break;
        }
    }
    return vis;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    // Luz a la rejilla de píxeles virtuales, como el resto del pixel art.
    let p = floor(in.world) + 0.5;
    var light = L.ambient.rgb;
    for (var i = 0u; i < L.count; i++) {
        let l = L.lights[i];
        let d = distance(p, l.pos.xy);
        if d >= l.pos.z {
            continue;
        }
        let x = d / l.pos.z;
        let falloff = (1.0 - x * x) * (1.0 - x * x);
        light += l.color.rgb * l.pos.w * falloff * visibility(l.pos.xy, p);
    }
    // No quemar del todo lo que ya brilla.
    light = min(light, vec3<f32>(2.2));
    return vec4<f32>(light, 1.0);
}
