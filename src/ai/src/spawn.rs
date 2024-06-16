use std::{marker::PhantomData, time::Duration};

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_asset_loader::prelude::*;
use bevy_tweening::AnimationSystem;
use grin_asset::AssetLoadState;
use grin_physics::PhysicsTime;

/// Enemy spawning systems.
///
/// All states occur before `AnimationUpdate`.
#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Copy, Clone)]
pub enum SpawnSet {
    /// Enemy spawn events are consumed, within the `PreUpdate` schedule.
    Spawn,
    /// Enemy spawner timers are initialized.
    TimerSet,
    /// Enemy spawner timers are ticked.
    TimerTick,
    /// Enemy spawner stages are transitioned, if applicable.
    Transition,
    /// Occurs after `Transition` stage.
    PostTransition,
}

pub struct MasterSpawnPlugin;

impl Plugin for MasterSpawnPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                SpawnSet::TimerSet,
                SpawnSet::TimerTick,
                SpawnSet::Transition,
                SpawnSet::PostTransition,
                AnimationSystem::AnimationUpdate,
            )
                .chain(),
        )
        .add_systems(Update, tick_spawn_indicators.in_set(SpawnSet::TimerTick));
    }
}

pub struct EnemySpawnPlugin<T: Component> {
    pub phantom_data: PhantomData<T>,
}

impl<T: Component> Default for EnemySpawnPlugin<T> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
        }
    }
}

impl<T: Component> Plugin for EnemySpawnPlugin<T> {
    fn build(&self, app: &mut App) {
        app.add_event::<EnemySpawn<T>>()
            .add_event::<SpawnBegan<T>>()
            .add_event::<SpawnStageReached<T>>()
            .add_event::<SpawnCompleted<T>>()
            .configure_loading_state(
                LoadingStateConfig::new(AssetLoadState::Loading)
                    .load_collection::<indicators::SpawnIndicatorAssets>(),
            )
            .add_systems(PreUpdate, init_spawn_events::<T>.in_set(SpawnSet::Spawn))
            .add_systems(
                Update,
                (
                    reinitialize_spawn_timers::<T>.in_set(SpawnSet::TimerSet),
                    transition_spawn_states::<T>.in_set(SpawnSet::Transition),
                    indicators::transition_indicator_states::<T>
                        .in_set(SpawnSet::PostTransition)
                        .run_if(in_state(AssetLoadState::Success)),
                ),
            );
    }
}

#[derive(Component, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum SpawnStage {
    /// Hitbox doesn't exist yet.
    #[default]
    Indicate,
    /// Hitbox exists, and will deal damage if the player is here.
    /// However, the AI is not yet active.
    Materialize,
    /// Spawning is complete, activating the AI and applying knockback.
    Finish,
}

impl SpawnStage {
    /// Provides the next `SpawnStage`.
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Indicate => Some(Self::Materialize),
            Self::Materialize => Some(Self::Finish),
            Self::Finish => None,
        }
    }

    /// Provides the duration of this `SpawnStage`.
    pub fn duration(&self) -> Duration {
        match self {
            Self::Indicate => Duration::from_millis(3000),
            Self::Materialize => Duration::from_millis(1000),
            Self::Finish => Duration::ZERO,
        }
    }
}

#[derive(Component, Clone, Debug, Default)]
pub struct Spawning {
    pub t: Timer,
}

#[derive(Bundle, Default)]
pub struct SpawningBundle {
    pub stage: SpawnStage,
    pub spawner: Spawning,
}

#[derive(Event, Clone)]
pub struct EnemySpawn<T> {
    pub transform: Transform,
    pub phantom_data: PhantomData<T>,
}

impl<T> Default for EnemySpawn<T> {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            phantom_data: PhantomData,
        }
    }
}

#[derive(Event)]
pub struct SpawnBegan<T> {
    pub entity: Entity,
    pub phantom_data: PhantomData<T>,
}

#[derive(Event)]
pub struct SpawnStageReached<T> {
    pub entity: Entity,
    pub stage: SpawnStage,
    pub phantom_data: PhantomData<T>,
}

#[derive(Event, Clone)]
pub struct SpawnCompleted<T> {
    pub entity: Entity,
    pub phantom_data: PhantomData<T>,
}

pub fn init_spawn_events<T: Component>(
    mut commands: Commands,
    mut spawn_events: EventReader<SpawnBegan<T>>,
    mut state_events: EventWriter<SpawnStageReached<T>>,
) {
    for SpawnBegan { entity, .. } in spawn_events.read() {
        state_events.send(SpawnStageReached {
            entity: *entity,
            stage: SpawnStage::Indicate,
            phantom_data: PhantomData,
        });

        commands.entity(*entity).insert(SpawningBundle::default());
    }
}

