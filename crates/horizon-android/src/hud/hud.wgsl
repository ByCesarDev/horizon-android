// Horizon Android — HUD 2D shader
// Renders textured quads (touch buttons, crosshair, hotbar) with alpha blending.
// Coordinates: NDC space (-1..1 x, +1..-1 y, i.e. origin top-left after transform).

struct VertexInput {
    @location(0) position: vec2<f32>,  // NDC position (-1..1, -1..1)
    @location(1) uv: vec2<f32>,        // texture UV (0..1)
    @location(2) alpha: f32,           // per-vertex alpha multiplier
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv;
    out.alpha = in.alpha;
    return out;
}

@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(t_diffuse, s_diffuse, in.uv);
    color.a *= in.alpha;
    return color;
}

