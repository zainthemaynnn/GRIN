pub mod beam;
pub mod blaze;
pub mod bwstatic;
pub mod duoquad;
pub mod fill;
pub mod gopro;
pub mod particles;
pub mod sketched;
pub mod tint;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_hanabi::HanabiPlugin;
use bevy_mod_outline::{OutlineBundle, OutlineMode, OutlineVolume};
use bevy_tweening::{TweenCompleted, TweeningPlugin};
use bitflags::bitflags;
use fill::FillPlugin;
use tint::TintPlugin;

use self::{
    beam::BeamPlugin,
    blaze::BlazePlugin,
    //bwstatic::BWStaticPlugin,
    duoquad::DuoQuadPlugin,
    gopro::GoProPlugin,
    sketched::{GlobalMeshOutline, SketchEffectPlugin},
};

#[repr(u8)]
pub enum RenderLayer {
    STANDARD,
    AVATAR,
}

pub struct RenderFXPlugins;

impl PluginGroup for RenderFXPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(HanabiPlugin)
            .add(TweeningPlugin)
            .add(SketchEffectPlugin {
                outline: GlobalMeshOutline {
                    standard: OutlineBundle {
                        outline: OutlineVolume {
                            colour: Color::BLACK,
                            width: 6.0,
                            visible: true,
                        },
                        mode: OutlineMode::RealVertex,
                        ..Default::default()
                    },
                    mini: OutlineBundle {
                        outline: OutlineVolume {
                            colour: Color::BLACK,
                            width: 4.0,
                            visible: true,
                        },
                        mode: OutlineMode::RealVertex,
                        ..Default::default()
                    },
                },
                autofill_sketch_effect: true,
            })
            //.add(BWStaticPlugin)
            .add(FillPlugin)
            .add(TintPlugin)
            .add(GoProPlugin)
            .add(DuoQuadPlugin)
            .add(BeamPlugin)
            .add(BlazePlugin)
    }
}

pub trait TweenAppExt {
    fn add_tween_completion_event<E: TweenCompletedEvent>(&mut self) -> &mut Self;
}

impl TweenAppExt for App {
    fn add_tween_completion_event<E: TweenCompletedEvent>(&mut self) -> &mut Self {
        self.add_event::<E>()
            .add_systems(PreUpdate, map_tween_event_id::<E>)
    }
}

pub trait TweenCompletedEvent: Event + From<Entity> {
    /// This can be anything; just make it statically unique or face the consequences.
    const EVENT_ID: u64;
}

pub fn map_tween_event_id<E: TweenCompletedEvent>(
    mut untyped_events: EventReader<TweenCompleted>,
    mut typed_events: EventWriter<E>,
) {
    for TweenCompleted { entity, .. } in untyped_events
        .read()
        .filter(|ev| ev.user_data == E::EVENT_ID)
    {
        typed_events.send(E::from(*entity));
    }
}

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct EffectFlags: u8 {
        /// Effect component will despawn on completion.
        const DESPAWN = 1 << 0;
        /// If applicable, will "undo" the effect and reset the entity to its base state.
        const REZERO = 1 << 1;
    }
}

impl Default for EffectFlags {
    fn default() -> Self {
        Self::DESPAWN | Self::REZERO
    }
}