pub fn tick_spawn_indicators(time: Res<PhysicsTime>, mut spawn_query: Query<&mut Spawning>) {
    for mut spawn in spawn_query.iter_mut() {
        spawn.t.tick(time.0.delta());
    }
}

pub fn transition_spawn_states<T: Component>(
    mut commands: Commands,
    spawn_query: Query<(Entity, &Spawning, &SpawnStage), With<T>>,
    mut state_events: EventWriter<SpawnStageReached<T>>,
    mut completed_events: EventWriter<SpawnCompleted<T>>,
) {
    for (e_spawn, spawn, stage) in spawn_query.iter() {
        if spawn.t.just_finished() {
            if let Some(new_stage) = stage.next() {
                debug!(msg="Spawn stage reached", e_spawn=?e_spawn, stage=?new_stage);
                state_events.send(SpawnStageReached {
                    entity: e_spawn,
                    stage: new_stage,
                    phantom_data: PhantomData,
                });
            } else {
                info!(msg="Spawn complete", e_spawn=?e_spawn);
                commands.entity(e_spawn).remove::<(Spawning, SpawnStage)>();
                completed_events.send(SpawnCompleted {
                    entity: e_spawn,
                    phantom_data: PhantomData,
                });
            }
        }
    }
}

pub fn reinitialize_spawn_timers<T: Component>(
    mut state_events: EventReader<SpawnStageReached<T>>,
    mut spawn_query: Query<(&mut Spawning, &mut SpawnStage), With<T>>,
) {
    for SpawnStageReached { entity, stage, .. } in state_events.read() {
        let Ok((mut spawning, mut stage_ref)) = spawn_query.get_mut(*entity) else {
            continue;
        };

        spawning.t.set_duration(stage.duration());
        spawning.t.reset();
        *stage_ref = *stage;

        trace!(
            msg="Initializing spawn stage timer",
            e_spawn=?entity,
            duration=?spawning.t.duration(),
        );
    }
}

// this module is a bit ugly. but really, what is life if not ugly? AMEN (REAL...)
pub mod indicators {
    use bevy::{ecs::system::EntityCommands, prelude::*};
    use bevy_asset_loader::prelude::*;
    use bevy_tweening::{Animator, EaseFunction, EaseMethod, Tween};
    use grin_damage::hitbox::{GltfHitboxAutoGenTarget, HitboxManager, Hitboxes, Hurtboxes};
    use grin_render::{
        fill::{FillCompletedEvent, FillEffect, FillParamLens},
        tint::{TintCompletedEvent, TintEffect, TintEmissiveLens},
        EffectFlags, TweenCompletedEvent,
    };
    use grin_rig::Idle;

    use super::{SpawnStage, SpawnStageReached};

    #[derive(Resource, AssetCollection)]
    pub struct SpawnIndicatorAssets {
        #[asset(key = "anim.rock")]
        pub rock_animation: Handle<AnimationClip>,
    }

    #[derive(Component, Copy, Clone, Debug, Default)]
    pub enum SpawnIndicatorEffect {
        #[default]
        Neon,
    }

    pub const NEON_EMISSIVE_SCALE: f32 = 2000.0;

    pub fn neon_effect(
        stage: &SpawnStage,
        commands: &mut EntityCommands,
        assets: &SpawnIndicatorAssets,
    ) {
        match stage {
            SpawnStage::Indicate => {
                commands.insert((
                    // pose
                    Idle {
                        clip: assets.rock_animation.clone(),
                    },
                    // fill
                    FillEffect::default(),
                    Animator::new(
                        Tween::new(
                            EaseMethod::Linear,
                            stage.duration().clone(),
                            FillParamLens {
                                start: 0.0,
                                end: 1.0,
                            },
                        )
                        .with_completed_event(FillCompletedEvent::EVENT_ID),
                    ),
                    // tint
                    TintEffect {
                        flags: EffectFlags::empty(),
                        ..Default::default()
                    },
                    Animator::new(
                        Tween::new(
                            EaseMethod::Linear,
                            stage.duration().clone(),
                            TintEmissiveLens {
                                start: Color::PINK.as_rgba_linear() * NEON_EMISSIVE_SCALE,
                                end: Color::PURPLE.as_rgba_linear() * NEON_EMISSIVE_SCALE,
                            },
                        )
                        .with_completed_event(TintCompletedEvent::EVENT_ID),
                    ),
                ));
            }
            SpawnStage::Materialize => {
                commands.insert((
                    // hitbox gen
                    HitboxManager::<Hitboxes>::default(),
                    HitboxManager::<Hurtboxes>::default(),
                    GltfHitboxAutoGenTarget::Here,
                    // de-tint
                    TintEffect {
                        flags: EffectFlags::DESPAWN | EffectFlags::REZERO,
                        ..Default::default()
                    },
                    Animator::new(
                        Tween::new(
                            EaseFunction::ExponentialOut,
                            stage.duration().clone(),
                            TintEmissiveLens {
                                start: Color::PURPLE.as_rgba_linear() * NEON_EMISSIVE_SCALE,
                                end: Color::BLACK.as_rgba_linear(),
                            },
                        )
                        .with_completed_event(TintCompletedEvent::EVENT_ID),
                    ),
                ));
            }
            SpawnStage::Finish => (),
        }
    }

