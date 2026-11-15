use std::f32::consts::PI;
use bevy::asset::LoadState;
use bevy::prelude::*;
use crate::state::{
    FridgeGoal, HeadphoneHazard, LevelEntity, PlatformBox, PowerupKind, PowerupPickup,
    SceneRetryPath, TendyPickup,
};

struct SimpleRng(u64);

impl SimpleRng {
    fn from_entropy() -> Self {
        let mut bytes = [0u8; 8];
        if getrandom::getrandom(&mut bytes).is_ok() {
            let v = u64::from_le_bytes(bytes);
            Self(if v == 0 { 0x9E37_79B9_7F4A_7C15 } else { v })
        } else {
            Self(0x517C_C1B7_2722_0A95)
        }
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.f32() * (hi - lo)
    }

    fn usize_range(&mut self, lo: usize, hi_inclusive: usize) -> usize {
        if hi_inclusive <= lo {
            return lo;
        }
        lo + (self.next_u32() as usize) % (hi_inclusive - lo + 1)
    }

    fn shuffle_indices(&mut self, n: usize) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            idx.swap(i, j);
        }
        idx
    }
}

pub fn retry_failed_scenes(
    asset_server: Res<AssetServer>,
    mut query: Query<(&mut SceneRoot, &mut SceneRetryPath)>,
) {
    for (mut scene, mut retry) in &mut query {
        if retry.attempts >= 4 {
            continue;
        }
        if matches!(
            asset_server.get_load_state(scene.0.id()),
            Some(LoadState::Failed(_))
        ) {
            retry.attempts += 1;
            scene.0 = asset_server.load(retry.path);
        }
    }
}

fn spawn_visual_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    size: Vec3,
    color: Color,
    perceptual_roughness: f32,
    metallic: f32,
) {
    commands.spawn((
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness,
            metallic,
            ..default()
        })),
        Transform::from_translation(center),
    ));
}

fn spawn_solid_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    half_extents: Vec3,
    style: Option<(Color, f32, f32)>,
) {
    let mut ec = commands.spawn((
        LevelEntity,
        Transform::from_translation(center),
        PlatformBox { half_extents },
    ));
    if let Some((color, perceptual_roughness, metallic)) = style {
        let size = (half_extents * 2.0).max(Vec3::splat(0.05));
        ec.insert((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness,
                metallic,
                ..default()
            })),
        ));
    }
}

fn spawn_counter_or_stool(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center_xz: Vec2,
    top_y: f32,
    height: f32,
    width_x: f32,
    depth_z: f32,
    top_col: Color,
    base_col: Color,
) {
    let center_y = top_y - height * 0.5;
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(center_xz.x, center_y, center_xz.y),
        Vec3::new(width_x * 0.5, height * 0.5, depth_z * 0.5),
        None,
    );
    let slab_h = (height * 0.18).clamp(0.24, 0.85);
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(center_xz.x, top_y - slab_h * 0.5, center_xz.y),
        Vec3::new(width_x, slab_h, depth_z),
        top_col,
        0.35,
        0.0,
    );
    if height > slab_h + 0.15 {
        let body_h = height - slab_h;
        spawn_visual_box(
            commands,
            meshes,
            materials,
            Vec3::new(center_xz.x, top_y - slab_h - body_h * 0.5, center_xz.y),
            Vec3::new(width_x - 0.35, body_h, depth_z - 0.35),
            base_col,
            0.52,
            0.0,
        );
    }
}

