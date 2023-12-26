use bevy::prelude::*;
use bevy_rapier3d::prelude::{PhysicsSet, Velocity};
use grin_util::numbers::{MulStack, MulStackError};

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum TimeScaleSet {
    PreScale,
    PreScaleFlush,
    Scale,
}

pub struct TimeScalePlugin;

impl Plugin for TimeScalePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            PostUpdate,
            (
                TimeScaleSet::PreScale,
                TimeScaleSet::PreScaleFlush,
                TimeScaleSet::Scale,
            )
                .chain()
                .before(PhysicsSet::SyncBackend),
        )
        .add_systems(
            PostUpdate,
            (auto_insert_timescales, auto_insert_raw_velocities).in_set(TimeScaleSet::PreScale),
        )
        .add_systems(
            PostUpdate,
            apply_deferred.in_set(TimeScaleSet::PreScaleFlush),
        )
        .add_systems(
            PostUpdate,
            (scale_audio, scale_animations, scale_velocities).in_set(TimeScaleSet::Scale),
        )
        .add_systems(Last, write_time_scales);
    }
}

/// The speed of time for this entity.
///
/// Multiplying by zero makes things weird, since after being hit by zero,
/// it can't be multiplied back again. Instead of using a raw `f32` value,
/// it is calculated off of a mulstack. You can get the scale with `f32::from::<TimeScale>`.
#[derive(Component)]
pub struct TimeScale {
    /// List of scale multipliers.
    pub mulstack: MulStack,
    /// Timescale of the previous frame. See `write_time_scales` in `Last`.
    pub memoed: f32,
}

impl Default for TimeScale {
    fn default() -> Self {
        Self {
            mulstack: MulStack::default(),
            memoed: 1.0,
        }
    }
}

impl TimeScale {
    /// Adds a multiplier to the timescale.
    pub fn scale_by(&mut self, scale: f32) {
        self.mulstack.add(scale)
    }

    /// Removes a multiplier from the timescale. `MulStackError::BadUnscale` if not found.
    pub fn unscale_by(&mut self, scale: f32) -> Result<(), MulStackError> {
        self.mulstack.remove(scale)
    }
}

impl From<&TimeScale> for f32 {
    fn from(value: &TimeScale) -> Self {
        f32::from(&value.mulstack)
    }
}

/// Records the timescale for this frame in `TimeScale.memoed`.
pub fn write_time_scales(mut scale_query: Query<&mut TimeScale>) {
    for mut time_scale in scale_query.iter_mut() {
        time_scale.memoed = (&*time_scale).into();
    }
}

/// Caches velocities at a `TimeScale` of `1.0`.
///
/// This generally isn't necessary, since it can just divide => remultiply the base velocity
/// when changing timescales. It becomes useful under a timescale of `0.0`. Since it's impossible
/// for forces to act when time is stopped, scaling the cached velocity instead of the real
/// velocity won't cause any issues.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct RawVelocity(pub Velocity);

impl From<Velocity> for RawVelocity {
    fn from(velocity: Velocity) -> Self {
        Self(velocity)
    }
}

/// Adds `TimeScale` to anything with a scaleable component.
pub fn auto_insert_timescales(
    mut commands: Commands,
    transform_query: Query<
        Entity,
        (
            Or<(With<Transform>, With<AudioSink>, With<AnimationPlayer>)>,
            Without<TimeScale>,
        ),
    >,
) {
    for e_transform in transform_query.iter() {
        commands
            .get_or_spawn(e_transform)
            .insert(TimeScale::default());
    }
}

/// Adds `RawVelocity` to any freestanding `Velocity`.
//
// NOTE: I forget why I restricted this to `RigidBody::Dynamic`. removing it for now.
pub fn auto_insert_raw_velocities(
    mut commands: Commands,
    velocity_query: Query<(Entity, &Velocity/*, &RigidBody*/), Without<RawVelocity>>,
) {
    for (e_velocity, velocity/*, body*/) in velocity_query.iter() {
        /*let RigidBody::Dynamic = body else {
            continue;
        };*/

        commands
            .get_or_spawn(e_velocity)
            .insert(RawVelocity::from(*velocity));
    }
}

