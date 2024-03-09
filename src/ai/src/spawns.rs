use bevy::prelude::*;
use grin_render::sketched::SketchMaterial;

pub trait Enemy {
    pub const faction: Faction,
}

pub enum Faction {
    Punk,
}

pub struct SpawnPlugin {

}

#[derive(Event)]
pub struct SpawnEvent {
    pub location: Vec2,
    pub duration: f32,
    pub entity: Entity,
    pub faction: Faction,
}

#[derive(Resource)]
pub struct MapInfo {
    pub main_tile: Vec3,
    pub size: Vec2,
}

impl MapInfo {
    pub fn surface_y(&self) -> f32 {
        self.main_tile.y
    }

    pub fn locate_planar(&self, coord: Vec2) -> Vec3 {
        assert!(coord.x >= -1.0 && coord.x <= 1.0);
        assert!(coord.y >= -1.0 && coord.y <= 1.0);
        self.main_tile + Vec3::new(coord.x * self.size.x / 2., 0.0, coord.y * self.size.y / 2.)
    }
}

#[derive(Component)]
pub struct FadeShade {
    pub t: f32,
}

pub fn spawn_faction(mut commands: Commands, mut spawn_events: EventReader<SpawnEvent>, map: Res<MapInfo>, mut material_query: Query<&mut Handle<SketchMaterial>>) {
    for e in spawn_events.read() {
        match e.faction {
            Faction::Punk => {

            }
        }
    }
}