fn spawn_ricos_cheese_cup(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    spilled: bool,
) {
    let cup_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.12, 0.10),
        perceptual_roughness: 0.42,
        ..default()
    });
    let cup_yellow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.82, 0.08),
        perceptual_roughness: 0.38,
        ..default()
    });
    let cup_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.96, 0.94),
        perceptual_roughness: 0.45,
        ..default()
    });
    let nacho_cheese = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.64, 0.04),
        emissive: LinearRgba::new(0.22, 0.10, 0.0, 1.0),
        perceptual_roughness: 0.22,
        ..default()
    });

    if spilled {
        commands.spawn((
            LevelEntity,
            Mesh3d(meshes.add(Cylinder::new(1.55 * scale, 0.07))),
            MeshMaterial3d(nacho_cheese.clone()),
            Transform::from_xyz(pos.x, pos.y + 0.035, pos.z),
        ));
    }

    let tilt = if spilled {
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(1.22)
    } else {
        Quat::from_rotation_y(yaw)
    };
    let visual_y = if spilled {
        pos.y + 0.86 * scale
    } else {
        pos.y
    };

    commands
        .spawn((
            LevelEntity,
            Transform::from_xyz(pos.x, visual_y, pos.z)
                .with_rotation(tilt)
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|cup| {
            cup.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.88, 1.55))),
                MeshMaterial3d(cup_red),
                Transform::from_xyz(0.0, 0.78, 0.0),
            ));
            cup.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.91, 0.44))),
                MeshMaterial3d(cup_yellow),
                Transform::from_xyz(0.0, 0.88, 0.0),
            ));
            cup.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.95, 0.14))),
                MeshMaterial3d(cup_white),
                Transform::from_xyz(0.0, 1.56, 0.0),
            ));
            cup.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.84, 0.16))),
                MeshMaterial3d(nacho_cheese),
                Transform::from_xyz(0.0, 1.58, 0.0),
            ));
        });

    let collider_y = if spilled { 0.72 * scale } else { 0.86 * scale };
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(pos.x, pos.y + collider_y, pos.z),
        Vec3::new(0.92 * scale, collider_y, 0.92 * scale),
        None,
    );
}

fn spawn_game_controller_obstacle(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
) {
    let shell = materials.add(StandardMaterial {
        base_color: Color::srgb(0.14, 0.16, 0.21),
        perceptual_roughness: 0.36,
        metallic: 0.15,
        ..default()
    });
    let dark_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.06, 0.07, 0.09),
        perceptual_roughness: 0.55,
        ..default()
    });
    let light_bar = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.75, 1.0),
        emissive: LinearRgba::new(0.35, 0.95, 1.6, 1.0),
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|pad| {
            // Center chassis + touchpad + glowing LED strip
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(2.05, 0.58, 1.25)))),
                MeshMaterial3d(shell.clone()),
                Transform::from_xyz(0.0, 0.34, 0.0),
            ));
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(1.05, 0.08, 0.58)))),
                MeshMaterial3d(dark_trim.clone()),
                Transform::from_xyz(0.0, 0.64, -0.14),
            ));
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.95, 0.09, 0.10)))),
                MeshMaterial3d(light_bar),
                Transform::from_xyz(0.0, 0.54, -0.60),
            ));
            // Left & right angled grips
            for sx in [-1.0_f32, 1.0_f32] {
                pad.spawn((
                    Mesh3d(meshes.add(Capsule3d::new(0.32, 0.92))),
                    MeshMaterial3d(shell.clone()),
                    Transform::from_xyz(sx * 1.10, 0.28, 0.38)
                        .with_rotation(Quat::from_rotation_x(0.95) * Quat::from_rotation_z(-sx * 0.28)),
                ));
                // Analog thumbsticks
                pad.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.18, 0.22))),
                    MeshMaterial3d(dark_trim.clone()),
                    Transform::from_xyz(sx * 0.42, 0.72, 0.26),
                ));
            }
            // D-pad on left
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.42, 0.10, 0.14)))),
                MeshMaterial3d(dark_trim.clone()),
                Transform::from_xyz(-0.72, 0.66, -0.12),
            ));
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.14, 0.10, 0.42)))),
                MeshMaterial3d(dark_trim),
                Transform::from_xyz(-0.72, 0.66, -0.12),
            ));
            // Colored ABXY face buttons on right
            for (bx, bz, col) in [
                (0.72_f32, -0.28_f32, Color::srgb(0.95, 0.78, 0.15)),
                (0.72_f32, 0.04_f32, Color::srgb(0.20, 0.85, 0.35)),
                (0.56_f32, -0.12_f32, Color::srgb(0.25, 0.55, 1.0)),
                (0.88_f32, -0.12_f32, Color::srgb(0.95, 0.25, 0.25)),
            ] {
                pad.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.085))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: col,
                        emissive: LinearRgba::from(col) * 0.25,
                        ..default()
                    })),
                    Transform::from_xyz(bx, 0.66, bz),
                ));
            }
        });

    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(pos.x, pos.y + 0.44 * scale, pos.z),
        Vec3::new(1.52 * scale, 0.44 * scale, 0.98 * scale),
        None,
    );
}

