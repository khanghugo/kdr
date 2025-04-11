@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

const RED_OFFSET = 0.0015;
const GREEN_OFFSET = 0.001;
const BLUE_OFFSET = -0.001;

const START_SCALE = 0.85; // starts around the edge
const END_SCALE = 0.95; // starts gradually until full effect

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = textureDimensions(input_tex);
    let uv = frag_coord.xy / vec2f(tex_size);

    let original_color = textureSample(input_tex, tex_sampler, uv);
    let r = textureSample(input_tex, tex_sampler, uv + vec2(RED_OFFSET)).r;
    let g = textureSample(input_tex, tex_sampler, uv + vec2(GREEN_OFFSET)).g;
    let b = textureSample(input_tex, tex_sampler, uv + vec2(BLUE_OFFSET)).b;

    var result = original_color;

    // effect starts around the edge
    let scale = length(uv - vec2(0.5)) / 0.5;

    if scale > START_SCALE {
        let t = smoothstep(START_SCALE, END_SCALE, scale);
        let aberrated_color = vec4f(r, g, b, original_color.a);
        result = mix(original_color, aberrated_color, vec4f(t));
    }

    return result;
}
