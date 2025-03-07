struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// ~~https://github.com/gfx-rs/wgpu/blob/trunk/examples/features/src/mipmap/blit.wgsl~~
// https://maierfelix.github.io/2020-01-13-webgpu-ray-tracing/
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;

    let x = f32((vertex_index << 1) & 2);
    let y = f32(vertex_index & 2);

    let xy = vec2<f32>(x, y);

    result.position = vec4(xy * 2.0 - 1.0, 0.0, 1.0);
    result.uv = xy;

    return result;
}

@group(0) @binding(0)
var input_texture: texture_2d_array<f32>;
@group(0) @binding(1)
var input_sampler: sampler;

struct LayerUniform {
    layer: u32,
};

@group(0) @binding(2)
var<uniform> layer_uniform: LayerUniform;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(input_texture, input_sampler, input.uv, layer_uniform.layer);
}