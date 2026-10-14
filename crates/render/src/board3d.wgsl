// Cubos con textura en perspectiva (`board3d.rs`): luz ambiente, una direccional suave y luces
// puntuales cálidas (antorchas, mechas), sin sombras.

const MAX_LIGHTS: u32 = 8u;

struct Scene {
    view_proj: mat4x4<f32>,
    ambient: vec4<f32>,
    // Hacia dónde apunta la luz direccional (normalizada) y su color.
    sun_dir: vec4<f32>,
    sun: vec4<f32>,
    // x = luces en uso.
    info: vec4<f32>,
    // xyz y radio.
    light_pos: array<vec4<f32>, 8>,
    light_color: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> scene: Scene;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) world: vec3<f32>,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
) -> VsOut {
    var o: VsOut;
    o.clip = scene.view_proj * vec4<f32>(pos, 1.0);
    o.uv = uv;
    o.normal = normal;
    o.color = color;
    o.world = pos;
    return o;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(tex, samp, in.uv);
    if (texel.a < 0.5) {
        discard;
    }
    let n = normalize(in.normal);
    var light = scene.ambient.rgb + scene.sun.rgb * max(dot(n, -scene.sun_dir.xyz), 0.0);
    let count = min(u32(scene.info.x), MAX_LIGHTS);
    for (var i = 0u; i < count; i++) {
        let lp = scene.light_pos[i];
        let d = lp.xyz - in.world;
        let dist = length(d);
        let fall = clamp(1.0 - dist / lp.w, 0.0, 1.0);
        // Algo de luz aunque la cara no mire a la luz: las caras laterales no quedan negras.
        let facing = max(dot(n, d / max(dist, 1e-4)), 0.0) * 0.8 + 0.2;
        light += scene.light_color[i].rgb * fall * fall * facing;
    }
    return vec4<f32>(texel.rgb * in.color.rgb * light, 1.0);
}