fn spawn_floor_headphones(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
) {
    let frame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.11, 0.15),
        perceptual_roughness: 0.35,
        metallic: 0.3,
        ..default()
    });
    let pad_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.22, 0.26),
        perceptual_roughness: 0.85,
        ..default()
    });
    let neon_trim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.88, 1.0),
        emissive: LinearRgba::new(0.25, 1.4, 2.0, 1.0),
        unlit: true,
        ..default()
    });
    let warn_ring = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.35, 0.18, 0.55),
        emissive: LinearRgba::new(0.9, 0.22, 0.08, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            HeadphoneHazard {
                radius: 1.78 * scale,
            },
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|hp| {
            // Subtle danger ring around floor headphones (Bevy Torus::new takes inner_radius, outer_radius)
            hp.spawn((
                Mesh3d(meshes.add(Torus::new(1.38, 1.56))),
                MeshMaterial3d(warn_ring),
                Transform::from_xyz(0.0, 0.03, 0.0).with_scale(Vec3::new(1.0, 0.22, 1.0)),
            ));
            // Curved headband resting flat on floor (-Z arch connecting left/right earcups)
            for (seg_x, seg_z, rot_y) in [
                (-0.78_f32, -0.58_f32, -0.55_f32),
                (-0.42_f32, -1.08_f32, -0.25_f32),
                (0.0_f32, -1.25_f32, 0.0_f32),
                (0.42_f32, -1.08_f32, 0.25_f32),
                (0.78_f32, -0.58_f32, 0.55_f32),
            ] {
                hp.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.56, 0.24, 0.18)))),
                    MeshMaterial3d(frame_mat.clone()),
                    Transform::from_xyz(seg_x, 0.28, seg_z)
                        .with_rotation(Quat::from_rotation_y(rot_y)),
                ));
            }
            // Left and Right cushioned over-ear cups + neon ring trim
            for sx in [-1.0_f32, 1.0_f32] {
                hp.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.54, 0.36))),
                    MeshMaterial3d(pad_mat.clone()),
                    Transform::from_xyz(sx * 0.92, 0.28, 0.20)
                        .with_rotation(Quat::from_rotation_z(PI * 0.5)),
                ));
                hp.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.46, 0.42))),
                    MeshMaterial3d(frame_mat.clone()),
                    Transform::from_xyz(sx * 0.96, 0.28, 0.20)
                        .with_rotation(Quat::from_rotation_z(PI * 0.5)),
                ));
                hp.spawn((
                    Mesh3d(meshes.add(Torus::new(0.045, 0.34))),
                    MeshMaterial3d(neon_trim.clone()),
                    Transform::from_xyz(sx * 1.18, 0.28, 0.20)
                        .with_rotation(Quat::from_rotation_z(PI * 0.5)),
                ));
            }
            // Boom mic arm sticking out
            hp.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.045, 0.95))),
                MeshMaterial3d(frame_mat),
                Transform::from_xyz(-0.75, 0.22, 0.85)
                    .with_rotation(Quat::from_rotation_x(PI * 0.5) * Quat::from_rotation_z(0.28)),
            ));
        });
}

