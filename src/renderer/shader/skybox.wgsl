struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) tex_coords: vec3f,
};

@group(0) @binding(0)
var<uniform> camera_view: mat4x4f;
@group(0) @binding(1)
var<uniform> camera_proj: mat4x4f;

@group(1) @binding(0)
var skybox: texture_cube<f32>;
@group(1) @binding(1)
var skybox_sampler: sampler;

@vertex
fn vs_main(@location(0) position: vec3f) -> VertexOutput {
    var output: VertexOutput;

    // need to rotate 90 degrees clockwise around z up axis
    let rotated_position = vec3f(-position.y, position.x, position.z);

    // dont flip the coordinate
    output.tex_coords = vec3f(rotated_position.x, rotated_position.z, rotated_position.y);;

    // Project position but ignore translation (only rotation)
    let view_no_translation = mat4x4f(
        camera_view[0],
        camera_view[1],
        camera_view[2],
        vec4f(0.0, 0.0, 0.0, 1.0)
    );

    // Set position to always be at the far plane
    let pos = camera_proj * view_no_translation * vec4f(position, 1.0);
    output.position = pos.xyww; // Set z = w so depth is always 1.0

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4f {
    return textureSample(skybox, skybox_sampler, input.tex_coords);
}
