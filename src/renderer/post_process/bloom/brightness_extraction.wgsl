@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

// vibe coding
@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = textureDimensions(input_tex);
    let uv = frag_coord.xy / vec2f(tex_size);

    let color = textureSample(input_tex, tex_sampler, uv.xy);

    // color is already linear as specified in the shader
    // let linear_color = pow(color.rgb, vec3(2.2));

    let luminance = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));

    return select(vec4(0.0), color, luminance > 0.8);
}
