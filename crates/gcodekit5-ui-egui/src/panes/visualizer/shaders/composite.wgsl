// Copies the offscreen 3D scene (stock and toolpath, drawn with a depth buffer) into egui's
// pass over the view. The scene is premultiplied, one texel per view pixel; a full-view
// triangle reads it with textureLoad.

@group(0) @binding(0) var scene: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(scene));
    let texel = clamp(vec2<i32>(in.uv * vec2<f32>(size)), vec2<i32>(0), size - vec2<i32>(1));
    return textureLoad(scene, texel, 0);
}
