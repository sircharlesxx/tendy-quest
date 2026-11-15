use bevy::prelude::*;
use crate::state::{LevelEntity, PlatformBox, PowerupKind, PowerupPickup, TendyPickup};

pub fn setup_lighting_and_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 550.0,
    });

    // Single warm sunny kitchen directional light (WebGL2 limit = 1 directional light)
    commands.spawn((
        DirectionalLight {
            illuminance: 19000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.86),
            ..default()
        },
        Transform::from_xyz(4.0, 22.0, 13.0).looking_at(Vec3::new(0.0, 0.0, -4.0), Vec3::Y),
    ));

    // Warm overhead kitchen fill & specular highlights across wood, ceramics, glass & syrup
    for (x, z) in [(-3.5_f32, 12.0_f32), (3.5_f32, 0.0_f32), (0.0_f32, -12.0_f32)] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.94, 0.84),
                intensity: 90000.0,
                range: 32.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 7.0, z),
        ));
    }

    // 3rd-person follow camera (updated by update_camera)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.6, 23.9).looking_at(Vec3::new(0.0, 1.25, 16.0), Vec3::Y),
    ));
}

pub fn spawn_kitchen_level(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_kitchen_level_inner(&mut commands, &asset_server, &mut meshes, &mut materials);
}

pub fn spawn_kitchen_level_inner(
    commands: &mut Commands,
    asset_server: &AssetServer,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    // Blender Giant Kitchen environment (butcher-block countertop, sink, window, gingham curtains, cabinets, cutting boards)
    let kitchen_scene: Handle<Scene> = asset_server.load("models/kitchen_environment.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(kitchen_scene),
        Transform::IDENTITY,
    ));

    // Invisible physics collider for the countertop: X in [-7, 7], Z in [-22, 22], top at Y = 0
    commands.spawn((
        LevelEntity,
        Transform::from_xyz(0.0, -0.5, 0.0),
        PlatformBox {
            half_extents: Vec3::new(7.0, 0.5, 22.0),
        },
    ));

    // Jumpable Kitchen Sponge platforms (from kitchen_sponge.glb)
    let sponge_scene: Handle<Scene> = asset_server.load("models/kitchen_sponge.glb#Scene0");
    for (x, z) in [(0.0_f32, 8.0_f32), (-2.5_f32, -5.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(sponge_scene.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(1.15)),
            PlatformBox {
                half_extents: Vec3::new(1.25, 0.45, 0.8),
            },
        ));
    }

    // Final Goal: Giant Pancake Stack at Z = -17.5 (top at Y ~ 1.9)
    let pancake_scene: Handle<Scene> = asset_server.load("models/pancake_stack.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(pancake_scene),
        Transform::from_xyz(0.0, 0.0, -17.5).with_scale(Vec3::splat(1.1)),
        PlatformBox {
            half_extents: Vec3::new(1.7, 0.95, 1.7),
        },
    ));

    // Decorative Kitchen Props (matching the video!)
    let plate_egg: Handle<Scene> = asset_server.load("models/plate_with_egg.glb#Scene0");
    for (x, z) in [(4.6_f32, 14.0_f32), (-4.8_f32, 2.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(plate_egg.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let syrup: Handle<Scene> = asset_server.load("models/syrup_bottle.glb#Scene0");
    for (x, z) in [(-5.2_f32, 12.2_f32), (5.2_f32, -8.0_f32), (-5.2_f32, -14.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(syrup.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let salt: Handle<Scene> = asset_server.load("models/salt_shaker.glb#Scene0");
    for (x, z) in [(-5.0_f32, 15.2_f32), (5.0_f32, 4.0_f32), (5.0_f32, -15.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(salt.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    // 10 Golden Chicken Tendies leading down the countertop to the Pancake Stack!
    let tendy_scene: Handle<Scene> = asset_server.load("models/golden_tendy.glb#Scene0");
    let tendy_spots = [
        (0.0_f32, 0.4_f32, 13.0_f32, false),
        (-2.5, 0.4, 10.5, false),
        (0.0, 1.4, 8.0, false),
        (3.0, 0.4, 5.0, false),
        (-3.2, 0.4, 1.5, false),
        (2.4, 0.4, -2.0, false),
        (-2.5, 1.4, -5.0, false),
        (3.2, 0.4, -9.0, false),
        (0.0, 0.4, -12.5, false),
        (0.0, 2.5, -17.5, true),
    ];
    for (i, (x, y, z, is_final)) in tendy_spots.into_iter().enumerate() {
        commands
            .spawn((
                LevelEntity,
                SceneRoot(tendy_scene.clone()),
                Transform::from_xyz(x, y, z).with_scale(Vec3::splat(if is_final { 1.35 } else { 1.0 })),
                TendyPickup {
                    base_y: y,
                    phase: i as f32 * 0.7,
                    is_final_goal: is_final,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.78, 0.22),
                        intensity: if is_final { 12000.0 } else { 6000.0 },
                        range: 5.5,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.6, 0.0),
                ));
            });
    }

    // Power-ups: Honey Mustard (speed) & Ranch Shield (absorbs 1 hit)
    let mustard_scene: Handle<Scene> = asset_server.load("models/honey_mustard_cup.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(mustard_scene),
        Transform::from_xyz(-3.5, 0.4, 12.0),
        PowerupPickup {
            kind: PowerupKind::HoneyMustard,
            base_y: 0.4,
            phase: 0.0,
        },
    ));

    let ranch_scene: Handle<Scene> = asset_server.load("models/ranch_shield.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(ranch_scene),
        Transform::from_xyz(3.5, 0.4, -1.0),
        PowerupPickup {
            kind: PowerupKind::RanchShield,
            base_y: 0.4,
            phase: 1.5,
        },
    ));
}
