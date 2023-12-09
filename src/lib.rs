mod aabb;
mod ball;
mod boundaries;
mod collision;
mod grid2;
mod gym;
mod occupancy_grid;
mod parameters;
mod player;
mod squad;
mod squad_ui;
mod team;

use ball::{Ball, BallAssets};
use bevy::core_pipeline::bloom::BloomSettings;
use bevy::prelude::*;
use bevy::render::view::NoFrustumCulling;
use bevy::window::CursorGrabMode;
use bevy_egui::EguiPlugin;
use bevy_mod_picking::prelude::*;
use bevy_mod_picking::DefaultPickingPlugins;
use bevy_rapier3d::prelude::*;
use boundaries::Boundaries;
use collision::{handle_ball_floor_collisions, handle_ball_player_collisions};
use gym::{Gym, GymAssets, GymParams};
use occupancy_grid::OccupancyGrid;
use player::{AvoidPlayers, KnockedOut, Player};
use smooth_bevy_cameras::controllers::orbit::{
    OrbitCameraBundle, OrbitCameraController, OrbitCameraPlugin,
};
use smooth_bevy_cameras::LookTransformPlugin;
use squad::{AllSquadAssets, Squad, SquadAi, SquadBehaviors, SquadStates};
use squad_ui::SquadUi;
use team::AllTeamAssets;

// IDEAS
// - add point-buy for squad statistics
// - add game start menu with configurable parameters
// - add enemy AI
// - make the throw loft adjustable, using a gizmo to show the arc
// - make players holding balls run to the "front" of their cluster
// - let players holding balls run closer to the current target enemy if they're
//   out of range
// - let players leave their cluster to quickly grab a nearby ball
// - let players block incoming thrown balls if they are holding a ball
// - make the cluster shape adjustable
// - let players dodge out of the way of thrown balls

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            EguiPlugin,
            DefaultPickingPlugins,
            RapierPhysicsPlugin::<NoUserData>::default(),
            // RapierDebugRenderPlugin::default(),
            LookTransformPlugin,
            OrbitCameraPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::rgb_u8(52, 75, 99)))
        .insert_resource(RapierBackendSettings {
            require_markers: true,
        })
        .init_resource::<SquadUi>()
        .add_systems(Startup, (setup, transparency_hack))
        .add_systems(PreUpdate, emulate_right_click_with_alt)
        .add_systems(
            Update,
            (
                grab_mouse,
                OccupancyGrid::update,
                SquadUi::toggle,
                SquadUi::draw,
                SquadAi::move_to_requested_positions,
                SquadAi::find_target_enemy,
            ),
        )
        .add_systems(Update, Player::initialize_kinematics)
        .add_systems(
            Update,
            (
                AvoidPlayers::avoid_other_players,
                Player::throw_ball_at_enemy,
                Player::follow_leader,
                KnockedOut::update,
                handle_ball_player_collisions,
                handle_ball_floor_collisions,
            )
                .after(Player::initialize_kinematics)
                .before(Player::finalize_kinematics),
        )
        .add_systems(Update, Player::finalize_kinematics)
        .add_systems(PostUpdate, SquadStates::update);
    }
}

// Mostly for laptops without good right click and drag support.
fn emulate_right_click_with_alt(keys: Res<Input<KeyCode>>, mut mouse: ResMut<Input<MouseButton>>) {
    if keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight) {
        mouse.press(MouseButton::Right);
    }
    if keys.just_released(KeyCode::AltLeft) || keys.just_released(KeyCode::AltRight) {
        mouse.release(MouseButton::Right);
    }
}

