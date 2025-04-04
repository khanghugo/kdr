@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var bloom_tex: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4f {
    let tex_size = vec2f(textureDimensions(scene_tex));
    let uv = position.xy / tex_size;

    let scene_color = textureSample(scene_tex, tex_sampler, uv);
    let bloom_color = textureSample(bloom_tex, tex_sampler, uv);
    return scene_color + bloom_color * 1.2; // Adjust bloom strength
}
