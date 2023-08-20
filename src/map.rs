use std::array::IntoIter;

use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use bevy_landmass::Archipelago;
use itertools::Itertools;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

use crate::{
    collider,
    util::vectors::{self, Vec3Ext},
};

#[derive(Default)]
pub struct MapPlugin {
    pub navmesh_debugging: bool,
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<MapLoadState>().add_systems(
            Update,
            (
                check_map_existence.run_if(in_state(MapLoadState::NotLoaded)),
                setup_map_navigation
                    .pipe(finish_navmesh_generation)
                    .run_if(in_state(MapLoadState::Loading)),
            ),
        );

        if self.navmesh_debugging {
            app.add_systems(Update, draw_navmesh.run_if(in_state(MapLoadState::Success)));
        }
    }
}

#[derive(Component)]
pub struct Map;

#[derive(Debug)]
pub struct NavMeshStatistics {
    pub hulls: usize,
    pub edges: usize,
    pub constraints: usize,
    pub verts: usize,
    pub raw_polys: usize,
    pub culled_polys: usize,
}

#[derive(Debug)]
pub enum NavMeshGenerationError {
    MapNotFound,
    NoPositionAttribute,
    BadPositionAttributeFormat(VertexAttributeValues),
    BadVertex(spade::InsertionError),
    // landmass validation error should be pub? I'll complain later.
    Validation,
}

pub fn check_map_existence(
    map_query: Query<Entity, (With<Map>, With<Children>)>,
    mut map_state: ResMut<NextState<MapLoadState>>,
) {
    if map_query.get_single().is_ok() {
        map_state.set(MapLoadState::Loading);
    }
}

