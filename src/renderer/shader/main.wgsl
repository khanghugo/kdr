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

// fragment
@group(1) @binding(0) var index_tex: texture_2d<u32>;
@group(1) @binding(1) var index_sampler: sampler;
@group(1) @binding(2) var palette_tex: texture_1d<f32>;
@group(1) @binding(3) var palette_sampler: sampler;

@fragment
fn fs_main(
    @location(0) normal: vec3f, 
    @location(1) texCoord: vec2f
    ) -> @location(0) vec4f {
    // Wrap UVs to [0.0, 1.0)
    let wrapped_uv = fract(texCoord);
    
    let tex_dims = textureDimensions(index_tex).xy;
    let coord_i32 = vec2<u32>(wrapped_uv * vec2f(f32(tex_dims.x), f32(tex_dims.y)));
    
    // cannot sample from uint for some reasons so that is fucked.
    let index_u32 = textureLoad(index_tex, coord_i32, 0).r;
    
    // look up in palette
    let palette_uv = f32(index_u32) / 255.0;
    let color = textureSample(palette_tex, palette_sampler, palette_uv);
    
    return color;
}