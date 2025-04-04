@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

// vibe coding
@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = vec2f(textureDimensions(input_tex));
    let uv = frag_coord.xy / tex_size;
    let texel_size = 1.0 / tex_size;

    let iterations = 3;      // More iterations = stronger blur (try 2-4)
    let offset_scale = 0.5;  // Larger = more spread (try 0.5-1.5)

    var result = vec4f(0.0);

    for (var i = 0; i < iterations; i++) {
        let offset = (f32(i) + offset_scale) * texel_size;
        result += textureSample(input_tex, tex_sampler, uv + vec2( offset.x,  offset.y));
        result += textureSample(input_tex, tex_sampler, uv + vec2(-offset.x,  offset.y));
        result += textureSample(input_tex, tex_sampler, uv + vec2( offset.x, -offset.y));
        result += textureSample(input_tex, tex_sampler, uv + vec2(-offset.x, -offset.y));
    }
    return result / (f32(iterations) * 4.0);
}
