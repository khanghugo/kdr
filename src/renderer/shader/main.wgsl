struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) normal: vec3f,
    @location(1) texCoord: vec2f,
    @location(2) lightmap_coord: vec2f,
};

@group(0) @binding(0)
var<uniform> camera: mat4x4f;

@vertex
fn vs_main(
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) texCoord: vec2f,
    @location(3) lightmap_coord: vec2f
) -> VertexOut {
    var output: VertexOut;

    output.position = camera * vec4f(pos, 1.0);
    output.texCoord = texCoord;
    output.normal = normal;
    output.lightmap_coord = lightmap_coord;

    return output;
}

// fragment
@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var linear_sampler: sampler;
@group(2) @binding(0) var lightmap: texture_2d<f32>;
@group(2) @binding(1) var lightmap_sampler: sampler;

fn gamma_correct(color: vec3f) -> vec3f {
    let gamma: f32 = 1.6;
    return pow(color, vec3f(1.0 / gamma));
}

@fragment
fn fs_main(
    @location(0) normal: vec3f, 
    @location(1) texCoord: vec2f,
    @location(2) lightmap_coord: vec2f
    ) -> @location(0) vec4f {
    let albedo = textureSample(texture, linear_sampler, texCoord);
    let light = textureSample(lightmap, lightmap_sampler, lightmap_coord).rgb * (128.0 / 192.0);
    let color = albedo.rgb * light.rgb * 2;

    let gamma_corrected = vec4(gamma_correct(color.rgb), albedo.a);

    return gamma_corrected;
}