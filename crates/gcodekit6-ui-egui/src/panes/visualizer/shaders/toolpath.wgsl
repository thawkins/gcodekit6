// Toolpath lines (the line shader): each vertex carries its
// program line, and lines the machine has acknowledged (`line < progress`) are drawn in
// `done_color`. The tag's top bit marks travel; travel or cuts the sidebar hides (`show`
// bit 1 travel, bit 2 cuts) are moved outside the clip volume, so they draw nothing.
// Colours arrive in sRGB and leave premultiplied, converted to linear when the target is an
// sRGB format.

struct Uniforms {
    view_proj: mat4x4<f32>,
    done_color: vec4<f32>,
    progress: u32,
    srgb_target: u32,
    show: u32,
    pad: u32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

fn finish(c: vec4<f32>) -> vec4<f32> {
    var rgb = c.rgb;
    if (u.srgb_target != 0u) {
        rgb = to_linear(rgb);
    }
    return vec4<f32>(rgb * c.a, c.a);
}

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) tag: u32,
) -> VertexOut {
    var out: VertexOut;
    let travel = (tag & 0x80000000u) != 0u;
    let line = tag & 0x7fffffffu;
    let shown = select(u.show & 2u, u.show & 1u, travel) != 0u;
    if (shown) {
        out.position = u.view_proj * vec4<f32>(pos, 1.0);
    } else {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    }
    if (line < u.progress) {
        out.color = u.done_color;
    } else {
        out.color = color;
    }
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return finish(in.color);
}
