// vibe coding
// Fullscreen triangle vertex shader
@vertex
fn resolve_vs(@builtin(vertex_index) vert_idx: u32) -> @builtin(position) vec4f {
    // Generate fullscreen triangle without any vertex buffers
    let pos = array(
        vec2f(-1.0, -1.0),
        vec2f(3.0, -1.0),
        vec2f(-1.0, 3.0)
    );

    return vec4f(pos[vert_idx], 0.0, 1.0);
}

// Fragment shader
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
