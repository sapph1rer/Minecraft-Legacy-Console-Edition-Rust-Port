struct SkyU {
    vp: mat4x4<f32>,
    fog: vec4<f32>,        // rgb fog colour
    fog_sky: vec4<f32>,    // start, end
    fog_world: vec4<f32>,  // start, end, density, mode
    tint: vec4<f32>,       // multiplied into every colour
};
@group(0) @binding(0) var<uniform> u: SkyU;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

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
    @location(2) depth: f32,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = u.vp * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.col = v.col;
    o.depth = o.clip.w;
    return o;
}

fn lin_fog(rgb: vec3<f32>, d: f32, s: f32, e: f32) -> vec3<f32> {
    let f = clamp((e - d) / max(e - s, 0.0001), 0.0, 1.0);
    return mix(u.fog.rgb, rgb, f);
}

// dome: flat colour fogged toward the horizon colour
@fragment
fn fs_dome(i: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(lin_fog(u.tint.rgb, i.depth, u.fog_sky.x, u.fog_sky.y), 1.0);
}

// textured or flat (uv.x < 0) with the vertex colour, no fog (sun, moon, stars, sunrise fan, end sky)
@fragment
fn fs_plain(i: VOut) -> @location(0) vec4<f32> {
    if (i.uv.x < -0.5) { return i.col * u.tint; }
    let c = textureSample(tex, samp, i.uv);
    return c * i.col * u.tint;
}

// clouds: flat colour, world fog
@fragment
fn fs_cloud(i: VOut) -> @location(0) vec4<f32> {
    var rgb = i.col.rgb;
    let d = i.depth;
    let w = u.fog_world;
    var f = 1.0;
    if (w.w < 0.5) {
        f = clamp((w.y - d) / max(w.y - w.x, 0.0001), 0.0, 1.0);
    } else {
        f = clamp(exp(-w.z * d), 0.0, 1.0);
    }
    rgb = mix(u.fog.rgb, rgb, f);
    return vec4<f32>(rgb, i.col.a);
}

@fragment
fn fs_depth_only(i: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
