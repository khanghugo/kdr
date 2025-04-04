@group(0) @binding(0) var texture: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = textureDimensions(texture);
    let uv = frag_coord.xy / vec2f(tex_size);

    let color = textureSample(texture, linear_sampler, uv);

    return color;
}
