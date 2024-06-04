use bevy::{
    core_pipeline::core_3d::Opaque3d,
    ecs::{
        query::ROQueryItem,
        system::{
            lifetimeless::{Read, SRes},
            SystemParamItem,
        },
    },
    pbr::{
        DrawMesh, MeshPipeline, MeshPipelineKey, MeshPipelineViewLayoutKey, SetMeshViewBindGroup,
        MAX_CASCADES_PER_LIGHT, MAX_DIRECTIONAL_LIGHTS,
    },
    prelude::*,
    render::{
        extract_component::{ComponentUniforms, DynamicUniformIndex, UniformComponentPlugin},
        mesh::MeshVertexBufferLayout,
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, RenderCommand, RenderCommandResult, RenderPhase,
            SetItemPipeline, TrackedRenderPass,
        },
        render_resource::{
            BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType,
            BlendState, BufferBindingType, ColorTargetState, ColorWrites, CompareFunction,
            DepthBiasState, DepthStencilState, FragmentState, MultisampleState, PipelineCache,
            PrimitiveState, RenderPipelineDescriptor, ShaderDefVal, ShaderSize, ShaderStages,
            ShaderType, SpecializedMeshPipeline, SpecializedMeshPipelineError,
            SpecializedMeshPipelines, TextureFormat, VertexState,
        },
        renderer::RenderDevice,
        texture::BevyDefault,
        view::ExtractedView,
        Extract, Render, RenderApp, RenderSet,
    },
};

pub struct BWStaticPlugin;

impl Plugin for BWStaticPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UniformComponentPlugin::<BWStaticUniform>::default())
            .get_sub_app_mut(RenderApp)
            .expect("Failed to get render app.")
            .init_resource::<SpecializedMeshPipelines<BWStaticPipeline>>()
            .add_render_command::<Opaque3d, DrawBWStatic>()
            .add_systems(ExtractSchedule, extract_bw_static_uniforms)
            .add_systems(
                Render,
                (queue_bind_group, queue_phase_item).in_set(RenderSet::Queue),
            );
    }

    fn finish(&self, app: &mut App) {
        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .init_resource::<BWStaticPipeline>();
    }
}

#[derive(Component, Clone)]
pub struct BWStaticEffect {
    pub rate: f32,
    pub enabled: bool,
}

impl Default for BWStaticEffect {
    fn default() -> Self {
        Self {
            rate: 1.0 / 16.0,
            enabled: true,
        }
    }
}

#[derive(Resource)]
struct BWStaticPipeline {
    mesh_pipeline: MeshPipeline,
    effect_layout: BindGroupLayout,
    shader: Handle<Shader>,
}

impl FromWorld for BWStaticPipeline {
    fn from_world(world: &mut World) -> Self {
        let world_cell = world.cell();
        let render_device = world_cell.resource::<RenderDevice>();
        let mesh_pipeline = world_cell.resource::<MeshPipeline>().to_owned();
        let shader = world_cell
            .resource::<AssetServer>()
            .load("shaders/static.wgsl");

        let effect_layout = render_device.create_bind_group_layout(
            Some("bw_static_bind_group_layout"),
            &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: Some(BWStaticUniform::SHADER_SIZE),
                },
                count: None,
            }],
        );

        Self {
            mesh_pipeline,
            effect_layout,
            shader,
        }
    }
}

impl SpecializedMeshPipeline for BWStaticPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut shader_defs = Vec::new();
        shader_defs.push(ShaderDefVal::Int(
            "MAX_DIRECTIONAL_LIGHTS".to_string(),
            MAX_DIRECTIONAL_LIGHTS as i32,
        ));
        shader_defs.push(ShaderDefVal::Int(
            "MAX_CASCADES_PER_LIGHT".to_string(),
            MAX_CASCADES_PER_LIGHT as i32,
        ));
        let vertex_buffer_layout = layout.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            //Mesh::ATTRIBUTE_COLOR.at_shader_location(1),
        ])?;
        let view_layout = self
            .mesh_pipeline
            .get_view_layout(MeshPipelineViewLayoutKey::from(key))
            .clone();

        Ok(RenderPipelineDescriptor {
            label: Some("bw_static_pipeline".into()),
            layout: vec![view_layout, self.effect_layout.clone()],
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: "vertex".into(),
                buffers: vec![vertex_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                entry_point: "fragment".into(),
                shader_defs: shader_defs.clone(),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::bevy_default(),
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                ..Default::default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Greater,
                stencil: Default::default(),
                // TODO: HOW THE HECK DO YOU TUNE THIS
                bias: DepthBiasState {
                    slope_scale: 0.5,
                    ..Default::default()
                },
            }),
            push_constant_ranges: vec![],
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        })
    }
}

