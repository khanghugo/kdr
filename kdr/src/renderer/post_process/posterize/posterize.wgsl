struct ColorData {
    @align(16) @size(12)
    color_count: u32,
    colors: array<vec4f, 64>
}

@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(0) @binding(2) var<uniform> color_data: ColorData;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = textureDimensions(input_tex);
    let uv = frag_coord.xy / vec2f(tex_size);

    let original_color = textureSample(input_tex, tex_sampler, uv).rgb;
    let grayscale = dot(original_color, vec3(0.299, 0.587, 0.114));

    let color_index = u32(floor(grayscale * f32(color_data.color_count)));
    let posterize_color = color_data.colors[color_index].rgb;

    return vec4(posterize_color, 1.0);
}
