struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) texCoord: vec2f,
    @location(1) layer_idx: u32,
};

@group(0) @binding(0)
var<uniform> camera: mat4x4f;

@vertex
fn vs_main(
    @location(0) pos: vec3f,
    @location(1) texCoord: vec2f,
    @location(2) layer_idx: u32,
    @location(3) model_proj: mat4x4f,
) -> VertexOut {
    var output: VertexOut;

    output.position = camera * model_proj * vec4f(pos, 1.0);
    output.texCoord = texCoord;
    output.layer_idx = layer_idx;

    return output;
}

// fragment
@group(1) @binding(0) var texture: texture_2d_array<f32>;
@group(1) @binding(1) var linear_sampler: sampler;

fn gamma_correct(color: vec3f) -> vec3f {
    let gamma: f32 = 1.6;
    return pow(color, vec3f(1.0 / gamma));
}

@fragment
fn fs_main(
    @location(0) texCoord: vec2f,
    @location(1) layer_idx: u32,
    ) -> @location(0) vec4f {
    let albedo = textureSample(texture, linear_sampler, texCoord, layer_idx);

    // let light = textureSample(lightmap, lightmap_sampler, lightmap_coord).rgb 
    // // from the the game
    // * (128.0 / 192.0);
    
    // // overbright
    // let color = albedo.rgb * light.rgb * 2;

    let gamma_corrected = vec4(gamma_correct(albedo.rgb), albedo.a);

    return gamma_corrected;
}