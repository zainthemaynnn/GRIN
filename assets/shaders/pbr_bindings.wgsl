#define_import_path grin::pbr_bindings

#import grin::pbr_types

@group(1) @binding(0)
var<uniform> material: StandardMaterial;
@group(1) @binding(1)
var base_color_texture: texture_2d_array<f32>; // NOTE: changed from original!
@group(1) @binding(2)
var base_color_sampler: sampler;
@group(1) @binding(3)
var emissive_texture: texture_2d<f32>;
@group(1) @binding(4)
var emissive_sampler: sampler;
@group(1) @binding(5)
var metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(6)
var metallic_roughness_sampler: sampler;
@group(1) @binding(7)
var occlusion_texture: texture_2d<f32>;
@group(1) @binding(8)
var occlusion_sampler: sampler;
@group(1) @binding(9)
var normal_map_texture: texture_2d<f32>;
@group(1) @binding(10)
var normal_map_sampler: sampler;

///@group(1) @binding(11)
//var<uniform> time;
//@group(1) @binding(12)
//var<uniform> dissolve: Dissolve;
//@group(1) @binding(13)
//var dissolve_texture: texture_2d<f32>;
//@group(1) @binding(14)
//var dissolve_sampler: sampler;
//@group(1) @binding(15)
//var<uniform> glitch: Glitch;
