// https://github.com/gfx-rs/wgpu/blob/trunk/examples/features/src/mipmap/blit.wgsl
@vertex
fn vs_main(@builtin(vertex_index) vert_idx: u32) -> @builtin(position) vec4f {
    let pos = array(
        vec2f(-1.0, -1.0),
        vec2f(3.0, -1.0),
        vec2f(-1.0, 3.0)
    );

    return vec4f(pos[vert_idx], 0.0, 1.0);
}
