struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) @interpolate(flat) layer_idx: u32,
    @location(4) @interpolate(flat) type_: u32,
    @location(5) data_a: vec3f,
    @location(6) @interpolate(flat) data_b: vec3u,
};

@group(0) @binding(0)
var<uniform> camera_view: mat4x4f;
@group(0) @binding(1)
var<uniform> camera_proj: mat4x4f;
@group(0) @binding(2)
var<uniform> camera_pos: vec3f;
@group(1) @binding(0)
var<uniform> entity_mvp: array<mat4x4f, 1024>; // make sure to match the max entity count

@vertex
fn skybox_mask_vs(
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) @interpolate(flat) layer_idx: u32,
    @location(4) @interpolate(flat) type_: u32,
    @location(5) data_a: vec3f,
    @location(6) @interpolate(flat) data_b: vec3u,
) -> VertexOut {
    var output: VertexOut;

    let bone_idx = data_b[1];
    let model_view = entity_mvp[bone_idx];

    output.position = vs_handle_mvp(world_position, model_view, data_b, type_);

    output.world_position = world_position;
    output.tex_coord = tex_coord;
    output.normal = normal;
    output.layer_idx = layer_idx;
    output.type_ = type_;
    output.data_a = data_a;
    output.data_b = data_b;

    let is_sky = type_ == 0 && data_b[2] == 1;

    // reverse z
    // if not sky, make it far plane, which means it will fail stencil depth
    if !is_sky {
        output.position.z = 65536.0;
    }

    // let is_model = type_ == 1;
    // let is_entity_brush = type_ == 0 && data_b[1] != 0;

    // // dont write depth if it is entity brush
    // // could be optimized further if we check for texture mask
    // if is_model {
    //     output.position.z = 65536.0;
    // }

    return output;
}

@vertex
fn vs_main(
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) @interpolate(flat) layer_idx: u32,
    @location(4) @interpolate(flat) type_: u32,
    @location(5) data_a: vec3f,
    @location(6) @interpolate(flat) data_b: vec3u,
) -> VertexOut {
    var output: VertexOut;

    let bone_idx = data_b[1];
    let model_view = entity_mvp[bone_idx];

    output.position = vs_handle_mvp(world_position, model_view, data_b, type_);

    output.world_position = world_position;
    output.tex_coord = tex_coord;
    output.normal = normal;
    output.layer_idx = layer_idx;
    output.type_ = type_;
    output.data_a = data_a;
    output.data_b = data_b;

    return output;
}

fn vs_handle_mvp(world_position: vec3f, model_view: mat4x4f, data_b: vec3u, type_: u32) -> vec4f {
    if type_ == 2 {
        let packed_frame_orientation = data_b[2];
        let frame_count = packed_frame_orientation >> 16;
        let orientation_type = packed_frame_orientation & 0xFFFF;

        let sprite_pos = model_view[3].xyz;
        let scale = length(model_view[0].xyz);
        let cam_right = vec3f(camera_view[0][0], camera_view[1][0], camera_view[2][0]);
        let cam_up    = vec3f(camera_view[0][1], camera_view[1][1], camera_view[2][1]);
    
        switch orientation_type {
            // parallel up right
            case 0u: {
                let world_pos = sprite_pos + vec3f(
                    world_position.x * cam_right.x * scale,
                    world_position.x * cam_right.y * scale,
                    world_position.y * scale  // Z uses local Y (no camera up)
                );

                return camera_proj * camera_view * vec4f(world_pos, 1.0);
            }
            // facing up right
            // fcked up i wont care
            case 1u: {}
            // parallel
            case 2u: {
                let world_pos = sprite_pos + (world_position.x * cam_right + world_position.y * cam_up) * scale;

                return camera_proj * camera_view * vec4f(world_pos, 1.0);
            }
            // oriented
            // use the mvp
            case 3u: {
                return camera_proj * camera_view * model_view * vec4(world_position, 1.0);
            }
            // parallel oriented
            case 4u: {
                let rotated_right = model_view * vec4f(cam_right, 0.0);
                let rotated_up    = model_view * vec4f(cam_up, 0.0);

                let world_pos = sprite_pos + (world_position.x * rotated_right.xyz + world_position.y * rotated_up.xyz) * scale;

                return camera_proj * camera_view * vec4f(world_pos, 1.0);
            }
            // nothing
            default: {
                return camera_proj * camera_view * model_view * vec4(world_position, 1.0);
            }
        }
    }

    return camera_proj * camera_view * model_view * vec4(world_position, 1.0);
}

fn gamma_correct(color: vec3f) -> vec3f {
    let gamma: f32 = 1.6;
    return pow(color, vec3f(1.0 / gamma));
}

fn pixel_art_filter(uv: vec2f, layer_idx: u32) -> vec4f {
    let res = vec2f(textureDimensions(texture, layer_idx));
    let pixel_uv = uv * res;

    let seam = floor(pixel_uv + 0.5);
    let clamped_uv = seam + clamp((pixel_uv - seam) / fwidth(pixel_uv), vec2f(-0.5), vec2f(0.5));

    return textureSample(texture, linear_sampler, clamped_uv / res, layer_idx);
}

