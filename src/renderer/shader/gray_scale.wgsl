@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = textureDimensions(input_tex);
    let uv = frag_coord.xy / vec2f(tex_size);

    let color = textureSample(input_tex, tex_sampler, uv).rgb;
    let grayscale = dot(color, vec3(0.299, 0.587, 0.114));

    return vec4(vec3(grayscale), 1.0);
}
