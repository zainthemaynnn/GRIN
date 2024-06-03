use bevy::{prelude::*, render::primitives::Aabb, scene::SceneInstance};

pub struct SpatialPlugin;

impl Plugin for SpatialPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, compute_scene_aabbs);
    }
}

/// Computes a `SceneAabb` for this scene.
///
/// This is shamelessly borrowed from
/// [`dmlary`](https://gist.github.com/dmlary/b9e1e9ef18789dfb0e6df8aca2f1ed74).
/// Why write it yourself when it's already out there, eh? Thanks.
#[derive(Component, Debug, Reflect)]
pub struct SceneAabb {
    pub min: Vec3,
    pub max: Vec3,
}

/// A new `SceneAabb` will be computed for this scene.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ComputeSceneAabb;

impl SceneAabb {
    fn new(center: Vec3) -> Self {
        Self {
            min: center,
            max: center,
        }
    }

    /// merge an child AABB into the Scene AABB
    fn merge_aabb(&mut self, aabb: &Aabb, global_transform: &GlobalTransform) {
        /*
              (2)-----(3)               Y
               | \     | \              |
               |  (1)-----(0) MAX       o---X
               |   |   |   |             \
          MIN (6)--|--(7)  |              Z
                 \ |     \ |
                  (5)-----(4)
        */
        // ^ these are getting too impressive. is every programmer good as ascii art except me?

        let min = aabb.min();
        let max = aabb.max();
        let corners = [
            global_transform.transform_point(Vec3::new(max.x, max.y, max.z)),
            global_transform.transform_point(Vec3::new(min.x, max.y, max.z)),
            global_transform.transform_point(Vec3::new(min.x, max.y, min.z)),
            global_transform.transform_point(Vec3::new(max.x, max.y, min.z)),
            global_transform.transform_point(Vec3::new(max.x, min.y, max.z)),
            global_transform.transform_point(Vec3::new(min.x, min.y, max.z)),
            global_transform.transform_point(Vec3::new(min.x, min.y, min.z)),
            global_transform.transform_point(Vec3::new(max.x, min.y, min.z)),
        ];

        for corner in corners {
            let gt = corner.cmpgt(self.max);
            let lt = corner.cmplt(self.min);

            trace!(corner=?corner, lt=?lt, gt=?gt);

            if gt.x {
                self.max.x = corner.x;
            } else if lt.x {
                self.min.x = corner.x;
            }

            if gt.y {
                self.max.y = corner.y;
            } else if lt.y {
                self.min.y = corner.y;
            }

            if gt.z {
                self.max.z = corner.z;
            } else if lt.z {
                self.min.z = corner.z;
            }
        }

        trace!(min=?min, max=?max);
    }
}

pub fn compute_scene_aabbs(
    mut commands: Commands,
    scene_manager: Res<SceneSpawner>,
    scene_instances: Query<(Entity, &SceneInstance, &GlobalTransform), With<ComputeSceneAabb>>,
    bounding_boxes: Query<(&Aabb, &GlobalTransform)>,
) {
    for (entity, instance, global_transform) in scene_instances.iter() {
        if !scene_manager.instance_is_ready(**instance) {
            continue;
        }

        let mut scene_aabb = SceneAabb::new(global_transform.translation());

        for e_node in scene_manager.iter_instance_entities(**instance) {
            let Ok((bb, transform)) = bounding_boxes.get(e_node) else {
                continue;
            };
            scene_aabb.merge_aabb(bb, transform);
        }

        debug!(
            msg="Generated scene AABB.",
            e_scene=?entity,
            aabb=?scene_aabb,
        );

        commands
            .entity(entity)
            .insert(scene_aabb)
            .remove::<ComputeSceneAabb>();
    }
}