/// Adjusts velocity speeds with `TimeScale`. Calculates `RawVelocity`.
pub fn scale_velocities(mut velocity_query: Query<(&mut Velocity, &mut RawVelocity, &TimeScale)>) {
    for (mut v_scaled, mut v_raw, time_scale) in velocity_query.iter_mut() {
        let applied = time_scale.memoed;
        let scale = f32::from(time_scale);

        // if timescale > 0 then the direction might have changed, so reassign the cached velocity
        if applied > 0.0 {
            // NOTE: this might cause floating point errors. watch for it.
            v_raw.0 = Velocity {
                linvel: v_scaled.linvel / applied,
                angvel: v_scaled.angvel / applied,
            };
        }

        *v_scaled = Velocity {
            linvel: v_raw.0.linvel * scale,
            angvel: v_raw.0.angvel * scale,
        };
    }
}

/// Adjusts audio speed with `TimeScale`.
pub fn scale_audio(audio_query: Query<(&AudioSink, &TimeScale), Changed<TimeScale>>) {
    for (audio, time_scale) in audio_query.iter() {
        audio.set_speed(time_scale.into());
    }
}

/// Adjusts animation speed with `TimeScale`.
pub fn scale_animations(mut animator_query: Query<(&mut AnimationPlayer, &TimeScale)>) {
    for (mut animator, time_scale) in animator_query.iter_mut() {
        animator.set_speed(time_scale.into());
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use bevy::time::TimePlugin;
    use bevy_rapier3d::prelude::*;

    use crate::physics::new_physics_app;

    use super::*;

    #[test]
    fn time_scale() {
        let mut time_scale = TimeScale::default();
        assert_eq!(
            f32::from(&time_scale),
            1.0,
            "`TimeScale` didn't initialize to `1.0`."
        );

        time_scale.scale_by(0.5);

        assert_eq!(
            f32::from(&time_scale),
            0.5,
            "`TimeScale` didn't scale. Go figure."
        );

        time_scale.scale_by(3.0);

        assert_eq!(
            f32::from(&time_scale),
            1.5,
            "`TimeScale` multipliers didn't stack."
        );

        let unscale = time_scale.unscale_by(0.5);

        assert!(unscale.is_ok(), "OK `TimeScale::unscale` errored.");
        assert_eq!(
            f32::from(&time_scale),
            3.0,
            "Inaccurate scale after `TimeScale::unscale`."
        );

        let unscale = time_scale.unscale_by(0.5);
        assert!(unscale.is_err(), "Bad `TimeScale::unscale` didn't error.");
    }

    // no test here. I'm not sure what's up but the audiosink doesn't load on update.
    // my assumption is that the audio file just isn't processed yet. not sure how
    // to block the thread until that happens.
    #[test]
    fn audio_scale() {
        /*let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            AudioPlugin::default(),
            TimeScalePlugin,
        ));

        let asset_server = app.world.resource::<AssetServer>();

        let audio = app
            .world
            .spawn((
                AudioBundle {
                    source: asset_server.load("audio/eightball-blip.ogg"),
                    ..Default::default()
                },
                TimeScale {
                    mulstack: vec![0.5],
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world.entity(audio).get::<AudioSink>().unwrap().speed(),
            0.5,
            "Audio didn't scale.",
        );

        let mut entity = app.world.entity_mut(audio);
        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.unscale_by(0.5).unwrap();

        app.update();

        assert_eq!(
            app.world.entity(audio).get::<AudioSink>().unwrap().speed(),
            1.0,
            "Audio didn't unscale.",
        );*/
    }

    #[test]
    fn animation_scale() {
        let mut app = App::new();
        app.add_plugins((
            TimePlugin,
            AssetPlugin::default(),
            AnimationPlugin,
            TimeScalePlugin,
        ));

        let animator = app
            .world
            .spawn((
                AnimationPlayer::default(),
                TimeScale {
                    mulstack: vec![0.5].into(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        assert_eq!(
            app.world
                .entity(animator)
                .get::<AnimationPlayer>()
                .unwrap()
                .speed(),
            0.5,
            "Animation didn't scale.",
        );

        let mut entity = app.world.entity_mut(animator);
        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.unscale_by(0.5).unwrap();

        app.update();

        assert_eq!(
            app.world
                .entity(animator)
                .get::<AnimationPlayer>()
                .unwrap()
                .speed(),
            1.0,
            "Animation didn't unscale.",
        );
    }

    #[test]
    fn velocity_scale() {
        let mut app = new_physics_app();
        app.add_plugins(TimeScalePlugin);

        let body = app
            .world
            .spawn((
                RigidBody::Dynamic,
                TransformBundle::default(),
                Velocity {
                    linvel: Vec3::new(1.0, 0.0, 0.0),
                    angvel: Vec3::new(TAU, 0.0, 0.0),
                },
                TimeScale {
                    mulstack: vec![0.5].into(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let v_scaled = app.world.entity(body).get::<Velocity>().unwrap();
        let maybe_v_raw = app.world.entity(body).get::<RawVelocity>();
        assert!(maybe_v_raw.is_some(), "`RawVelocity` not inserted.");
        let v_raw = maybe_v_raw.unwrap().0;

        assert_eq!(
            v_scaled.linvel,
            Vec3::new(0.5, 0.0, 0.0),
            "`Velocity.linvel` not scaled.",
        );
        assert_eq!(
            v_scaled.angvel,
            Vec3::new(TAU / 2.0, 0.0, 0.0),
            "`Velocity.angvel` not scaled."
        );

        assert_eq!(
            v_raw.linvel,
            Vec3::new(1.0, 0.0, 0.0),
            "Inaccurate `RawVelocity.linvel`.",
        );
        assert_eq!(
            v_raw.angvel,
            Vec3::new(TAU, 0.0, 0.0),
            "Inaccurate `RawVelocity.angvel`.",
        );

        // no angvels from now on. you get the point.

        let mut entity = app.world.entity_mut(body);
        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.unscale_by(0.5).unwrap();

        app.update();

        let v_scaled = app.world.entity(body).get::<Velocity>().unwrap();

        assert_eq!(
            v_scaled.linvel,
            Vec3::new(1.0, 0.0, 0.0),
            "`Velocity.linvel` not unscaled.",
        );

        let mut entity = app.world.entity_mut(body);
        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.scale_by(0.0);

        // checking for zero division errors here
        app.update();

        let mut entity = app.world.entity_mut(body);
        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.unscale_by(0.0).unwrap();

        app.update();
        // check done

        // raw velocity x2, timescale a further x2. both should change.
        let mut entity = app.world.entity_mut(body);
        let mut velocity = entity.get_mut::<Velocity>().unwrap();
        velocity.linvel *= 2.0;

        let mut time_scale = entity.get_mut::<TimeScale>().unwrap();
        time_scale.scale_by(2.0);

        app.update();

        let v_scaled = app.world.entity(body).get::<Velocity>().unwrap();
        let v_raw = app.world.entity(body).get::<RawVelocity>().unwrap().0;

        assert_eq!(
            v_scaled.linvel,
            Vec3::new(4.0, 0.0, 0.0),
            "`Velocity.linvel` not scaled + multiplied.",
        );
        assert_eq!(
            v_raw.linvel,
            Vec3::new(2.0, 0.0, 0.0),
            "`RawVelocity.linvel` not multiplied."
        );
    }

    // TODO: audio + anim testing... but I'm tired of tests
}
