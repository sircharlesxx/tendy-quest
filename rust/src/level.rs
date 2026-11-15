use bevy::prelude::*;
use crate::state::{LevelEntity, PlatformBox, PowerupKind, PowerupPickup, TendyPickup};

pub fn setup_lighting_and_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.84, 0.91, 0.98)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.95, 0.88),
        brightness: 650.0,
    });

    // Warm sunny kitchen sunlight
    commands.spawn((
        DirectionalLight {
            illuminance: 16000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
        Transform::from_xyz(8.0, 18.0, 12.0).looking_at(Vec3::new(0.0, 0.0, -4.0), Vec3::Y),
    ));

    // 3rd-person follow camera (positioned in player.rs)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 4.2, 22.0).looking_at(Vec3::new(0.0, 1.2, 12.0), Vec3::Y),
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
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.82, 0.62, 0.40),
        perceptual_roughness: 0.62,
        ..default()
    });
    let backsplash_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.93, 0.88),
        perceptual_roughness: 0.7,
        ..default()
    });
    let cabinet_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.31, 0.19),
        perceptual_roughness: 0.75,
        ..default()
    });
    let window_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.90, 1.0),
        emissive: LinearRgba::rgb(0.65, 0.82, 1.0),
        ..default()
    });

    // Long Giant Wooden Countertop: X in [-7, 7], Z in [-22, 22], top at Y = 0
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::new(14.0, 1.0, 44.0))),
        MeshMaterial3d(wood_mat.clone()),
        Transform::from_xyz(0.0, -0.5, 0.0),
        PlatformBox {
            half_extents: Vec3::new(7.0, 0.5, 22.0),
        },
    ));

    // Kitchen wall + sunny window at far end (Z = -23)
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::new(30.0, 14.0, 1.0))),
        MeshMaterial3d(backsplash_mat.clone()),
        Transform::from_xyz(0.0, 6.0, -23.0),
    ));
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::new(10.0, 6.0, 0.2))),
        MeshMaterial3d(window_mat),
        Transform::from_xyz(0.0, 6.5, -22.4),
    ));
    // Upper cabinets flanking the window
    for x in [-9.5_f32, 9.5_f32] {
        commands.spawn((
            LevelEntity,
            Mesh3d(meshes.add(Cuboid::new(6.5, 6.5, 2.5))),
            MeshMaterial3d(cabinet_mat.clone()),
            Transform::from_xyz(x, 7.0, -21.5),
        ));
    }

    // Jumpable Kitchen Sponge platforms (from kitchen_sponge.glb)
    let sponge_scene: Handle<Scene> = asset_server.load("assets/models/kitchen_sponge.glb#Scene0");
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
    let pancake_scene: Handle<Scene> = asset_server.load("assets/models/pancake_stack.glb#Scene0");
    commands.spawn((
        LevelEntity,
        SceneRoot(pancake_scene),
        Transform::from_xyz(0.0, 0.0, -17.5).with_scale(Vec3::splat(1.1)),
        PlatformBox {
            half_extents: Vec3::new(1.7, 0.95, 1.7),
        },
    ));

    // Decorative Kitchen Props (matching the video!)
    let plate_egg: Handle<Scene> = asset_server.load("assets/models/plate_with_egg.glb#Scene0");
    for (x, z) in [(4.6_f32, 14.0_f32), (-4.8_f32, 2.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(plate_egg.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let syrup: Handle<Scene> = asset_server.load("assets/models/syrup_bottle.glb#Scene0");
    for (x, z) in [(-5.4_f32, 12.0_f32), (5.2_f32, -8.0_f32), (-5.2_f32, -14.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(syrup.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    let salt: Handle<Scene> = asset_server.load("assets/models/salt_shaker.glb#Scene0");
    for (x, z) in [(-5.2_f32, 15.0_f32), (5.0_f32, 4.0_f32), (5.0_f32, -15.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(salt.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    // 10 Golden Chicken Tendies leading down the countertop to the Pancake Stack!
    let tendy_scene: Handle<Scene> = asset_server.load("assets/models/golden_tendy.glb#Scene0");
    let tendy_spots = [
        (0.0_f32, 0.4_f32, 13.0_f32, false),
        (-2.5, 0.4, 10.5, false),
        (0.0, 1.4, 8.0, false), // above sponge 1
        (3.0, 0.4, 5.0, false),
        (-3.2, 0.4, 1.5, false),
        (2.4, 0.4, -2.0, false),
        (-2.5, 1.4, -5.0, false), // above sponge 2
        (3.2, 0.4, -9.0, false),
        (0.0, 0.4, -12.5, false),
        (0.0, 2.5, -17.5, true), // Final Tendy atop the Pancake Stack!
    ];
    for (i, (x, y, z, is_final)) in tendy_spots.into_iter().enumerate() {
        commands.spawn((
            LevelEntity,
            SceneRoot(tendy_scene.clone()),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(if is_final { 1.35 } else { 1.0 })),
            TendyPickup {
                base_y: y,
                phase: i as f32 * 0.7,
                is_final_goal: is_final,
            },
        ));
    }

    // Power-ups: Honey Mustard (speed) & Ranch Shield (absorbs 1 hit)
    let mustard_scene: Handle<Scene> = asset_server.load("assets/models/honey_mustard_cup.glb#Scene0");
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

    let ranch_scene: Handle<Scene> = asset_server.load("assets/models/ranch_shield.glb#Scene0");
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
