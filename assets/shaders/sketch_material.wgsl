#define_import_path grin::fragment

@group(1) @binding(100)
var<uniform> sketch_enabled: u32;

@group(1) @binding(101)
var<uniform> sketch_layer: u32;

@group(1) @binding(102)
var<uniform> sketch_texture_array: texture_2d_array<f32>;

@group(1) @binding(103)
var<uniform> sketch_texture_array_sampler: sampler;

@fragment
fn fragment(
    in: MeshVertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // overwriting the base color with the array-texture base color.

    // this (unfortunately) makes `base_color_texture(_sampler)` a bit of redundant GPU space.
    // the alternative is to go back to how I used to do it... copy-pasting engine shaders.
    // maybe, when I'm free, I'll try adding support for multidimensional standardmaterial textures
    // to the actual engine, but it's such a niche use case I don't even know if it will be used.
    // that said, I am not enough of an expert to know the performance implications of this.
    // maybe it's something I can look at when the game is finished.

    // TODO: I'm pretty sure if statements in a shader is a no-no... but turning it into a shader
    // flag is going to be difficult as of bevy 0.12.
    // however, I think if the condition is a uniform it shouldn't be a problem?
    // I should ask someone more experienced about this.
    if sketch_enabled == 1 {
        pbr_input.material.base_color *= textureSampleBias(sketch_texture_array, sketch_texture_array_sampler, uv, sketch_layer, view.mip_bias);
    }

    pbr_input.material.base_color = alpha_discard(
        pbr_input.material,
        pbr_input.material.base_color
    );

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}