fn grab_mouse(
    mut windows: Query<&mut Window>,
    mouse: Res<Input<MouseButton>>,
    key: Res<Input<KeyCode>>,
) {
    let mut window = windows.single_mut();

    if mouse.just_pressed(MouseButton::Left) {
        window.cursor.grab_mode = CursorGrabMode::Locked;
    }

    if key.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let gym_params = GymParams::default();
    let he = gym_params.half_extents();
    let gym_assets = GymAssets::new(gym_params, &mut meshes, &mut materials);
    Gym::spawn(&mut commands, &gym_assets);
    let bounds = Boundaries { min: -he, max: he };
    let player_spawn_aabbs = gym_params.player_spawn_aabbs(16.0);
    let ball_spawn_aabb = gym_params.ball_spawn_aabb(4.0);
    let occupancy = gym_params.occupancy_grid();

    commands
        .spawn(Camera3dBundle {
            camera: Camera {
                // Required for bloom.
                hdr: true,
                ..default()
            },
            ..default()
        })
        .insert((
            BloomSettings::default(),
            OrbitCameraBundle::new(
                OrbitCameraController {
                    mouse_rotate_sensitivity: Vec2::splat(0.3),
                    mouse_translate_sensitivity: Vec2::splat(4.0),
                    mouse_wheel_zoom_sensitivity: 0.2,
                    pixels_per_line: 53.0,
                    smoothing_weight: 0.8,
                    ..default()
                },
                Vec3::new(50.0, 50.0, 0.0),
                Vec3::ZERO,
                Vec3::Y,
            ),
            RapierPickable,
        ));

    // TODO: animated spotlights could look really cool
    let hhe = 0.5 * he;
    let light_positions = [
        Vec3::new(0.0, 5.0, 0.0),
        Vec3::new(-hhe.x, 5.0, -hhe.z),
        Vec3::new(hhe.x, 5.0, -hhe.z),
        Vec3::new(-hhe.x, 5.0, hhe.z),
        Vec3::new(hhe.x, 5.0, hhe.z),
    ];
    for light_position in light_positions {
        commands.spawn(PointLightBundle {
            point_light: PointLight {
                intensity: 2000.0,
                range: 50.0,
                shadows_enabled: true,
                ..default()
            },
            transform: Transform::from_translation(light_position),
            ..default()
        });
    }

    let ball_assets = BallAssets::new(&mut meshes, &mut materials);
    let n_balls = 1000;
    Ball::spawn_multiple_in_aabb(
        &mut commands,
        &ball_assets,
        &bounds,
        ball_spawn_aabb,
        n_balls,
    );

    let team_colors = [Color::GREEN, Color::BLUE];
    let squad_teams = [0, 0, 0, 0, 1, 1, 1, 1];
    let n_squads = squad_teams.len();
    let squad_size = 750;

    let squad_colors = squad_teams.map(|t| team_colors[t as usize]);
    let team_assets = AllTeamAssets::new(team_colors, &mut meshes, &mut materials);
    let squad_assets = AllSquadAssets::new(squad_colors, &mut materials);

    let mut squad_ai_entities = Vec::new();
    Squad::spawn_in_line(
        &mut commands,
        &team_assets.teams[0],
        &squad_assets,
        0,
        0..4,
        player_spawn_aabbs[0],
        squad_size,
        &mut squad_ai_entities,
    );
    Squad::spawn_in_line(
        &mut commands,
        &team_assets.teams[1],
        &squad_assets,
        1,
        4..8,
        player_spawn_aabbs[1],
        squad_size,
        &mut squad_ai_entities,
    );

    let squad_behaviors = SquadBehaviors::new(squad_ai_entities);
    let squad_states = SquadStates::new(vec![squad_size; n_squads]);

    commands.insert_resource(ball_assets);
    commands.insert_resource(bounds);
    commands.insert_resource(occupancy);
    commands.insert_resource(squad_behaviors);
    commands.insert_resource(squad_states);
    commands.insert_resource(team_assets);
    commands.insert_resource(squad_assets);
}

// HACK: front-load a stutter that occurs the first time a transparent material
// is rendered
fn transparency_hack(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let transparent_material = materials.add(Color::GRAY.with_a(0.01).into());
    commands
        .spawn(PbrBundle {
            material: transparent_material,
            // Keep it out of site.
            transform: Transform::from_xyz(0.0, -f32::INFINITY, 0.0),
            mesh: meshes.add(
                shape::Plane {
                    size: 0.0,
                    subdivisions: 0,
                }
                .try_into()
                .unwrap(),
            ),
            ..default()
        })
        // Needs to actually get drawn.
        .insert(NoFrustumCulling);
}