/// https://www.shadertoy.com/view/MllBWf
fn pixel_art_filter2(uv: vec2f, layer_idx: u32) -> vec4f {
    // all textures in the array have the same dimensions
    let res = vec2f(textureDimensions(texture, 0));
    let pixel_uv = uv * res + 0.5;

    let fl = floor(pixel_uv);
    var fr = fract(pixel_uv);

    let aa = fwidth(pixel_uv) * 1.0;

    fr = smoothstep(
        vec2f(0.5) - aa,
        vec2f(0.5) + aa,
        fr
    );

    let final_uv = (fl + fr - 0.5) / res;

    return textureSample(texture, linear_sampler, final_uv, layer_idx);
}

// higher mip level will have some alpha interpolated, and it won't be 1.0
// this makes alpha test discards fragments that should not be discarded
fn alpha_test(uv: vec2f, layer_idx: u32, color: vec3f, alpha: f32) -> vec3f {
    let alpha_threshold = 0.95;

    let tex_size = vec2f(textureDimensions(texture, 0));
    let deriv = max(length(dpdx(uv * tex_size)), length(dpdy(uv * tex_size)));
    let mip_level = clamp(log2(deriv), 0.0, 10.0);
    let adjusted_threshold = alpha_threshold * exp2(-mip_level * 0.5);

    if alpha < adjusted_threshold {
        discard;
    }

    // boost dark edge
    let compensation = 1.0 + 0.5 * mip_level;

    return color * compensation;
}

// https://www.shadertoy.com/view/XlBBRR
fn bicubic_filtering(uv: vec2f, layer_idx: u32) -> vec4f {
    let tex_size = vec2f(textureDimensions(texture, 0));
    let pixel_uv = uv * tex_size + 0.5;
    let fract_part = fract(pixel_uv);
    let floor_part = floor(pixel_uv);

    // Cubic interpolation polynomial (3-2x)x²
    // let weights = fract_part * fract_part * (3.0 - 2.0 * fract_part);
    // Wider interpolation curve (-4x² + 7x³ - 3x⁴)
    // let weights = fract_part * fract_part * (4.0 - fract_part * (7.0 - 3.0 * fract_part));
    // Alternative smoother version (uncomment if preferred):
    let weights = fract_part * fract_part * fract_part *
                 (fract_part * (fract_part * 6.0 - 15.0) + 10.0);

    let sample_uv = (floor_part + weights - 0.5) / tex_size;
    return textureSample(texture, linear_sampler, sample_uv, layer_idx);
}

// https://www.shadertoy.com/view/WtjyWy
fn nearest_aa_filtering(_uv: vec2f, layer_idx: u32) -> vec4f {
    let sharpness = 1.5;

    let tex_size = vec2f(textureDimensions(texture, 0));
    let tile_uv = _uv * tex_size;

    let dx = vec2f(dpdx(tile_uv.x), dpdy(tile_uv.x));
    let dy = vec2f(dpdx(tile_uv.y), dpdy(tile_uv.y));

    let dxdy = vec2f(
        max(abs(dx.x), abs(dx.y)),
        max(abs(dy.x), abs(dy.y))
    );

    let texel_delta = fract(tile_uv) - 0.5;
    let dist_from_edge = 0.5 - abs(texel_delta);

    let aa_factor = dist_from_edge * sharpness / dxdy;

    let uv = _uv - texel_delta * clamp(aa_factor, vec2f(0.0), vec2f(1.0)) / tex_size;
    return textureSample(texture, linear_sampler, uv, layer_idx);
}

// fragment
@group(2) @binding(0) var texture: texture_2d_array<f32>;
@group(2) @binding(1) var linear_sampler: sampler;
@group(2) @binding(2) var nearest_sampler: sampler;
@group(3) @binding(0) var lightmap: texture_2d<f32>;
@group(3) @binding(1) var lightmap_sampler: sampler;

struct PushConstants {
    render_flags: u32,
    time: f32,
}

// push constant is just `render_nodraw == 1`
var<push_constant> push_constants: PushConstants;

const RENDER_NODRAW_FLAG: u32 = 1u << 0u;
const FULL_BRIGHT_FLAG: u32 = 1u << 1u;

