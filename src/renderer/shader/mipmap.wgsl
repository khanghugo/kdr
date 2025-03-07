struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// https://github.com/gfx-rs/wgpu/blob/trunk/examples/features/src/mipmap/blit.wgsl
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;

    let x = f32(vertex_index >> 1);
    let y = f32(vertex_index & 1);

    let xy = vec2<f32>(
        x, y
    ) * 2.0;

    result.position = vec4<f32>(
        xy.x * 2.0 - 1.0,
        1.0 - xy.y * 2.0,
        0.0, 1.0
    );

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