#[derive(Component, Clone, ShaderType)]
struct BWStaticUniform {
    t: f32,
}

fn extract_bw_static_uniforms(
    mut commands: Commands,
    query: Extract<Query<(Entity, &BWStaticEffect)>>,
    time: Res<Time>,
    mut last_t: Local<f32>,
    mut seed: Local<f32>,
) {
    for (entity, effect) in query.iter() {
        if !effect.enabled {
            continue;
        }
        let t = (time.elapsed_seconds_wrapped() / effect.rate).trunc();
        if t != *last_t {
            *last_t = t;
            *seed = rand::random::<f32>() * 128.0;
        }
        commands
            .get_or_spawn(entity)
            .insert(BWStaticUniform { t: *seed });
    }
}

#[rustfmt::skip]
type DrawBWStatic = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetBWStaticUniformBindGroup<1>,
    DrawMesh,
);

struct SetBWStaticUniformBindGroup<const I: usize>;

impl<const I: usize> RenderCommand<Opaque3d> for SetBWStaticUniformBindGroup<I> {
    type Param = SRes<BWStaticBindGroup>;

    type ViewQuery = ();

    type ItemQuery = Read<DynamicUniformIndex<BWStaticUniform>>;

    fn render<'w>(
        _item: &Opaque3d,
        _view: ROQueryItem<'w, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, Self::ItemQuery>>,
        param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(binding) = entity else {
            return RenderCommandResult::Failure;
        };

        pass.set_bind_group(I, &param.into_inner().bind_group, &[binding.index()]);
        RenderCommandResult::Success
    }
}

fn queue_phase_item(
    draw_functions: Res<DrawFunctions<Opaque3d>>,
    mut pipelines: ResMut<SpecializedMeshPipelines<BWStaticPipeline>>,
    specialize_pipeline: Res<BWStaticPipeline>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<Mesh>>,
    mesh_handles: Query<(Entity, &Handle<Mesh>), With<BWStaticUniform>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Opaque3d>)>,
    msaa: Res<Msaa>,
) {
    let draw_function = draw_functions.read().get_id::<DrawBWStatic>().unwrap();
    for (_view, mut render_phase) in views.iter_mut() {
        for (entity, mesh_handle) in mesh_handles.iter() {
            if let Some(mesh) = meshes.get(mesh_handle) {
                let key = MeshPipelineKey::from_msaa_samples(msaa.samples())
                    | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);

                let pipeline = pipelines
                    .specialize(&pipeline_cache, &specialize_pipeline, key, &mesh.layout)
                    .expect("Failed to specialize.");

                render_phase.add(Opaque3d {
                    pipeline,
                    entity,
                    draw_function,
                    batch_range: 0..0,
                    dynamic_offset: None,
                    asset_id: mesh_handle.into(),
                });
            }
        }
    }
}

#[derive(Resource)]
struct BWStaticBindGroup {
    bind_group: BindGroup,
}

fn queue_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline: Res<BWStaticPipeline>,
    uniforms: Res<ComponentUniforms<BWStaticUniform>>,
) {
    if let Some(buffer_binding) = uniforms.binding() {
        let bind_group = device.create_bind_group(
            Some("bw_static_bind_group"),
            &pipeline.effect_layout,
            &[BindGroupEntry {
                binding: 0,
                resource: buffer_binding,
            }],
        );

        commands.insert_resource(BWStaticBindGroup { bind_group });
    }
}
