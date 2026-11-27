// Stock removal surface (the stock removal shader): position,
// normal and colour per vertex, a diffuse term (at least 0.1) and a soft specular highlight
// with the light in view space at (0.35, 0.35, 1). The original drew into a non-sRGB target, so
// the shaded colour is taken as sRGB and converted to linear when the target is an sRGB
// format. Output is opaque.

struct Uniforms {
    view_proj: mat4x4<f32>,
    done_color: vec4<f32>,
    progress: u32,
    srgb_target: u32,
    pad: vec2<u32>,
    // Inverse transpose of the view's rotation, in the upper-left 3x3.
    normal_matrix: mat4x4<f32>,
    light_dir: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / 12.92;
    let high = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = u.view_proj * vec4<f32>(pos, 1.0);
    out.normal = normalize((u.normal_matrix * vec4<f32>(normal, 0.0)).xyz);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let light_dir = normalize(u.light_dir.xyz);
    let diffuse = max(dot(normal, light_dir), 0.1);
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);
    let half_dir = normalize(light_dir + view_dir);
    let specular = pow(max(dot(normal, half_dir), 0.0), 24.0) * 0.3;
    var rgb = clamp(in.color.rgb * diffuse + vec3<f32>(specular), vec3<f32>(0.0), vec3<f32>(1.0));
    if (u.srgb_target != 0u) {
        rgb = to_linear(rgb);
    }
    return vec4<f32>(rgb, 1.0);
}
