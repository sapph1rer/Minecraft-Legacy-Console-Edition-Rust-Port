struct Uniforms {
    view_proj: mat4x4<f32>,
    fog: vec4<f32>,    // rgb fog colour
    cam: vec4<f32>,    // xyz camera
    fog_p: vec4<f32>,  // start, end, density, mode (0 = linear, 1 = exp)
};
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;
@group(0) @binding(3) var lightmap: texture_2d<f32>;
@group(0) @binding(4) var light_samp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) col: vec4<f32>,
    @location(3) light: vec2<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
    @location(2) light: vec2<f32>,
    @location(3) depth: f32,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.col = v.col;
    o.light = v.light;
    o.depth = o.clip.w;
    return o;
}

// GL_FOG: linear or exp on the eye-space depth
fn fogged(rgb: vec3<f32>, d: f32) -> vec3<f32> {
    var f = 1.0;
    if (u.fog_p.w < 0.5) {
        f = clamp((u.fog_p.y - d) / max(u.fog_p.y - u.fog_p.x, 0.0001), 0.0, 1.0);
    } else {
        f = clamp(exp(-u.fog_p.z * d), 0.0, 1.0);
    }
    return mix(u.fog.rgb, rgb, f);
}

// light texture: column = block light, row = sky light (texel centres at (n + 0.5) / 16)
fn lit(l: vec2<f32>) -> vec3<f32> {
    let uv = (vec2<f32>(l.y, l.x) + vec2<f32>(0.5)) / 16.0;
    return textureSampleLevel(lightmap, light_samp, uv, 0.0).rgb;
}

@fragment
fn fs_opaque(i: VOut) -> @location(0) vec4<f32> {
    if (i.uv.x < 0.0) { return vec4<f32>(i.col.rgb, 1.0); }
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    if (c.a < 0.5) { discard; }
    return vec4<f32>(fogged(c.rgb * i.col.rgb * lit(i.light), i.depth), 1.0);
}

@fragment
fn fs_trans(i: VOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    let a = c.a * i.col.a;
    if (a < 0.02) { discard; }
    return vec4<f32>(fogged(c.rgb * i.col.rgb * lit(i.light), i.depth), a);
}

// particles: GL_ALPHA_TEST > 1/255
@fragment
fn fs_particle(i: VOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    let a = c.a * i.col.a;
    if (a <= 1.0 / 255.0) { discard; }
    return vec4<f32>(fogged(c.rgb * i.col.rgb * lit(i.light), i.depth), a);
}

// eye / armor glow layers: full brightness, GL_ONE + GL_ONE
@fragment
fn fs_glow(i: VOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    if (c.a <= 1.0 / 255.0) { discard; }
    return vec4<f32>(fogged(c.rgb * i.col.rgb, i.depth), 1.0);
}

// destroy animation: glColor4f(1, 1, 1, 0.5) with the alpha test, blended as (DST_COLOR, SRC_COLOR)
@fragment
fn fs_crack(i: VOut) -> @location(0) vec4<f32> {
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    if (c.a * 0.5 <= 0.1) { discard; }
    return vec4<f32>(c.rgb, c.a * 0.5);
}
