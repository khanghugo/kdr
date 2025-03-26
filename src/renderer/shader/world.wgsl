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
@group(3) @binding(0) var lightmap: texture_2d<f32>;
@group(3) @binding(1) var lightmap_sampler: sampler;

fn gamma_correct(color: vec3f) -> vec3f {
    let gamma: f32 = 1.6;
    return pow(color, vec3f(1.0 / gamma));
}

fn calculate_base_color(
    tex_coord: vec2f,
    normal: vec3f,
    layer_idx: u32,
    model_idx: u32,
    type_: u32,
    data_a: vec3f,
    data_b: vec2u,
) -> vec4f {
    let albedo = textureSample(texture, linear_sampler, tex_coord, layer_idx);

    // alpha testing
    if albedo.a != 1.0 {
        discard;
    }

    if type_ == 0 {
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
        if rendermode != 2 {
            final_color *= light;
            final_color = gamma_correct(final_color);
        }

        return vec4(final_color, alpha);
    } else if type_ == 1 {
        let alpha = min(albedo.a, 1.0);

        var final_color = albedo.rgb * alpha;

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
    let color = calculate_base_color(tex_coord, normal, layer_idx, model_idx, type_, data_a, data_b);

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

    let color = calculate_base_color(tex_coord, normal, layer_idx, model_idx, type_, data_a, data_b);

    // -position.z goes like from 0 to 2
    // *100.0 because that is what the world looks like
    let depth_z = -position.z * 100.0;
    let distance_weight = clamp(0.03 / (1e-5 + pow(depth_z / 200.0, 4.0)), 1e-2, 3e3);
    let alpha_weight = min(1.0, max(color.r, max(color.g, max(color.b, color.a))) * 40.0 + 0.01);

    let weight = distance_weight * alpha_weight * alpha_weight;

    return FragOutput(
        color * weight,
        vec4(color.a, 0.0, 0.0, color.a)
    );
}