    pub fn transition_indicator_states<T: Component>(
        mut commands: Commands,
        assets: Res<SpawnIndicatorAssets>,
        effect_query: Query<&SpawnIndicatorEffect>,
        mut events: EventReader<SpawnStageReached<T>>,
    ) {
        for SpawnStageReached { entity, stage, .. } in events.read() {
            let effect = effect_query.get(*entity).copied().unwrap_or_default();
            match effect {
                SpawnIndicatorEffect::Neon => {
                    neon_effect(stage, &mut commands.entity(*entity), &assets)
                }
            }
        }
    }
}

#[derive(SystemParam)]
pub struct EnemySpawnerParams<'w, 's, T: Component> {
    pub commands: Commands<'w, 's>,
    pub indicator_events: EventWriter<'w, SpawnBegan<T>>,
}

/// Wrapper for a system returning a `Bundle`, which describes the initial enemy properties *before*
/// the spawn indicator. This bundle will be used as a template when responding to `EnemySpawn` events,
/// using the spawned entity to convert the `SpawnEvent` into a `SpawnBegan` event. A  `TransformBundle`
/// corresponding to the event is attached to the entity.
///
/// This is generally used to:
/// - Set the `EnemyIdentifier`
/// - Set the `SpawnIndicatorEffect`
/// - Set the rig
///
/// This bundle is independent from `SpawnIndicatorEffect` and will be merged with components provided
/// by the indicator systems.
///
/// Note: `Events<EnemySpawn<I>>` cannot be used as a system param of `spawn_fn`.
pub fn enemy_spawner<T, B, F, Marker>(
    mut spawn_fn: F,
) -> impl FnMut(In<F::In>, EventReader<EnemySpawn<T>>, ParamSet<(F::Param, EnemySpawnerParams<T>)>) -> ()
where
    T: Component,
    B: Bundle,
    F: SystemParamFunction<Marker, In = (), Out = B>,
{
    move |In(spawn_fn_in), mut spawn_events, mut params| {
        for EnemySpawn { transform, .. } in spawn_events.read() {
            let bundle = spawn_fn.run(spawn_fn_in, params.p0());

            let EnemySpawnerParams {
                mut commands,
                mut indicator_events,
            } = params.p1();

            let e_agent = commands
                .spawn(bundle)
                .insert(TransformBundle::from_transform(*transform))
                .id();

            indicator_events.send(SpawnBegan {
                entity: e_agent,
                phantom_data: PhantomData,
            });

            info!("Spawning agent {:?}", e_agent);
        }
    }
}

#[derive(SystemParam)]
pub struct AiSpawnerParams<'w, 's> {
    pub commands: Commands<'w, 's>,
}

/// Wrapper for a system returning a `Bundle`, which describes the final enemy properties *after*
/// the spawn indicator. This bundle will be used as a template when responding to `SpawnCompleted` events.
///
/// Note: `Events<SpawnCompleted<I>>` cannot be used as a system param of `spawn_fn`.
pub fn ai_spawner<T, B, F, Marker>(
    mut spawn_fn: F,
) -> impl FnMut(In<F::In>, EventReader<SpawnCompleted<T>>, ParamSet<(F::Param, AiSpawnerParams)>) -> ()
where
    T: Component,
    B: Bundle,
    F: SystemParamFunction<Marker, In = (), Out = B>,
{
    move |In(spawn_fn_in), mut spawn_events, mut params| {
        for SpawnCompleted {
            entity: e_agent, ..
        } in spawn_events.read()
        {
            let bundle = spawn_fn.run(spawn_fn_in, params.p0());

            let AiSpawnerParams { mut commands } = params.p1();

            commands.entity(*e_agent).insert(bundle);

            info!("Activating agent {:?}", *e_agent);
        }
    }
}
