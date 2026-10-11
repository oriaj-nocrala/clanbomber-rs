// Bloom por compute: se queda con lo que brilla de la escena a media resolución virtual
// (`prefilter`) y lo emborrona con un gaussiano separable (`blur`, horizontal o vertical).

struct Blur { dir: vec2<i32>, _pad: vec2<i32> };

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var dst: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> blur_dir: Blur;

const THRESHOLD: f32 = 0.8;
const KNEE: f32 = 0.25;

@compute @workgroup_size(8, 8)
fn prefilter(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(dst);
    if id.x >= size.x || id.y >= size.y {
        return;
    }
    // Cuatro muestras bilineales: cubre 4x4 píxeles de la escena si es el doble de grande.
    let texel = 1.0 / vec2<f32>(size);
    let uv = (vec2<f32>(id.xy) + 0.5) * texel;
    let o = texel * 0.25;
    var c = textureSampleLevel(src, samp, uv + vec2<f32>(-o.x, -o.y), 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(o.x, -o.y), 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(-o.x, o.y), 0.0).rgb;
    c += textureSampleLevel(src, samp, uv + vec2<f32>(o.x, o.y), 0.0).rgb;
    c *= 0.25;
    // Un NaN o infinito en la escena se extendería con el desenfoque en un cuadrado negro: fuera.
    // (Por bits: WGSL deja suponer que no hay NaN, así que `c != c` podría desaparecer.)
    let exponent = bitcast<vec3<u32>>(c) & vec3<u32>(0x7f800000u);
    c = select(c, vec3<f32>(0.0), exponent == vec3<u32>(0x7f800000u));
    // Umbral con rodilla suave (sin cortes bruscos entre lo que brilla y lo que no).
    let bright = max(c.r, max(c.g, c.b));
    var soft = clamp(bright - THRESHOLD + KNEE, 0.0, 2.0 * KNEE);
    soft = soft * soft / (4.0 * KNEE + 1e-4);
    let w = max(soft, bright - THRESHOLD) / max(bright, 1e-4);
    textureStore(dst, vec2<i32>(id.xy), vec4<f32>(c * w, 1.0));
}

@compute @workgroup_size(8, 8)
fn blur(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = vec2<i32>(textureDimensions(dst));
    let p = vec2<i32>(id.xy);
    if p.x >= size.x || p.y >= size.y {
        return;
    }
    // Gaussiano de 9 muestras (sigma ~ 2).
    let weights = array<f32, 5>(0.2270, 0.1945, 0.1216, 0.0540, 0.0162);
    var c = textureLoad(src, p, 0).rgb * weights[0];
    for (var i = 1; i < 5; i++) {
        let d = blur_dir.dir * i;
        c += textureLoad(src, clamp(p + d, vec2<i32>(0), size - 1), 0).rgb * weights[i];
        c += textureLoad(src, clamp(p - d, vec2<i32>(0), size - 1), 0).rgb * weights[i];
    }
    textureStore(dst, p, vec4<f32>(c, 1.0));
}
