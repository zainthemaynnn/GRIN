use std::{f32::consts::FRAC_PI_3, time::Duration};

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy_tweening::{
    lens::{TransformPositionLens, TransformScaleLens},
    Animator, EaseFunction, Tracks, Tween,
};
use grin_asset::AssetLoadState;
use grin_render::sketched::{SketchMaterial, OutlineScaleMode};
use grin_util::tween::TweenCompletedEvent;
use rand::prelude::*;
use rand_distr::UnitSphere;

pub struct ImpactPlugin;

impl Plugin for ImpactPlugin {
    fn build(&self, app: &mut App) {
        app.add_collection_to_loading_state::<_, ImpactAssets>(AssetLoadState::Loading)
            .add_systems(
                Update,
                init_impact.run_if(in_state(AssetLoadState::Success)),
            );
    }
}

#[derive(Resource, AssetCollection)]
pub struct ImpactAssets {
    #[asset(key = "mesh.sphere_100cm")]
    pub particle: Handle<Mesh>,
    #[asset(key = "mat.white_unlit")]
    pub white: Handle<SketchMaterial>,
}

/// Creates an "impact" effect.
///
/// This is actually a component instead of an event so that entity histories can cache the
/// effect description alone, and not every single particle, in order to save memory.
#[derive(Component, Default)]
pub struct Impact {
    pub particle_count: u32,
    pub particle_radius: f32,
    pub burst_radius: f32,
    pub duration: Duration,
    pub rng_seed: <StdRng as SeedableRng>::Seed,
}

impl Impact {
    pub fn from_burst_radius(burst_radius: f32) -> Self {
        let sphere_volume = 4.0 * FRAC_PI_3 * burst_radius.powi(3);
        Self {
            particle_count: (sphere_volume * 2.0) as u32,
            particle_radius: 0.2,
            burst_radius,
            duration: Duration::from_secs_f32(0.4),
            ..Default::default()
        }
    }
}

pub fn init_impact(
    mut commands: Commands,
    impact_assets: Res<ImpactAssets>,
    mut impact_query: Query<(Entity, &mut Impact, &GlobalTransform)>,
) {
    for (e_impact, mut impact, g_transform) in impact_query.iter_mut() {
        thread_rng().fill(&mut impact.rng_seed);
        let mut rng = StdRng::from_seed(impact.rng_seed);

        let material = impact_assets.white.clone();
        let mesh = impact_assets.particle.clone();
        let transform = g_transform
            .compute_transform()
            .with_scale(Vec3::splat(impact.particle_radius));
        let duration = impact.duration;
        let radius = impact.burst_radius;

        commands.spawn_batch((0..impact.particle_count).map(move |_| {
            let dir = Vec3::from_array(UnitSphere.sample(&mut rng));
            let tracks = Tracks::new([
                Tween::new(
                    EaseFunction::QuadraticOut,
                    duration,
                    TransformPositionLens {
                        start: transform.translation,
                        end: transform.translation + dir * radius * rng.gen_range(0.8..1.2),
                    },
                ),
                Tween::new(
                    EaseFunction::QuadraticIn,
                    duration,
                    TransformScaleLens {
                        start: transform.scale,
                        end: Vec3::ZERO,
                    },
                )
                .with_completed_event(TweenCompletedEvent::Despawn as u64),
            ]);
            (
                MaterialMeshBundle {
                    material: material.clone(),
                    mesh: mesh.clone(),
                    transform,
                    ..Default::default()
                },
                Velocity::linear(dir),
                Animator::new(tracks),
                OutlineScaleMode::Scale(16.0),
            )
        }));

        commands.entity(e_impact).despawn();
    }
}
