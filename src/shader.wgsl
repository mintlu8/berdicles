#define_import_path berdicle

#import bevy_pbr::mesh_functions::get_world_from_local;
#import bevy_pbr::view_transformations::{
    position_world_to_clip, 
    position_world_to_view,
    position_view_to_clip,
};

@group(1) @binding(0) var<uniform> local_to_world_x: vec4<f32>;
@group(1) @binding(1) var<uniform> local_to_world_y: vec4<f32>;
@group(1) @binding(2) var<uniform> local_to_world_z: vec4<f32>;

@group(2) @binding(0) var<uniform> color: vec4<f32>;
@group(2) @binding(1) var texture: texture_2d<f32>;
@group(2) @binding(2) var texture_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
    @location(2) uv: vec2<f32>,
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) vertex_color: vec4<f32>,
#endif

    @location(10) id: u32,
    @location(11) lifetime: f32,
    @location(12) seed: f32,
    @location(13) fac: f32,

    @location(14) transform_x: vec4<f32>,
    @location(15) transform_y: vec4<f32>,
    @location(16) transform_z: vec4<f32>,
    @location(17) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

    @location(0) id: u32,
    @location(1) lifetime: f32,
    @location(2) fac: f32,
    @location(3) seed: f32,
    @location(4) color: vec4<f32>,
    @location(5) uv: vec2<f32>,
#ifdef VERTEX_NORMALS
    @location(6) normal: vec3<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(7) vertex_color: vec4<f32>,
#endif
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
#ifndef BILLBOARD
    let position = vec3(
        dot(vec4(vertex.position, 1.0), vertex.transform_x),
        dot(vec4(vertex.position, 1.0), vertex.transform_y),
        dot(vec4(vertex.position, 1.0), vertex.transform_z),
    );
    let world_position = vec3(
        dot(vec4(position, 1.0), local_to_world_x),
        dot(vec4(position, 1.0), local_to_world_y),
        dot(vec4(position, 1.0), local_to_world_z),
    );
    out.clip_position = position_world_to_clip(world_position);
#ifdef VERTEX_NORMALS
    // This only works if scale is uniform, otherwise an approximation.
    // todo: fix this
    let normal = vec3(
        dot(vec4(vertex.normal, 1.0), vertex.transform_x),
        dot(vec4(vertex.normal, 1.0), vertex.transform_y),
        dot(vec4(vertex.normal, 1.0), vertex.transform_z),
    );
    let world_normal = vec3(
        dot(vec4(normal, 1.0), local_to_world_x),
        dot(vec4(normal, 1.0), local_to_world_y),
        dot(vec4(normal, 1.0), local_to_world_z),
    );
    out.normal = normalize(world_normal);
#endif
#else
    let transform = vec3(vertex.transform_x.w, vertex.transform_y.w, vertex.transform_z.w);
    let world_position = vec3(
        dot(vec4(transform, 1.0), local_to_world_x),
        dot(vec4(transform, 1.0), local_to_world_y),
        dot(vec4(transform, 1.0), local_to_world_z),
    );
    let position = position_world_to_view(world_position);
    let vertex_position = vec3(
        dot(vec2(vertex.position.xy), vertex.transform_x.xy),
        dot(vec2(vertex.position.xy), vertex.transform_y.xy),
        vertex.position.z
    );
    out.clip_position = position_view_to_clip(position + vertex_position);
#ifdef VERTEX_NORMALS
    // The intension is 2d object, so don't change the normal
    out.normal = vertex.normal;
#endif
#endif
    out.id = vertex.id;
    out.lifetime = vertex.lifetime;
    out.fac = vertex.fac;
    out.seed = vertex.seed;
    out.color = vertex.color;
#ifdef VERTEX_COLORS
    out.vertex_color = vertex.vertex_color;
#endif
    out.uv = vertex.uv;
    return out;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(texture, texture_sampler, input.uv);
    return input.color * color * sampled
#ifdef VERTEX_COLORS
        * input.vertex_color
#endif
    ;
}