struct Camera {
    model_to_view: mat4x4<f32>,
    projection: vec4<f32>,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) view_position: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

@vertex fn vertex(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>, @location(2) normal: vec4<f32>) -> VertexOutput {
    let p = (camera.model_to_view * vec4(position, 1.0)).xyz;
    var output: VertexOutput;
    output.position = vec4(p.xy * camera.projection.xy, p.z * camera.projection.z + camera.projection.w, 1.0);
    output.view_position = p;
    output.color = color;
    output.normal = (camera.model_to_view * vec4(normal.xyz, 0.0)).xyz;
    return output;
}

@fragment fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let face = cross(dpdx(input.view_position), dpdy(input.view_position));
    let flat = select(face, -face, face.z < 0.0);
    let interpolated = select(-input.normal, input.normal, front);
    let normal = normalize(select(flat, interpolated, dot(input.normal, input.normal) > 0.0));
    let light = normalize(vec3(0.35, -0.45, 1.0));
    let brightness = 0.22 + 0.78 * max(dot(normal, light), 0.0);
    return vec4(input.color * brightness, 1.0);
}
