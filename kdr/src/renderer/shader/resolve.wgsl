@group(0) @binding(0) var accum_tex: texture_2d<f32>;
@group(0) @binding(1) var reveal_tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

// gemini 2.5 is pretty fucking good i am not sure whatever the fuck these means
@fragment
fn resolve_fs(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let accum_size = textureDimensions(accum_tex);
    let uv = frag_coord.xy / vec2f(accum_size);

    // Sample Accumulation buffer: (sum(rgb*a*w), sum(a*w))
    let accum = textureSample(accum_tex, tex_sampler, uv);
    // Sample Revealage buffer: sum(log(1 - a))
    let reveal_log_sum = textureSample(reveal_tex, tex_sampler, uv).r;

    // Reconstruct revealage product: product(1 - a) = exp(sum(log(1 - a)))
    // Clamp the input to exp to prevent potential issues with large negative numbers if many fully opaque fragments overlap. exp(-20) is already tiny.
    let reveal = exp(clamp(reveal_log_sum, -20.0, 0.0)); // reveal_log_sum should always be <= 0

    // Avoid division by zero for total weight (accum.a = sum(a*w))
    let epsilon = 1e-5;
    let total_weight = max(accum.a, epsilon);

    // Calculate average color (un-premultiply)
    let average_color = accum.rgb / total_weight;

    // Calculate final coverage/alpha: 1.0 - product(1 - a)
    let coverage = 1.0 - reveal;

    // Clamp average color in case weights caused it to exceed 1.0
    let clamped_average_color = clamp(average_color, vec3f(0.0), vec3f(1.0));

    // Output final color and coverage (pre-multiplied alpha style is common)
    // let final_premultiplied_color = clamped_average_color * coverage;
    // return vec4f(final_premultiplied_color, coverage);

    // Or return average color and coverage separately as in your original code
    // This might be useful if you composite later.
    return vec4f(clamped_average_color, coverage);
}