// NOTE: I don't actually know if it's possible for this to run twice or not
// I need to check the logs on a more complex map
pub fn setup_map_navigation(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    map_query: Query<Entity, (With<Map>, With<Children>)>,
    mesh_query: Query<(&GlobalTransform, &Handle<Mesh>, &Name)>,
    children_query: Query<&Children>,
) -> Result<NavMeshStatistics, NavMeshGenerationError> {
    let e_map = map_query
        .get_single()
        .map_err(|_| NavMeshGenerationError::MapNotFound)?;

    let mut obstacles = Vec::default();

    for e_node in children_query.iter_descendants(e_map) {
        let Ok((g_transform, mesh, name)) = mesh_query.get(e_node) else {
            continue;
        };

        match name.as_str() {
            "Plane" => (),
            _ => {
                obstacles.push((*g_transform, mesh.clone()));
            }
        }

        commands.entity(e_node).insert(collider!(&meshes, mesh));
    }

    // https://skatgame.net/mburo/ps/thesis_demyen_2006.pdf
    // https://www.jdxdev.com/blog/2021/07/06/rts-pathfinding-2-dynamic-navmesh-with-constrained-delaunay-triangles/
    //
    // I would like to thank this guy. touched it up a little, but it is basically their's.
    // https://discord.com/channels/691052431525675048/1138102751444877333/1138136084895772682
    //
    // NOTE: it's important that none of the primitives in the scene are overlapping.
    // otherwise there's a good chance of overlapping triangulation constraints.
    // it is probably possible to merge these into the same hull or use a completely different approach.
    // however, this doesn't really make the map all that more difficult to make.
    // just need to keep it in mind.
    info!("Begin navmesh generation...");

    let mut triangulation = ConstrainedDelaunayTriangulation::<Point2<f32>>::new();
    let mut hulls = Vec::new();

    for (g_transform, h_mesh) in obstacles.iter() {
        let mesh = meshes.get(h_mesh).unwrap();

        let mut positions = match mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .ok_or(NavMeshGenerationError::NoPositionAttribute)?
        {
            VertexAttributeValues::Float32x3(positions) => Ok(positions
                .iter()
                // transforming to global XZ plane
                .map(|p| g_transform.transform_point(Vec3::from(*p)).xz_flat())
                .collect_vec()),
            // I do not think I am supposed to clone this
            // but lifetimes... bleh
            v => Err(NavMeshGenerationError::BadPositionAttributeFormat(
                v.clone(),
            )),
        }?;
        // since the y has been normalized, vertically aligned points are duped
        positions.sort_unstable_by(Vec3::lexographic_cmp);
        positions.dedup();

        let hull = vectors::convex_hull_2d(positions.as_slice());
        hulls.push(hull);
    }

    // using convex hull edges as constraints
    // it should be accurate enough
    for hull in hulls.iter() {
        for (p0, p1) in hull.iter().circular_tuple_windows() {
            let (from, to) = (Point2::new(p0.x, p0.z), Point2::new(p1.x, p1.z));
            triangulation
                .add_constraint_edge(from, to)
                .map_err(|e| NavMeshGenerationError::BadVertex(e))?;
        }
    }

    // all vertices are included
    let vertices = triangulation
        .vertices()
        .map(|v| {
            let p = v.position();
            Vec3::new(p.x, 0.0, p.y)
        })
        .collect_vec();

    // for polygons, check if each face lies within the obstacle hulls to omit them
    let polygons = triangulation
        .inner_faces()
        .filter_map(|f| {
            let center = f.center();
            for hull in hulls.iter() {
                if vectors::lies_within_convex_hull(hull, &Vec3::new(center.x, 0.0, center.y)) {
                    return None;
                }
            }
            Some(f.vertices().map(|v| v.index()).to_vec())
        })
        .collect_vec();
    let culled_polys = polygons.len();

    let mut edges = Vec::new();

    for edge in triangulation.undirected_edges() {
        edges.push(
            edge.vertices()
                .map(|p| Vec3::new(p.position().x, 0.0, p.position().y)),
        );
    }

    let navmesh = landmass::NavigationMesh {
        mesh_bounds: None,
        vertices,
        polygons,
    }
    .validate()
    .map_err(|_| NavMeshGenerationError::Validation)?;

    let archipelago = commands
        .spawn(Archipelago::new(
            landmass::Archipelago::create_from_navigation_mesh(navmesh),
        ))
        .id();

    commands.insert_resource(NavMesh { archipelago });

    // TODO: yes, this is a big waste of memory if debugging is disabled.
    // there's not really a clean way to toggle debugging while putting this line here
    // because at the end of the system I lose access to edges if I don't copy them.
    // this would be solved if `Archipelago.archipelago.nav_data` was pub, but it isn't.
    // either I'll fork it or cook up a solution, but this should definitely be fixed.
    commands.insert_resource(NavMeshEdges(edges));

    Ok(NavMeshStatistics {
        hulls: hulls.len(),
        edges: triangulation.num_undirected_edges(),
        constraints: triangulation.num_constraints(),
        verts: triangulation.num_vertices(),
        raw_polys: triangulation.num_inner_faces(),
        culled_polys,
    })
}

pub fn finish_navmesh_generation(
    In(result): In<Result<NavMeshStatistics, NavMeshGenerationError>>,
    mut map_state: ResMut<NextState<MapLoadState>>,
) {
    match result {
        Ok(stats) => {
            map_state.set(MapLoadState::Success);
            info!("Navmesh generation success: {:#?}", stats);
        }
        Err(e) => {
            map_state.set(MapLoadState::Fail);
            error!("Navmesh generation fail: {:?}", e);
        }
    }
}

#[derive(Resource)]
pub struct NavMesh {
    pub archipelago: Entity,
}

#[derive(Resource)]
pub struct NavMeshEdges(Vec<[Vec3; 2]>);

pub fn draw_navmesh(mut gizmos: Gizmos, edges: Res<NavMeshEdges>) {
    for [p0, p1] in edges.0.iter() {
        gizmos.line(*p0, *p1, Color::WHITE);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MapLoadState {
    #[default]
    NotLoaded,
    Loading,
    Success,
    Fail,
}

impl States for MapLoadState {
    type Iter = IntoIter<MapLoadState, 4>;

    fn variants() -> Self::Iter {
        [Self::NotLoaded, Self::Loading, Self::Success, Self::Fail].into_iter()
    }
}
