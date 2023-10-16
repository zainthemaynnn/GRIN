pub mod beam;
pub mod blaze;
pub mod bwstatic;
pub mod duoquad;
pub mod gopro;
pub mod particles;
pub mod sketched;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_hanabi::HanabiPlugin;
use bevy_mod_outline::{OutlineBundle, OutlineVolume};
use bevy_tweening::TweeningPlugin;

use self::{
    beam::BeamPlugin,
    blaze::BlazePlugin,
    bwstatic::BWStaticPlugin,
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
                        ..Default::default()
                    },
                    mini: OutlineBundle {
                        outline: OutlineVolume {
                            colour: Color::BLACK,
                            width: 4.0,
                            visible: true,
                        },
                        ..Default::default()
                    },
                },
                autofill_sketch_effect: true,
            })
            .add(BWStaticPlugin)
            .add(GoProPlugin)
            .add(DuoQuadPlugin)
            .add(BeamPlugin)
            .add(BlazePlugin)
    }
}
