#import bevy_pbr::mesh_view_bindings

struct BWStatic {
    t: f32,
}

@group(1) @binding(0)
var<uniform> bw_static: BWStatic;

// https://thebookofshaders.com/10/
fn random(uv: vec2<f32>) -> f32 {
    return fract(sin(dot(uv, vec2<f32>(12.9898, 78.233))) * 43758.5453123);
}

struct VertexIn {
    @location(0) pos: vec3<f32>,
}

struct VertexOut {
    @builtin(position) frag_coord: vec4<f32>,
}

struct FragmentIn {
    @builtin(position) frag_coord: vec4<f32>,
}

struct FragmentOut {
    @builtin(frag_depth) depth: f32,
    @location(0) color: vec4<f32>,
}

@vertex
fn vertex(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.frag_coord = view.view_proj * vec4<f32>(in.pos, 1.0);
    return out;
}

@fragment
fn fragment(in: FragmentIn) -> FragmentOut {
    // simple as!
    var out: FragmentOut;
    out.color = vec4<f32>(vec3<f32>(random(in.frag_coord.xy + vec2(bw_static.t))), 1.0);
    out.depth = in.frag_coord.z;
    return out;
}
