use std::array::IntoIter;

use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use bevy_landmass::Archipelago;
use geo::{
    BooleanOps, Contains, ConvexHull, Coord, Line, LineString, LinesIter, MultiPolygon, OpType,
};
use geo_offset::Offset;
use itertools::Itertools;
use spade::{ConstrainedDelaunayTriangulation, Point2, Triangulation};

use crate::{
    collider, humanoid::HUMANOID_RADIUS, render::sketched::NoOutline, util::vectors::Vec3Ext,
};

/// How much to offset navmesh from obstacles.
//
// TODO: odds are I will need multiple navmeshes for agents of similar radii
// right now there are only humanoids, but this needs to be done when I add others
pub const NAVMESH_EROSION: f64 = HUMANOID_RADIUS as f64;

#[derive(Default)]
pub struct MapPlugin {
    pub navmesh_debugging: Option<Color>,
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

        if let Some(color) = self.navmesh_debugging {
            app.add_systems(
                Update,
                draw_navmesh_system_with_color(color).run_if(in_state(MapLoadState::Success)),
            );
        }
    }
}

#[derive(Component)]
pub struct Map;

#[derive(Debug)]
pub struct NavMeshStatistics {
    pub simple_boundaries: usize,
    pub simple_obstacles: usize,
    pub merged_boundaries: usize,
    pub merged_obstacles: usize,
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

    let mut map_meshes = Vec::default();

    for e_node in children_query.iter_descendants(e_map) {
        let Ok((g_transform, mesh, name)) = mesh_query.get(e_node) else {
            continue;
        };

        // polygons within "restricted" hulls are excluded
        // I don't want this to include the larger, all-encompassing floor
        // which will define the navmesh boundary
        let restricted = match name.as_str() {
            "Plane" => false,
            _ => true,
        };

        map_meshes.push((restricted, *g_transform, mesh.clone()));

        commands.entity(e_node).insert(collider!(&meshes, mesh));
        if !restricted {
            commands.entity(e_node).insert(NoOutline);
        }
    }

    // https://skatgame.net/mburo/ps/thesis_demyen_2006.pdf
    // https://www.jdxdev.com/blog/2021/07/06/rts-pathfinding-2-dynamic-navmesh-with-constrained-delaunay-triangles/
    //
    // I would like to thank this guy. touched it up a little, but it is basically their's.
    // https://discord.com/channels/691052431525675048/1138102751444877333/1138136084895772682
    info!("Begin navmesh generation...");

    let mut triangulation = ConstrainedDelaunayTriangulation::<Point2<f32>>::new();
    let mut boundaries = Vec::new(); // boundary hulls (allow contained polygons)
    let mut obstacles = Vec::new(); // obstacle hulls (cull contained polygons)

    for (restricted, g_transform, h_mesh) in map_meshes.iter() {
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

        // when quantum computers become mainstream
        // I hope there's no such thing as f32 vs f64
        let hull = LineString::from_iter(positions.iter().map(|v| Coord {
            x: v.x as f64,
            y: v.z as f64,
        }))
        .convex_hull();
        match restricted {
            false => &mut boundaries,
            true => &mut obstacles,
        }
        .push(hull);
    }

    let n_boundaries = boundaries.len();
    let n_obstacles = obstacles.len();

    let boundaries = boundaries
        .into_iter()
        // combine intersecting zones into a single shape
        .fold(MultiPolygon::new(Vec::default()), |acc, hull| {
            // erode the boundary area
            match hull.offset_with_arc_segments(-NAVMESH_EROSION, 1) {
                Ok(hull) => acc.boolean_op(&hull, OpType::Union),
                Err(..) => acc,
            }
        });

    let obstacles = obstacles
        .into_iter()
        // combine intersecting obstacles into a single shape
        .fold(MultiPolygon::new(Vec::default()), |acc, hull| {
            // increase obstacle radius
            match hull.offset_with_arc_segments(NAVMESH_EROSION, 1) {
                Ok(hull) => acc.boolean_op(&hull, OpType::Union),
                Err(..) => acc,
            }
        })
        // remove edges outside of the map bounds
        .boolean_op(&boundaries, OpType::Intersection);

    // use all hull edges as constraints
    for Line { start, end } in obstacles.lines_iter().chain(boundaries.lines_iter()) {
        triangulation
            .add_constraint_edge(
                Point2::new(start.x as f32, start.y as f32),
                Point2::new(end.x as f32, end.y as f32),
            )
            .map_err(|e| NavMeshGenerationError::BadVertex(e))?;
    }

    // all vertices are included
    let vertices = triangulation
        .vertices()
        .map(|v| Vec3::new(v.position().x, 0.0, v.position().y))
        .collect_vec();

    // for polygons, check if each face lies within the obstacle hulls to omit them
    let polygons = triangulation
        .inner_faces()
        .filter_map(|f| {
            let Point2 { x, y } = f.center();
            for hull in obstacles.iter() {
                if hull.contains(&Coord {
                    x: x as f64,
                    y: y as f64,
                }) {
                    return None;
                }
            }
            Some(f.vertices().map(|v| v.index()).to_vec())
        })
        .collect_vec();
    let n_culled_polys = polygons.len();

    let navmesh_geometry = landmass::NavigationMesh {
        mesh_bounds: None,
        vertices,
        polygons,
    };

    // TODO: unfortunately the nav mesh data is not pub.
    // is there a good way to not copy this when debug is off?
    commands.insert_resource(NavMeshGeometry(navmesh_geometry.clone()));

    let navmesh = navmesh_geometry
        .validate()
        .map_err(|_| NavMeshGenerationError::Validation)?;

    let archipelago = commands
        .spawn(Archipelago::new(
            landmass::Archipelago::create_from_navigation_mesh(navmesh),
        ))
        .id();

    commands.insert_resource(NavMesh { archipelago });

    Ok(NavMeshStatistics {
        simple_boundaries: n_boundaries,
        simple_obstacles: n_obstacles,
        merged_boundaries: boundaries.0.len(),
        merged_obstacles: obstacles.0.len(),
        edges: triangulation.num_undirected_edges(),
        constraints: triangulation.num_constraints(),
        verts: triangulation.num_vertices(),
        raw_polys: triangulation.num_inner_faces(),
        culled_polys: n_culled_polys,
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
pub struct NavMeshGeometry(pub landmass::NavigationMesh);

fn draw_navmesh_system_with_color(color: Color) -> impl Fn(Gizmos, Res<NavMeshGeometry>) {
    move |mut gizmos: Gizmos, navmesh: Res<NavMeshGeometry>| {
        for poly in navmesh.0.polygons.iter() {
            for (p0, p1) in poly.iter().copied().circular_tuple_windows() {
                gizmos.line(navmesh.0.vertices[p0], navmesh.0.vertices[p1], color);
            }
        }
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
