struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) normal: vec3f,
    @location(1) texCoord: vec2f,
};

@group(0) @binding(0)
var<uniform> camera: mat4x4f;

@vertex
fn vs_main(
    @location(0) pos: vec3f,
    @location(1) normal: vec3f,
    @location(2) texCoord: vec2f
) -> VertexOut {
    var output: VertexOut;
    output.position = camera * vec4f(pos, 1.0);
    output.texCoord = texCoord;
    output.normal = normal;
    return output;
}

@group(1) @binding(0) var sampler0: sampler;
@group(1) @binding(1) var current_texture: texture_2d<f32>;

@fragment
fn fs_main(
    @location(0) normal: vec3f, 
    @location(1) texCoord: vec2f
    ) -> @location(0) vec4f {
    let tex_color = textureSampleLevel(current_texture, sampler0, texCoord, 0.0);
    // let tex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);

    return tex_color;
}