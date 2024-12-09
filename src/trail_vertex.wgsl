#define_import_path berdicle

#import bevy_pbr::{
    mesh_bindings::mesh,
    mesh_functions,
    skinning,
    morph::morph,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::view,
}

// How this works:
// We stores tangent in normal `(next - prev).normalize()`
// and width in uv_b
// We move one point down `view x tangent * width` and other `-view x tangent * width`.
// Normal is `-view`
@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    var world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

    var tangent = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
    
    out.world_position = mesh_functions::mesh_position_local_to_world(world_from_local, vec4<f32>(vertex.position, 1.0));

    out.world_normal = normalize(out.world_position.xyz - view.world_position);
    out.world_position += vec4(cross(tangent, out.world_normal), 0.0) * vertex.uv_b.x; 
    
    out.position = position_world_to_clip(out.world_position.xyz);

    out.uv = vertex.uv;
    out.uv_b = vertex.uv;

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif

#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif

    return out;
}
