use bevy::prelude::*;
use bevy_mod_outline::OutlineBundle;

#[derive(Resource)]
pub struct GlobalMeshOutline {
    pub bundle: OutlineBundle,
}

pub struct GlobalMeshOutlinePlugin {
    pub bundle: OutlineBundle,
}

impl Plugin for GlobalMeshOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalMeshOutline { bundle: self.bundle.clone() });
    }
}
