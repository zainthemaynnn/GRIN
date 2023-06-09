pub mod bwstatic;
pub mod duoquad;
pub mod gopro;
pub mod sketched;

use bevy::{app::PluginGroupBuilder, prelude::*};
use bevy_mod_outline::{OutlineBundle, OutlineVolume};

use self::{
    bwstatic::BWStaticPlugin,
    sketched::{GlobalMeshOutline, SketchEffectPlugin}, gopro::GoProPlugin, duoquad::DuoQuadPlugin,
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
            .add(SketchEffectPlugin {
                outline: GlobalMeshOutline {
                    standard: OutlineBundle {
                        outline: OutlineVolume {
                            colour: Color::BLACK,
                            width: 8.0,
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
    }
}
