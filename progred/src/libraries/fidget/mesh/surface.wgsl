@group(0) @binding(0) var color: texture_2d<f32>;
@group(0) @binding(1) var depth: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var output: VertexOutput;
    output.position = vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    output.uv = uv;
    return output;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment fn fragment(input: VertexOutput) -> FragmentOutput {
    let size = textureDimensions(depth);
    let pixel = clamp(vec2<i32>(input.uv * vec2<f32>(size)), vec2(0), vec2<i32>(size) - vec2(1));
    let z = textureLoad(depth, pixel, 0).r;
    if z < 0.0 { discard; }
    var output: FragmentOutput;
    output.color = textureLoad(color, pixel, 0);
    output.depth = z;
    return output;
}
