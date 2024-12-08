#define_import_path berdicle

#import bevy_pbr::mesh_functions::get_world_from_local;
#import bevy_pbr::view_transformations::{
    position_world_to_clip, 
    position_world_to_view,
    position_view_to_clip,
};

@group(2) @binding(0) var<uniform> billboard: i32;
@group(2) @binding(1) var<uniform> color: vec4<f32>;
@group(2) @binding(2) var texture: texture_2d<f32>;
@group(2) @binding(3) var texture_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,

    @location(3) id: u32,
    @location(4) lifetime: f32,
    @location(5) fac: f32,
    @location(6) seed: f32,

    @location(7) transform_x: vec4<f32>,
    @location(8) transform_y: vec4<f32>,
    @location(9) transform_z: vec4<f32>,
    @location(10) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

    @location(0) id: u32,
    @location(1) lifetime: f32,
    @location(2) fac: f32,
    @location(3) seed: f32,
    @location(4) color: vec4<f32>,
    @location(5) normal: vec3<f32>,
    @location(6) uv: vec2<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    if billboard == 0 {
        let x = dot(vec4(vertex.position, 1.0), vertex.transform_x);
        let y = dot(vec4(vertex.position, 1.0), vertex.transform_y);
        let z = dot(vec4(vertex.position, 1.0), vertex.transform_z);
        out.clip_position = position_world_to_clip(vec3(x, y, z));
    } else {
        let transform = vec3(vertex.transform_x.w, vertex.transform_y.w, vertex.transform_z.w);
        let position = position_world_to_view(transform);
        out.clip_position = position_view_to_clip(position + vertex.position);
    }
    out.id = vertex.id;
    out.lifetime = vertex.lifetime;
    out.fac = vertex.fac;
    out.seed = vertex.seed;
    out.color = vertex.color;
    out.normal = vertex.normal;
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(texture, texture_sampler, input.uv);
    return input.color * color * sampled;
}