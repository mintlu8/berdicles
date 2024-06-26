//! Shader module for `berdicles`.

use bevy::{asset::Handle, render::render_resource::Shader};

const VERTEX: &str = stringify!(
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
);

const VERTEX_OUT: &str = stringify!(
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
);

const VERTEX_FN: &str = stringify!(
    @vertex
    fn vertex(vertex: Vertex) -> VertexOutput {
        var out: VertexOutput;
        var x = dot(vec4(vertex.position, 1.0), vertex.transform_x);
        var y = dot(vec4(vertex.position, 1.0), vertex.transform_y);
        var z = dot(vec4(vertex.position, 1.0), vertex.transform_z);
        out.clip_position = position_world_to_clip(vec3(x, y, z));
        out.id = vertex.id;
        out.lifetime = vertex.lifetime;
        out.fac = vertex.fac;
        out.seed = vertex.seed;
        out.color = vertex.color;
        out.normal = vertex.normal;
        out.uv = vertex.uv;
        return out;
    }
);

const FRAGMENT_FN: &str = stringify!(
    @group(2) @binding(0) var<uniform> color: vec4<f32>;
    @group(2) @binding(1) var texture: texture_2d<f32>;
    @group(2) @binding(2) var texture_sampler: sampler;

    @fragment
    fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
        let sampled = textureSample(texture, texture_sampler, input.uv);
        return input.color * color * sampled;
    }
);

const DBG_FN: &str = stringify!(
    @fragment
    fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
        if front {
            return vec4(0.0, 0.0, 1.0, 0.0);
        } else {
            return vec4(1.0, 0.0, 0.0, 0.0);
        }
    }
);

pub static SHADER_VERTEX: &str = const_format::concatcp!(
    "#import bevy_pbr::mesh_functions::get_world_from_local\n",
    "#import bevy_pbr::view_transformations::position_world_to_clip,\n",
    VERTEX,
    VERTEX_OUT,
    VERTEX_FN
);

pub static SHADER_FRAGMENT: &str = const_format::concatcp!(VERTEX_OUT, FRAGMENT_FN);

pub static SHADER_DBG: &str = const_format::concatcp!(VERTEX_OUT, DBG_FN);

const fn weak_from_str(s: &str) -> Handle<Shader> {
    if s.len() > 16 {
        panic!()
    }
    let mut bytes = [0u8; 16];
    let s = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        bytes[i] = s[i];
        i += 1;
    }
    Handle::weak_from_u128(u128::from_le_bytes(bytes))
}

pub static PARTICLE_VERTEX: Handle<Shader> = weak_from_str("berdicle/vert");
pub static PARTICLE_FRAGMENT: Handle<Shader> = weak_from_str("berdicle/frag");
pub static PARTICLE_DBG_FRAGMENT: Handle<Shader> = weak_from_str("berdicle/dbg");
