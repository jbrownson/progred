struct Params {
    row_x: vec4<f32>,
    row_y: vec4<f32>,
    // source is premultiplied, mask is full-size, target width, target height
    flags: vec4<f32>,
}
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var mask: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: Params;

@vertex fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(positions[index], 0.0, 1.0);
}

fn pixel(p: vec2<i32>) -> vec4<f32> {
    let hi = vec2<i32>(textureDimensions(source)) - vec2(1);
    let color = textureLoad(source, clamp(p, vec2(0), hi), 0);
    return select(vec4(color.rgb * color.a, color.a), color, params.flags.x != 0.0);
}

@fragment fn fragment(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let point = vec3(p.xy, 1.0);
    let uv = vec2(dot(params.row_x.xyz, point), dot(params.row_y.xyz, point)) - vec2(0.5);
    let lo = vec2<i32>(floor(uv));
    let f = fract(uv);
    let color = mix(mix(pixel(lo), pixel(lo + vec2(0, 1)), f.y),
                    mix(pixel(lo + vec2(1, 0)), pixel(lo + vec2(1, 1)), f.y), f.x);
    let mask_point = select(vec2(0), vec2<i32>(p.xy), params.flags.y != 0.0);
    return color * textureLoad(mask, mask_point, 0).a;
}
