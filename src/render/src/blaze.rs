use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use grin_util::distr;

use super::particles::SetVelocityModifier;

pub struct BlazePlugin;

impl Plugin for BlazePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_blaze_effect_assets)
            .add_systems(PostUpdate, spawn_blaze_particles);
    }
}

/// A bunch of particle effects in a circular area with expiration time.
// TODO: collision detection
#[derive(Component, Default)]
pub struct Blaze {
    /// Particle effect asset.
    pub effect: Handle<EffectAsset>,
    /// Effect radius.
    pub radius: f32,
}

impl Blaze {
    pub const RADIUS_PROPERTY_TAG: &'static str = "radius";
}

#[derive(Resource)]
pub struct BlazeParticles {
    pub fire: Handle<EffectAsset>,
    pub ring_wave: Handle<EffectAsset>,
}

pub fn create_blaze_effect_assets(mut commands: Commands, mut assets: ResMut<Assets<EffectAsset>>) {
    let fire = {
        let w = ExprWriter::new();
        let radius_prop = w.add_property(Blaze::RADIUS_PROPERTY_TAG, 1.into());

        let pos = SetPositionCircleModifier {
            center: w.lit(Vec3::ZERO).expr(),
            axis: w.lit(Vec3::Y).expr(),
            radius: w.prop(radius_prop).expr(),
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

        let effect = EffectAsset::new(vec![2048], Spawner::rate(16.0.into()), w.finish())
            .with_name("blaze_particle")
            .init(pos)
            .init(vel)
            .init(age)
            .init(lifetime)
            .render(color)
            .render(size)
            .render(OrientModifier::default());

        assets.add(effect)
    };

    let ring_wave = {
        let w = ExprWriter::new();
        let radius_prop = w.add_property(Blaze::RADIUS_PROPERTY_TAG, 1.into());

        let pos = SetPositionCircleModifier {
            center: w.lit(Vec3::ZERO).expr(),
            axis: w.lit(Vec3::Y).expr(),
            radius: w.prop(radius_prop).expr(),
            dimension: ShapeDimension::Volume,
        };

        let vel = SetVelocitySphereModifier {
            speed: w.lit(1.0).uniform(w.lit(1.5)).expr(),
            // TODO: REPLACE 1.0 WITH RADIUS_PROPERTY_TAG SOMEHOW
            center: w
                .lit(Vec3::new(0.0, -1.0 / 15.0_f32.to_radians().tan(), 0.0))
                .expr(),
        };

        let lifetime = SetAttributeModifier::new(Attribute::LIFETIME, w.lit(0.5).expr());

        let size = SizeOverLifetimeModifier {
            gradient: distr::f32_map(10, &distr::linear, &distr::quad).fold(
                Gradient::new(),
                |mut grad, (i, t)| {
                    grad.add_key(i, Vec2::splat((1.0 - t) * 0.2));
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

        let drag = LinearDragModifier::new(w.lit(5.0).expr());

        let effect = EffectAsset::new(vec![512], Spawner::once(128.0.into(), false), w.finish())
            .with_name("ring_wave_particle")
            .init(pos)
            .init(vel)
            .init(lifetime)
            .update(drag)
            .render(color)
            .render(size)
            .render(OrientModifier::default());

        assets.add(effect)
    };

    commands.insert_resource(BlazeParticles { fire, ring_wave });
}

pub fn spawn_blaze_particles(
    mut commands: Commands,
    assets: Res<BlazeParticles>,
    blaze_query: Query<(Entity, &Blaze), Added<Blaze>>,
) {
    for (e_blaze, blaze) in blaze_query.iter() {
        let effect = ParticleEffect::new(assets.fire.clone());
        let effect_properties = {
            let mut props = EffectProperties::default();
            props.set(Blaze::RADIUS_PROPERTY_TAG, blaze.radius.into());
            props
        };

        commands.entity(e_blaze).insert(ParticleEffectBundle {
            effect,
            effect_properties,
            ..Default::default()
        });
    }
}
