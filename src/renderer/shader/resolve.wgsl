@group(0) @binding(0) var accum_tex: texture_2d<f32>;
@group(0) @binding(1) var reveal_tex: texture_2d<f32>;
@group(0) @binding(2) var resolve_sampler: sampler;

@fragment
fn resolve_fs(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let accum_size = textureDimensions(accum_tex);
    let uv = frag_coord.xy / vec2f(accum_size);

    let accum = textureSample(accum_tex, resolve_sampler, uv);
    let reveal = textureSample(reveal_tex, resolve_sampler, uv).r;

    // color is premultiplied so we divide it here
    let average_color = accum.rgb / max(accum.a, 1e-5);
    let coverage = 1.0 - reveal;

    var final_color = vec4f(average_color, coverage);

    return final_color;
}
