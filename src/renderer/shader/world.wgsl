struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) world_position: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) layer_idx: u32,
    @location(4) model_idx: u32,
    @location(5) type_: u32,
    @location(6) data_a: vec3f,
    @location(7) data_b: vec2u,
};

@group(0) @binding(0)
var<uniform> camera_view: mat4x4f;
@group(0) @binding(1)
var<uniform> camera_proj: mat4x4f;
@group(0) @binding(2)
var<uniform> camera_pos: vec3f;
@group(1) @binding(0)
var<storage> model_view_array: array<mat4x4f>;

@vertex
fn z_prepass_vs(
    @location(0) pos: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) layer_idx: u32,
    @location(4) model_idx: u32,
    @location(5) type_: u32,
    @location(6) data_a: vec3f,
    @location(7) data_b: vec2u,
) -> VertexOut {
    var output: VertexOut;

    let model_view = model_view_array[model_idx];

    let clip_pos = camera_proj * camera_view * model_view * vec4(pos, 1.0);

    output.position = clip_pos;
    output.world_position = pos;
    output.tex_coord = tex_coord;
    output.normal = normal;
    output.layer_idx = layer_idx;
    output.model_idx = model_idx;
    output.type_ = type_;
    output.data_a = data_a;
    output.data_b = data_b;

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
    @location(0) pos: vec3f,
    @location(1) tex_coord: vec2f,
    @location(2) normal: vec3f,
    @location(3) layer_idx: u32,
    @location(4) model_idx: u32,
    @location(5) type_: u32,
    @location(6) data_a: vec3f,
    @location(7) data_b: vec2u,
) -> VertexOut {
    var output: VertexOut;

    let model_view = model_view_array[model_idx];

    let clip_pos = camera_proj * camera_view * model_view * vec4(pos, 1.0);

    output.position = clip_pos;
    output.world_position = pos;
    output.tex_coord = tex_coord;
    output.normal = normal;
    output.layer_idx = layer_idx;
    output.model_idx = model_idx;
    output.type_ = type_;
    output.data_a = data_a;
    output.data_b = data_b;

    return output;
}

// fragment
@group(2) @binding(0) var texture: texture_2d_array<f32>;
@group(2) @binding(1) var linear_sampler: sampler;
@group(2) @binding(2) var nearest_sampler: sampler;
@group(3) @binding(0) var lightmap: texture_2d<f32>;
@group(3) @binding(1) var lightmap_sampler: sampler;

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

fn calculate_base_color(
    position: vec4f,
    tex_coord: vec2f,
    normal: vec3f,
    layer_idx: u32,
    model_idx: u32,
    type_: u32,
    data_a: vec3f,
    data_b: vec2u,
) -> vec4f {
    var albedo: vec4f;

    // albedo = textureSample(texture, linear_sampler, tex_coord, layer_idx);
    albedo = bicubic_filtering(tex_coord, layer_idx);
    // albedo = nearest_aa_filtering(tex_coord, layer_idx);
    // albedo = pixel_art_filter2(tex_coord, layer_idx);

    if type_ == 0 {
        // this is bsp vertex
        let is_sky = data_b[1];

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

        return vec4(final_color, alpha);
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
    @location(3) layer_idx: u32,
    @location(4) model_idx: u32,
    @location(5) type_: u32,
    @location(6) data_a: vec3f,
    @location(7) data_b: vec2u,
) -> @location(0) vec4f {
    let color = calculate_base_color(position, tex_coord, normal, layer_idx, model_idx, type_, data_a, data_b);

    return color;
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
    @location(3) layer_idx: u32,
    @location(4) model_idx: u32,
    @location(5) type_: u32,
    @location(6) data_a: vec3f,
    @location(7) data_b: vec2u,
) -> FragOutput {
    // let is_opposite = dot(normal, normalize(world_position - camera_pos)) > 0.0;

    // if is_opposite {
    //     discard;
    // }

    let color = calculate_base_color(position, tex_coord, normal, layer_idx, model_idx, type_, data_a, data_b);

    // -position.z goes like from 0 to 2
    // *100.0 because that is what the world looks like
    // and the depth texture is mostly white, very sad
    let depth_z = -position.z * 100.0;
    let distance_weight = clamp(0.03 / (1e-5 + pow(depth_z / 200.0, 4.0)), 1e-2, 3e3);
    let alpha_weight = min(1.0, max(color.r, max(color.g, max(color.b, color.a))) * 40.0 + 0.01);

    let weight = distance_weight * alpha_weight * alpha_weight;

    return FragOutput(
        color * weight,
        vec4(color.a, 0.0, 0.0, color.a)
    );
}