fn calculate_base_color(
    position: vec4f,
    tex_coord: vec2f,
    normal: vec3f,
    layer_idx: u32,
    type_: u32,
    data_a: vec3f,
    data_b: vec3u,
) -> vec4f {
    var albedo: vec4f;

    albedo = textureSample(texture, linear_sampler, tex_coord, layer_idx);
    // albedo = bicubic_filtering(tex_coord, layer_idx);
    // albedo = nearest_aa_filtering(tex_coord, layer_idx);
    // albedo = pixel_art_filter2(tex_coord, layer_idx);

    // need to explicitly specify we are shifting a u32 with u32
    let render_nodraw = (push_constants.render_flags & RENDER_NODRAW_FLAG) != 0u;
    let full_bright = (push_constants.render_flags & FULL_BRIGHT_FLAG) != 0u;

    if type_ == 0 {
        // this is bsp vertex
        let is_nodraw = data_b[1] == 2;

        if is_nodraw {
            if render_nodraw {
                return albedo;
            } else {
                discard;
            }
        }

        // it doesn't matter if we discard sky here or not
        // in the skybox pass, the stencil will write over fragments behind it regardless
        // just draw it anyway so depth test is updated
        // if is_sky {
        //     discard;
        // }

        let lightmap_coord = vec2f(data_a[0], data_a[1]);
        let light = textureSample(lightmap, lightmap_sampler, lightmap_coord).rgb
        // from the the game
        * (128.0 / 192.0);

        let rendermode = data_b[0];
        let renderamt = data_a[2];

        let alpha = min(albedo.a, renderamt);

        // pre multiply alpha and overbright
        var final_color = albedo.rgb * alpha * 2.0;

        // some render mode don't have lightmap
        // dont gamma corect it either because it is a bit too bright
        if rendermode == 0 || rendermode == 4 {
            final_color *= light;
            final_color = gamma_correct(final_color);
        }

        if rendermode == 4 {
            final_color = alpha_test(tex_coord, layer_idx, final_color, alpha);
        }

        // full bright goes last because we might want to discard fragments
        if full_bright {
            return albedo;
        }

        return vec4(final_color, alpha);
    } else if type_ == 1 {
        // this is mdl vertex

        let alpha = albedo.a;

        // pre multiply
        var final_color = albedo.rgb * alpha;

        // light is always pointing down
        let normal_z = (normal.z + 1.0) / 2.0;

        let texture_flags = data_b[0];

        // if not flatshade, don't do shading
        if (texture_flags & 1u) == 0 {
            final_color = final_color * normal_z;
        }

        // masked
        if (texture_flags & (1u << 6)) != 0 {
            final_color = alpha_test(tex_coord, layer_idx, final_color, alpha);
        }

        // additive
        if (texture_flags & (1u << 5)) != 0 {

        }

        // need to repeat it because we also want to filter othre stuffs we don't want to draw like nodraw :()
        if full_bright {
            return albedo;
        }

        return vec4(final_color, alpha);
    } else if type_ == 2 {
        let frame_count = data_b[2] >> 16;
        let frame_rate = data_a[0];
        let time = push_constants.time;

        let curr_frame = u32(time * frame_rate) % frame_count;

        // the frames are continuous
        albedo = textureSample(texture, linear_sampler, tex_coord, layer_idx + curr_frame);
    }

    return albedo;
}

// opaque objects
@fragment
fn fs_opaque(
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) @interpolate(flat) layer_idx: u32,
    @location(4) @interpolate(flat) type_: u32,
    @location(5) data_a: vec3f,
    @location(6) @interpolate(flat) data_b: vec3u,
) -> @location(0) vec4f {
    let color = calculate_base_color(position, tex_coord, normal, layer_idx, type_, data_a, data_b);

    // at this stage, the fragment is either discarded or it is fully opaque
    // hardcode alpha 1.0 here just to be safe
    // there are some alpha tested textures that are misused will have alpha 0 instead
    // even though they are not rendermode 4
    // should not do this inside the calculate_base_color because it is also used for transparent fragments
    return vec4(color.rgb, 1.0);
}

// WBOIT resolve
struct FragOutput {
    @location(0) accum: vec4f,
    @location(1) reveal: vec4f,
}

@fragment
fn fs_transparent(
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) @interpolate(flat) layer_idx: u32,
    @location(4) @interpolate(flat) type_: u32,
    @location(5) data_a: vec3f,
    @location(6) @interpolate(flat) data_b: vec3u,
) -> FragOutput {
    // let is_opposite = dot(normal, normalize(world_position - camera_pos)) > 0.0;

    let color = calculate_base_color(position, tex_coord, normal, layer_idx, type_, data_a, data_b);

    // -position.z goes like from 0 to 2
    // *100.0 because that is what the world looks like
    // and the depth texture is mostly white, very sad
    let depth_z = -position.z * 100.0;
    let distance_weight = clamp(0.03 / (1e-5 + pow(depth_z / 200.0, 4.0)), 1e-2, 3e3);
    let alpha_weight = min(1.0, max(color.r, max(color.g, max(color.b, color.a))) * 40.0 + 0.01);

    let weight = distance_weight * alpha_weight * alpha_weight;

    // reminder, colors are already pre multiplied
    let accum_color = color * weight;

    // revealage math shit that circumvents webgl2 INDEPENDENT_BLENDING
    let epsilon = 1e-6;
    let reveal_log = log(max(epsilon, 1.0 - color.a));
    // Output to the Red channel of the reveal target
    let reveal_color = vec4(reveal_log, 0.0, 0.0, 0.0);

    return FragOutput(
        accum_color,
        reveal_color
    );
}