pub fn setup_lighting_and_camera(mut commands: Commands) {
    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 620.0,
    });

    // Single warm sunny apartment directional light (WebGL2 limit = 1 directional light)
    commands.spawn((
        DirectionalLight {
            illuminance: 20000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.96, 0.86),
            ..default()
        },
        Transform::from_xyz(12.0, 38.0, 22.0).looking_at(Vec3::new(0.0, -4.0, -6.0), Vec3::Y),
    ));

    // Warm overhead lights spanning the full 1,200 sq ft open-concept apartment
    for (x, z) in [
        (-32.0_f32, 22.0_f32),
        (0.0_f32, 26.0_f32),
        (32.0_f32, 22.0_f32),
        (-32.0_f32, -18.0_f32),
        (0.0_f32, -14.0_f32),
        (32.0_f32, -18.0_f32),
    ] {
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.94, 0.84),
                intensity: 165000.0,
                range: 68.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(x, 11.5, z),
        ));
    }

    // 3rd-person follow camera positioned behind Starter Breakfast Island
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.5, 36.5).looking_at(Vec3::new(0.0, 0.7, 29.0), Vec3::Y),
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
    let mut rng = SimpleRng::from_entropy();

    let wood_col = Color::srgb(0.76, 0.56, 0.36);
    let cab_col = Color::srgb(0.42, 0.27, 0.15);
    let table_col = Color::srgb(0.64, 0.42, 0.24);
    let wall_col = Color::srgb(0.84, 0.82, 0.76);
    let trim_col = Color::srgb(0.94, 0.93, 0.90);
    let tile_col = Color::srgb(0.50, 0.62, 0.52);
    let sofa_col = Color::srgb(0.24, 0.36, 0.52);

    // 1. Full 1,200 Sq Ft Open-Concept Hardwood Apartment Floor (X in [-56, 56], Z in [-48, 48], top Y = -6.0)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -6.5, 0.0),
        Vec3::new(56.0, 0.5, 48.0),
        None,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -6.27, 0.0),
        Vec3::new(112.0, 0.46, 96.0),
        Color::srgb(0.60, 0.44, 0.29),
        0.48,
        0.0,
    );
    // Hardwood plank strips & Area Rugs defining Living Room, Central Boss Arena, and Dining zones
    for x in [-42.0_f32, -28.0, -14.0, 0.0, 14.0, 28.0, 42.0] {
        spawn_visual_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, -6.01, 0.0),
            Vec3::new(0.22, 0.03, 95.5),
            Color::srgb(0.46, 0.33, 0.20),
            0.55,
            0.0,
        );
    }
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(-36.0, -5.98, -6.0),
        Vec3::new(26.0, 0.04, 32.0),
        Color::srgb(0.25, 0.34, 0.46),
        0.92,
        0.0,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -5.98, -10.0),
        Vec3::new(22.0, 0.04, 36.0),
        Color::srgb(0.70, 0.22, 0.18),
        0.88,
        0.0,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(38.0, -5.98, -4.0),
        Vec3::new(20.0, 0.04, 28.0),
        Color::srgb(0.78, 0.72, 0.58),
        0.90,
        0.0,
    );

    // Perimeter 1,200 Sq Ft Apartment Walls + Baseboard Trim (no south visible wall so 3rd-person view stays open)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 5.0, -48.0),
        Vec3::new(56.4, 11.5, 0.4),
        Some((wall_col, 0.74, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-56.0, 5.0, 0.0),
        Vec3::new(0.4, 11.5, 48.4),
        Some((wall_col, 0.74, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(56.0, 5.0, 0.0),
        Vec3::new(0.4, 11.5, 48.4),
        Some((wall_col, 0.74, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 5.0, 48.0),
        Vec3::new(56.4, 11.5, 0.5),
        None,
    );
    // White Baseboards
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -5.45, -47.45),
        Vec3::new(111.5, 1.1, 0.25),
        trim_col,
        0.5,
        0.0,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(-55.45, -5.45, 0.0),
        Vec3::new(0.25, 1.1, 95.5),
        trim_col,
        0.5,
        0.0,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(55.45, -5.45, 0.0),
        Vec3::new(0.25, 1.1, 95.5),
        trim_col,
        0.5,
        0.0,
    );

    // 2. Zone 1: South Starter Breakfast Island (X in [-16, 16], Z in [20, 36], top Y = 0.0)
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(0.0, 28.0),
        0.0,
        6.0,
        32.0,
        16.0,
        wood_col,
        cab_col,
    );
    // Stepping Stools leading down from Starter Island to Apartment Floor
    for (x, top_y, h, z) in [
        (0.0_f32, -1.85_f32, 4.15_f32, 18.0_f32),
        (0.0_f32, -3.85_f32, 2.15_f32, 14.0_f32),
        (18.5_f32, -1.85_f32, 4.15_f32, 25.0_f32),
        (18.5_f32, -3.85_f32, 2.15_f32, 20.5_f32),
        (-18.5_f32, -1.85_f32, 4.15_f32, 25.0_f32),
        (-18.5_f32, -3.85_f32, 2.15_f32, 20.5_f32),
    ] {
        spawn_counter_or_stool(
            commands,
            meshes,
            materials,
            Vec2::new(x, z),
            top_y,
            h,
            if x.abs() < 1.0 { 5.6 } else { 4.4 },
            3.6,
            table_col,
            cab_col,
        );
    }

    // Starter Island Props (Sponges, Giant Pancake Stack, Plates, Syrup, Salt)
    let sponge_path = "models/kitchen_sponge.glb#Scene0";
    let sponge_scene: Handle<Scene> = asset_server.load(sponge_path);
    for (x, z) in [(-6.5_f32, 26.5_f32), (6.5_f32, 25.0_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(sponge_scene.clone()),
            SceneRetryPath::new(sponge_path),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(1.15)),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.45, z),
            Vec3::new(1.25, 0.45, 0.8),
            None,
        );
    }

    let pancake_path = "models/pancake_stack.glb#Scene0";
    let pancake_scene: Handle<Scene> = asset_server.load(pancake_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(pancake_scene),
        SceneRetryPath::new(pancake_path),
        Transform::from_xyz(0.0, 0.0, 22.8).with_scale(Vec3::splat(1.1)),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, 0.95, 22.8),
        Vec3::new(1.7, 0.95, 1.7),
        None,
    );

    let plate_path = "models/plate_with_egg.glb#Scene0";
    let plate_egg: Handle<Scene> = asset_server.load(plate_path);
    for (x, z) in [(-11.0_f32, 30.0_f32), (11.0_f32, 29.5_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(plate_egg.clone()),
            SceneRetryPath::new(plate_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.14, z),
            Vec3::new(1.55, 0.14, 1.55),
            None,
        );
    }

    let syrup_path = "models/syrup_bottle.glb#Scene0";
    let syrup: Handle<Scene> = asset_server.load(syrup_path);
    for (x, z) in [(-12.5_f32, 24.0_f32), (12.5_f32, 23.5_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(syrup.clone()),
            SceneRetryPath::new(syrup_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 1.4, z),
            Vec3::new(0.68, 1.4, 0.68),
            None,
        );
    }

    let salt_path = "models/salt_shaker.glb#Scene0";
    let salt: Handle<Scene> = asset_server.load(salt_path);
    for (x, z) in [(-9.5_f32, 22.5_f32), (9.5_f32, 22.5_f32)] {
        commands.spawn((
            LevelEntity,
            SceneRoot(salt.clone()),
            SceneRetryPath::new(salt_path),
            Transform::from_xyz(x, 0.0, z),
        ));
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(x, 0.65, z),
            Vec3::new(0.48, 0.65, 0.48),
            None,
        );
    }

    // 3. Zone 2: North Kitchen Counter, Giant Toaster, Giant Stove & Refrigerator Goal
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(-14.0, -41.0),
        0.0,
        6.0,
        56.0,
        12.0,
        wood_col,
        cab_col,
    );
    // Sage tiled backsplash behind North Kitchen Counter
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(-14.0, 3.6, -47.2),
        Vec3::new(56.0, 7.2, 0.45),
        tile_col,
        0.22,
        0.0,
    );
    // Stepping Stools up to North Kitchen Countertop
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(-14.0, -29.5),
        -3.85,
        2.15,
        5.2,
        3.6,
        table_col,
        cab_col,
    );
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(-14.0, -33.2),
        -1.85,
        4.15,
        5.2,
        3.6,
        table_col,
        cab_col,
    );

    let toaster_path = "models/giant_toaster.glb#Scene0";
    let toaster_scene: Handle<Scene> = asset_server.load(toaster_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(toaster_scene),
        SceneRetryPath::new(toaster_path),
        Transform::from_xyz(-32.0, 0.0, -40.5).with_scale(Vec3::splat(1.25)),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-32.0, 0.925, -40.5),
        Vec3::new(1.5, 0.925, 1.2),
        None,
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-32.0, 2.7, -45.2),
        Vec3::new(4.2, 0.16, 1.3),
        Some((wood_col, 0.45, 0.0)),
    );

    let stove_path = "models/giant_stove.glb#Scene0";
    let stove_scene: Handle<Scene> = asset_server.load(stove_path);
    commands.spawn((
        LevelEntity,
        SceneRoot(stove_scene),
        SceneRetryPath::new(stove_path),
        Transform::from_xyz(-4.0, 0.0, -40.5),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-4.0, 0.55, -40.5),
        Vec3::new(2.2, 0.55, 1.8),
        None,
    );

    // Towering Stainless-Steel Refrigerator Goal on the Apartment Floor (opens at 12/12 Adult Size!)
    let fridge_path = "models/giant_fridge.glb#Scene0";
    let fridge_scene: Handle<Scene> = asset_server.load(fridge_path);
    commands.spawn((
        LevelEntity,
        FridgeGoal,
        SceneRoot(fridge_scene),
        SceneRetryPath::new(fridge_path),
        Transform::from_xyz(26.0, -6.0, -41.0),
    ));
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(26.0, 0.6, -41.7),
        Vec3::new(3.4, 6.6, 2.0),
        None,
    );

    // 4. Zone 3: West Living Room & Gaming Lounge (Sofa, Coffee Table, Entertainment Center & TV)
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-44.0, -4.8, -5.0),
        Vec3::new(4.2, 1.2, 11.0),
        Some((sofa_col, 0.85, 0.0)),
    );
    spawn_solid_box(
        commands,
        meshes,
        materials,
        Vec3::new(-47.6, -3.2, -5.0),
        Vec3::new(0.8, 2.8, 11.0),
        Some((sofa_col, 0.85, 0.0)),
    );
    for z_arm in [-15.4_f32, 5.4_f32] {
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(-44.0, -3.6, z_arm),
            Vec3::new(4.2, 2.0, 0.7),
            Some((sofa_col, 0.85, 0.0)),
        );
    }
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(-32.0, -5.0),
        -3.8,
        2.2,
        6.4,
        13.0,
        table_col,
        cab_col,
    );
    // TV Console & Big Screen TV against West Wall
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(-53.2, -5.0),
        -3.6,
        2.4,
        3.2,
        18.0,
        cab_col,
        cab_col,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(-54.4, -0.2, -5.0),
        Vec3::new(0.35, 5.8, 11.8),
        Color::srgb(0.05, 0.06, 0.08),
        0.2,
        0.4,
    );
    spawn_visual_box(
        commands,
        meshes,
        materials,
        Vec3::new(-54.15, -0.2, -5.0),
        Vec3::new(0.10, 5.1, 11.0),
        Color::srgb(0.12, 0.24, 0.42),
        0.15,
        0.0,
    );

    // 5. Zone 4: East Dining Room (Giant Dining Table & Chairs)
    spawn_counter_or_stool(
        commands,
        meshes,
        materials,
        Vec2::new(38.0, -4.0),
        -1.2,
        4.8,
        10.0,
        16.0,
        table_col,
        cab_col,
    );
    for (cz, back_z) in [(6.2_f32, 7.8_f32), (-14.2_f32, -15.8_f32)] {
        spawn_counter_or_stool(
            commands,
            meshes,
            materials,
            Vec2::new(38.0, cz),
            -3.2,
            2.8,
            4.0,
            3.6,
            table_col,
            cab_col,
        );
        spawn_solid_box(
            commands,
            meshes,
            materials,
            Vec3::new(38.0, -1.7, back_z),
            Vec3::new(2.0, 1.5, 0.22),
            Some((table_col, 0.45, 0.0)),
        );
    }

    // 6. Zone 5: Randomized Floor Obstacles on the 1,200 Sq Ft Apartment Floor (Y = -6.0)
    let cheese_pool = [
        (-8.5_f32, 8.5_f32),
        (9.2, 6.0),
        (-16.0, -2.5),
        (15.5, -3.0),
        (-10.5, -11.5),
        (10.5, -15.0),
        (-22.0, -18.0),
        (21.0, -19.5),
        (-11.0, -25.5),
        (12.0, -27.5),
        (-23.5, 14.5),
        (24.0, 13.5),
        (-36.5, 18.0),
        (42.0, 20.0),
    ];
    let cheese_count = rng.usize_range(8, 13).min(cheese_pool.len());
    for (n, idx) in rng
        .shuffle_indices(cheese_pool.len())
        .into_iter()
        .take(cheese_count)
        .enumerate()
    {
        let (bx, bz) = cheese_pool[idx];
        let pos = Vec3::new(
            bx + rng.range(-0.9, 0.9),
            -6.0,
            bz + rng.range(-0.9, 0.9),
        );
        spawn_ricos_cheese_cup(
            commands,
            meshes,
            materials,
            pos,
            rng.range(0.0, PI * 2.0),
            rng.range(0.95, 1.32),
            n % 3 == 1,
        );
    }

    let controller_pool = [
        (-4.5_f32, 4.2_f32),
        (5.8, 1.5),
        (-19.5, 5.5),
        (19.0, 6.5),
        (-6.2, -6.5),
        (-14.5, -13.5),
        (14.5, -12.0),
        (-26.0, -6.0),
        (26.5, -5.5),
        (5.0, -24.5),
        (-29.0, -22.0),
    ];
    let ctrl_count = rng.usize_range(6, 10).min(controller_pool.len());
    for idx in rng
        .shuffle_indices(controller_pool.len())
        .into_iter()
        .take(ctrl_count)
    {
        let (bx, bz) = controller_pool[idx];
        let pos = Vec3::new(
            bx + rng.range(-0.8, 0.8),
            -6.0,
            bz + rng.range(-0.8, 0.8),
        );
        spawn_game_controller_obstacle(
            commands,
            meshes,
            materials,
            pos,
            rng.range(0.0, PI * 2.0),
            rng.range(1.0, 1.28),
        );
    }

    // Always place one clear HeadphoneHazard right ahead of the starter stairs (Z = 9.4), plus randomized extras
    spawn_floor_headphones(
        commands,
        meshes,
        materials,
        Vec3::new(0.0, -6.0, 9.4),
        0.15,
        1.05,
    );
    let headphone_pool = [
        (-10.5_f32, 2.0_f32),
        (10.8, -1.5),
        (-6.8, -17.5),
        (8.5, -21.0),
        (-24.5, -11.0),
        (24.5, -9.0),
        (-16.5, 16.5),
        (17.5, -26.5),
    ];
    let extra_hp = rng.usize_range(4, 7).min(headphone_pool.len());
    for idx in rng
        .shuffle_indices(headphone_pool.len())
        .into_iter()
        .take(extra_hp)
    {
        let (bx, bz) = headphone_pool[idx];
        let pos = Vec3::new(
            bx + rng.range(-0.7, 0.7),
            -6.0,
            bz + rng.range(-0.7, 0.7),
        );
        spawn_floor_headphones(
            commands,
            meshes,
            materials,
            pos,
            rng.range(0.0, PI * 2.0),
            rng.range(0.95, 1.18),
        );
    }

    // 7. 12 Randomized Golden Chicken Tendies guiding Stan across the 1,200 Sq Ft Apartment (17cm -> 180cm Adult!)
    let tendy_path = "models/golden_tendy.glb#Scene0";
    let tendy_scene: Handle<Scene> = asset_server.load(tendy_path);

    // Zone A: South Breakfast Island & Tiered Platforms (pick 4 randomized spots)
    let mut zone_a_spots: Vec<(f32, f32, f32)> = Vec::with_capacity(4);
    let starter_island_pool = [
        (0.0_f32, 0.4_f32, 25.5_f32, 2.0_f32, 1.0_f32),
        (-4.0, 0.4, 30.2, 1.6, 1.1),
        (4.0, 0.4, 30.2, 1.6, 1.1),
    ];
    let pick_start = rng.usize_range(0, starter_island_pool.len() - 1);
    let (sx, sy, sz, jx, jz) = starter_island_pool[pick_start];
    zone_a_spots.push((
        sx + rng.range(-jx, jx),
        sy,
        sz + rng.range(-jz, jz),
    ));

    let island_extras_pool = [
        (-6.5_f32, 1.35_f32, 26.5_f32, 0.45_f32, 0.35_f32),
        (6.5, 1.35, 25.0, 0.45, 0.35),
        (0.0, 2.35, 22.8, 0.55, 0.55),
        (-11.0, 0.75, 30.0, 0.55, 0.55),
        (11.0, 0.75, 29.5, 0.55, 0.55),
        (0.0, -1.2, 18.0, 1.2, 0.5),
        (-18.5, -1.2, 25.0, 0.8, 0.5),
        (18.5, -1.2, 25.0, 0.8, 0.5),
    ];
    for idx in rng.shuffle_indices(island_extras_pool.len()).into_iter().take(3) {
        let (bx, by, bz, rjx, rjz) = island_extras_pool[idx];
        zone_a_spots.push((bx + rng.range(-rjx, rjx), by, bz + rng.range(-rjz, rjz)));
    }

    // Zone B: Open 1,200 Sq Ft Apartment Floor, West Living Room & East Dining (pick 5 randomized spots)
    let zone_b_pool = [
        (-14.0_f32, -5.2_f32, 6.0_f32, 2.4_f32, 2.2_f32),
        (14.5, -5.2, 4.5, 2.4, 2.2),
        (0.0, -5.2, -5.0, 3.0, 3.0),
        (-19.0, -5.2, -16.0, 2.6, 2.6),
        (19.0, -5.2, -15.0, 2.6, 2.6),
        (24.0, -5.2, 12.0, 2.4, 2.2),
        (-24.0, -5.2, 12.0, 2.4, 2.2),
        (0.0, -5.2, -22.5, 3.0, 2.2),
        (-32.0, -3.1, -5.0, 1.5, 3.2),
        (-44.0, -2.9, -5.0, 1.5, 4.0),
        (38.0, -0.5, -4.0, 2.2, 4.0),
        (38.0, -2.5, -14.2, 0.7, 0.5),
    ];
    let mut zone_b_spots: Vec<(f32, f32, f32)> = Vec::with_capacity(5);
    for idx in rng.shuffle_indices(zone_b_pool.len()).into_iter().take(5) {
        let (bx, by, bz, rjx, rjz) = zone_b_pool[idx];
        zone_b_spots.push((bx + rng.range(-rjx, rjx), by, bz + rng.range(-rjz, rjz)));
    }

    // Zone C: North Kitchen Countertop & Fridge Guard (pick 2 counter spots + 1 randomized fridge-zone tendy)
    let zone_c_pool = [
        (-32.0_f32, 2.45_f32, -40.5_f32, 0.45_f32, 0.35_f32),
        (-32.0, 3.3, -45.2, 1.5, 0.35),
        (-21.0, 0.45, -40.0, 2.5, 2.0),
        (-14.0, -1.15, -33.2, 1.1, 0.5),
        (-4.0, 1.75, -40.5, 0.8, 0.45),
        (6.5, 0.45, -40.0, 2.5, 2.0),
        (10.0, 0.45, -37.2, 1.4, 1.0),
    ];
    let mut zone_c_spots: Vec<(f32, f32, f32)> = Vec::with_capacity(3);
    for idx in rng.shuffle_indices(zone_c_pool.len()).into_iter().take(2) {
        let (bx, by, bz, rjx, rjz) = zone_c_pool[idx];
        zone_c_spots.push((bx + rng.range(-rjx, rjx), by, bz + rng.range(-rjz, rjz)));
    }
    zone_c_spots.push((
        26.0 + rng.range(-5.5, 5.5),
        -5.2,
        -32.5 + rng.range(-2.5, 3.0),
    ));

    let scales = [1.0_f32, 1.02, 1.05, 1.08, 1.14, 1.20, 1.26, 1.32, 1.38, 1.48, 1.60, 1.80];
    let all_tendy_spots = zone_a_spots
        .into_iter()
        .chain(zone_b_spots)
        .chain(zone_c_spots);
    for (i, (x, y, z)) in all_tendy_spots.enumerate() {
        let scale = scales[i.min(scales.len() - 1)];
        commands.spawn((
            LevelEntity,
            SceneRoot(tendy_scene.clone()),
            SceneRetryPath::new(tendy_path),
            Transform::from_xyz(x, y, z).with_scale(Vec3::splat(scale)),
            TendyPickup {
                base_y: y,
                phase: i as f32 * 0.7,
                is_final_goal: i == 11,
            },
        ));
    }

    // Power-ups: Honey Mustard (speed) & Ranch Shield across Starter Island & Apartment Floor
    let mustard_path = "models/honey_mustard_cup.glb#Scene0";
    let mustard_scene: Handle<Scene> = asset_server.load(mustard_path);
    for (i, (x, y, z)) in [(-10.5_f32, 0.4_f32, 27.5_f32), (16.0_f32, -5.4_f32, 10.0_f32)]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            LevelEntity,
            SceneRoot(mustard_scene.clone()),
            SceneRetryPath::new(mustard_path),
            Transform::from_xyz(x, y, z),
            PowerupPickup {
                kind: PowerupKind::HoneyMustard,
                base_y: y,
                phase: i as f32 * 1.1,
            },
        ));
    }

    let ranch_path = "models/ranch_shield.glb#Scene0";
    let ranch_scene: Handle<Scene> = asset_server.load(ranch_path);
    for (i, (x, y, z)) in [(10.5_f32, 0.4_f32, 27.5_f32), (-16.0_f32, -5.4_f32, -15.0_f32)]
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            LevelEntity,
            SceneRoot(ranch_scene.clone()),
            SceneRetryPath::new(ranch_path),
            Transform::from_xyz(x, y, z),
            PowerupPickup {
                kind: PowerupKind::RanchShield,
                base_y: y,
                phase: 1.5 + i as f32 * 1.1,
            },
        ));
    }
}
