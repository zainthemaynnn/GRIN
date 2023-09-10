use bevy::prelude::*;
use bevy_hanabi::prelude::*;

use crate::{physics::PhysicsTime, util::distr};

use super::particles::SetVelocityModifier;

pub struct BlazePlugin;

impl Plugin for BlazePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_blaze_effect_assets)
            .add_systems(PostUpdate, (spawn_blaze_particles, tick_blaze_particles));
    }
}

/// A bunch of particle effects in a circular area with expiration time.
// TODO: collision detection
#[derive(Component, Default)]
pub struct Blaze {
    /// Effect timer. The timer is automatically updated and the entity is despawned when finished.
    pub timer: Timer,
    /// Particle effect asset.
    pub effect: Handle<EffectAsset>,
    /// Spawner to use for this effect.
    ///
    /// Note: when the particle is added, this is removed by `Option::take`.
    /// Don't trust this value to exist.
    pub spawner: Option<Spawner>,
    /// Effect radius.
    pub radius: f32,
}

impl Blaze {
    pub const RADIUS_PROPERTY_TAG: &'static str = "radius";
}

#[derive(Resource)]
pub struct BlazeParticles {
    pub fire: Handle<EffectAsset>,
}

pub fn create_blaze_effect_assets(mut commands: Commands, mut assets: ResMut<Assets<EffectAsset>>) {
    let w = ExprWriter::new();

    let pos = SetPositionCircleModifier {
        center: w.lit(Vec3::ZERO).expr(),
        axis: w.lit(Vec3::Y).expr(),
        radius: w.prop(Blaze::RADIUS_PROPERTY_TAG).expr(),
        dimension: ShapeDimension::Volume,
    };

    let vel = SetVelocityModifier {
        direction: w.lit(Vec3::Y).expr(),
        speed: w.lit(1.0).uniform(w.lit(1.5)).expr(),
    };

    let age = SetAttributeModifier::new(Attribute::AGE, w.lit(0.0).uniform(w.lit(0.2)).expr());

    let lifetime =
        SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.8).uniform(w.lit(1.2)).expr());

    let size = SizeOverLifetimeModifier {
        gradient: distr::f32_map(10, &distr::linear, &distr::quad).fold(
            Gradient::new(),
            |mut grad, (i, t)| {
                grad.add_key(i, Vec2::splat(1.0 - t));
                grad
            },
        ),
        screen_space_size: false,
    };

    let color = ColorOverLifetimeModifier {
        gradient: distr::f32_map(10, &distr::linear, &distr::quad).fold(
            Gradient::new(),
            |mut grad, (i, t)| {
                grad.add_key(
                    i,
                    Vec4::new(1.0, 1.0, 0.0, 1.0).lerp(Vec4::new(1.0, 0.0, 0.0, 1.0), t),
                );
                grad
            },
        ),
    };

    let effect = EffectAsset::new(2048, Spawner::rate(16.0.into()), w.finish())
        .with_name("blaze_particle")
        .init(pos)
        .init(vel)
        .init(age)
        .init(lifetime)
        .render(color)
        .render(size)
        .render(BillboardModifier);

    commands.insert_resource(BlazeParticles {
        fire: assets.add(effect),
    });
}

pub fn spawn_blaze_particles(
    mut commands: Commands,
    assets: Res<BlazeParticles>,
    mut blaze_query: Query<(Entity, &mut Blaze), Added<Blaze>>,
) {
    for (e_blaze, mut blaze) in blaze_query.iter_mut() {
        // what the heck is this generic for? was it an oversight? :P
        let mut effect = ParticleEffect::new(assets.fire.clone()).with_properties::<String>([(
            Blaze::RADIUS_PROPERTY_TAG.to_owned(),
            blaze.radius.into(),
        )]);
        if let Some(spawner) = blaze.spawner.take() {
            effect = effect.with_spawner(spawner);
        }
        commands
            .entity(e_blaze)
            .insert((effect, CompiledParticleEffect::default()));
    }
}

pub fn tick_blaze_particles(
    mut commands: Commands,
    time: Res<PhysicsTime>,
    mut blaze_query: Query<(Entity, &mut Blaze)>,
) {
    for (e_blaze, mut blaze) in blaze_query.iter_mut() {
        if blaze.timer.tick(time.0.delta()).just_finished() {
            commands.entity(e_blaze).despawn();
        }
    }
}
