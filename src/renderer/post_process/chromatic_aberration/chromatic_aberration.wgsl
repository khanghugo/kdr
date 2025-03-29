// @group(0) @binding(0) var input_tex: texture_2d<f32>;
// @group(0) @binding(1) var tex_sampler: sampler;

// const RED_OFFSET = 0.0015;
// const GREEN_OFFSET = 0.001;
// const BLUE_OFFSET = -0.001;

// const START_SCALE = 0.85; // starts around the edge
// const END_SCALE = 0.95; // starts gradually until full effect

// @fragment
// fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
//     let tex_size = textureDimensions(input_tex);
//     let uv = frag_coord.xy / vec2f(tex_size);

//     let original_color = textureSample(input_tex, tex_sampler, uv);
//     let r = textureSample(input_tex, tex_sampler, uv + vec2(RED_OFFSET)).r;
//     let g = textureSample(input_tex, tex_sampler, uv + vec2(GREEN_OFFSET)).g;
//     let b = textureSample(input_tex, tex_sampler, uv + vec2(BLUE_OFFSET)).b;

//     var result = original_color;

//     // effect starts around the edge
//     let scale = length(uv - vec2(0.5)) / 0.5;

//     if scale > START_SCALE {
//         let t = smoothstep(START_SCALE, END_SCALE, scale);
//         let aberrated_color = vec4f(r, g, b, original_color.a);
//         result = mix(original_color, aberrated_color, vec4f(t));
//     }

//     return result;
// }

// vibe code solution
@group(0) @binding(0) var input_tex: texture_2d<f32>;
@group(0) @binding(1) var depth_tex: texture_depth_2d;
@group(0) @binding(2) var tex_sampler: sampler;

// Chromatic Aberration
const MAX_CA_STRENGTH = 0.02;     // Barely visible at max strength
const EDGE_FALLOFF_EXP = 3.0;       // Only affects extreme edges

// Depth of Field
const FOCUS_DISTANCE = 0.65;        // Focus on middle distance
const BLUR_RANGE = 0.55;            // Narrow transition zone

@fragment
fn fs_main(@builtin(position) frag_coord: vec4f) -> @location(0) vec4f {
    let tex_size = vec2f(textureDimensions(input_tex));
    let uv = frag_coord.xy / tex_size;

    // Sample depth and calculate blurriness
    let depth = textureSample(depth_tex, tex_sampler, uv);
    let depth_dist = abs(depth - FOCUS_DISTANCE);
    let blur_factor = smoothstep(
        FOCUS_DISTANCE - BLUR_RANGE,
        FOCUS_DISTANCE + BLUR_RANGE,
        depth_dist
    );

    // PROPER SCREEN DISTANCE CALCULATION
    let screen_dist = length(uv - vec2(0.5));  // Range: 0-0.707 (center to corner)
    let edge_factor = pow(screen_dist * 1.414, EDGE_FALLOFF_EXP);  // Normalize to 0-1

    // Combined CA strength with clamping
    let ca_strength = min(blur_factor * edge_factor, 1.0) * MAX_CA_STRENGTH;

    // Subtle CA with multiple offsets
    let r_offset = vec2(ca_strength * 0.7, 0.0);  // Horizontal only
    let b_offset = vec2(-ca_strength * 0.7, 0.0);

    let r = textureSample(input_tex, tex_sampler, uv + r_offset).r;
    let g = textureSample(input_tex, tex_sampler, uv).g;
    let b = textureSample(input_tex, tex_sampler, uv + b_offset).b;

    return vec4f(r, g, b, 1.0);
}
