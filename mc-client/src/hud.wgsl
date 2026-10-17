@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;

struct VIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) col: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
};

@vertex
fn vs(v: VIn) -> VOut {
    var o: VOut;
    o.clip = vec4<f32>(v.pos, 0.0, 1.0);
    o.uv = v.uv;
    o.col = v.col;
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    if (i.uv.x < 0.0) {
        return i.col;
    }
    let c = textureSampleLevel(atlas, atlas_samp, i.uv, 0.0);
    if (c.a < 0.01) { discard; }
    return vec4<f32>(c.rgb * i.col.rgb, c.a * i.col.a);
}
