use std::f32::consts::PI;
use bevy::prelude::*;
use crate::enemies::{spawn_enemy_at, Enemy, EnemyKind};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlayerStats};
use crate::ui::HudOverlay;

#[derive(Component)]
pub struct CutsceneEntity;

#[derive(Component)]
pub struct CutsceneMailman;

#[derive(Component)]
pub struct CutscenePackage;

#[derive(Component)]
pub struct CutsceneMom;

#[derive(Component)]
pub struct CutsceneEsxiLed(pub f32);

#[derive(Component)]
pub struct CutsceneEyeReflection;

/// Marker attached once Stan's Viking helmet/axe/shield have been parented onto `plaid_hero.glb`.
/// Checked every frame by `ensure_hero_vikings_gear` because GLTF `SceneRoot` children spawn
/// asynchronously after `spawn_player`.
#[derive(Component)]
pub struct HeroCosmetic;

#[derive(Component)]
pub struct HeroSaberBlade;

#[derive(Component)]
pub struct HeroRageAuraRoot;

#[derive(Component)]
pub struct HeroRageShell;

#[derive(Component)]
pub struct HeroRageRing(pub f32);

#[derive(Component)]
pub struct HeroRageSpark {
    pub angle: f32,
    pub height: f32,
    pub speed: f32,
}

#[derive(Component)]
pub struct HeroRageVein;

#[derive(Component)]
pub struct HeroRageMaterial {
    pub orig_color: Color,
    pub orig_emissive: LinearRgba,
}

#[derive(Component)]
pub struct CutsceneOverlay;

#[derive(Component)]
pub struct CutsceneBeatTitle;

#[derive(Component)]
pub struct CutsceneSubtitleText;

#[derive(Component)]
pub struct BossHudNode;

#[derive(Component)]
pub struct BossHpBarFill;

#[derive(Component)]
pub struct BossQuoteText;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MomState {
    Pursue,
    SaberWindup,
    SaberSlash,
    Staggered,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BossTendyVariety {
    CrispyGolden,
    HoneyGlazed,
    SpicyBuffalo,
}

#[derive(Component)]
pub struct BossHealingTendy {
    pub kind: BossTendyVariety,
    pub base_y: f32,
    pub phase: f32,
}

const BOSS_TENDY_SPOTS: [Vec3; 8] = [
    Vec3::new(-18.0, -5.1, 10.0),
    Vec3::new(18.0, -5.1, 10.0),
    Vec3::new(-24.0, -5.1, -12.0),
    Vec3::new(24.0, -5.1, -12.0),
    Vec3::new(-10.0, -5.1, -24.0),
    Vec3::new(10.0, -5.1, -24.0),
    Vec3::new(-28.0, -5.1, 2.0),
    Vec3::new(28.0, -5.1, 2.0),
];

fn spawn_boss_healing_tendy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    kind: BossTendyVariety,
    pos: Vec3,
    phase: f32,
) {
    let (ring_col, emissive, scale_mul) = match kind {
        BossTendyVariety::CrispyGolden => (
            Color::srgba(1.0, 0.84, 0.22, 0.82),
            LinearRgba::new(2.4, 1.7, 0.25, 1.0),
            1.30,
        ),
        BossTendyVariety::HoneyGlazed => (
            Color::srgba(0.24, 0.96, 0.52, 0.85),
            LinearRgba::new(0.4, 3.2, 1.1, 1.0),
            1.48,
        ),
        BossTendyVariety::SpicyBuffalo => (
            Color::srgba(1.0, 0.34, 0.12, 0.86),
            LinearRgba::new(3.6, 0.85, 0.15, 1.0),
            1.36,
        ),
    };
    let tendy_scene: Handle<Scene> = asset_server.load("models/golden_tendy.glb#Scene0");
    let ring_mat = materials.add(StandardMaterial {
        base_color: ring_col,
        emissive,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            BossHealingTendy {
                kind,
                base_y: pos.y,
                phase,
            },
            LevelEntity,
            Transform::from_translation(pos).with_scale(Vec3::splat(scale_mul)),
            Visibility::default(),
        ))
        .with_children(|p| {
            p.spawn(SceneRoot(tendy_scene));
            p.spawn((
                Mesh3d(meshes.add(Torus::new(0.10, 0.85))),
                MeshMaterial3d(ring_mat),
                Transform::from_xyz(0.0, -0.35, 0.0),
            ));
        });
}

#[derive(Component)]
pub struct MomBoss {
    pub hp: f32,
    pub max_hp: f32,
    pub state: MomState,
    pub timer: f32,
    pub voice_timer: f32,
    pub summon_timer: f32,
    pub tendy_respawn_timer: f32,
    pub tendy_spawn_seq: usize,
    pub quote_idx: usize,
    pub walk_phase: f32,
    pub lunge_dir: Vec3,
    pub hit_cooldown: f32,
    pub dealt_slash_hit: bool,
}

#[derive(Component, Clone, Copy)]
pub struct MomPart(pub &'static str);

const MOM_QUOTES: [&str; 5] = [
    "STAN! YOU MUST EAT YOUR VEGGIES!",
    "NO MORE CHICKEN TENDIES UNTIL YOUR PLATE IS GREEN!",
    "TAKE OFF THAT SILLY VIKINGS ONESIE RIGHT NOW!",
    "MY BROCCOLI LEGION WILL TEACH YOU NUTRITION!",
    "DROP THAT LIGHTSABER AND CHEW YOUR CARROTS!",
];

/// Spawn festive Christmas-themed Minnesota Vikings onesie overlay + glowing lightsaber on Stan.
pub fn ensure_hero_vikings_gear(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stats: Res<PlayerStats>,
    state: Res<State<GameState>>,
    existing_q: Query<Entity, With<HeroCosmetic>>,
    part_q: Query<(Entity, &Name), Without<HeroCosmetic>>,
) {
    let should_wear = matches!(
        *state.get(),
        GameState::BossFight | GameState::MomVictoryCutscene | GameState::CityIntroCutscene | GameState::CityPlay
    ) || (*state.get() == GameState::Cutscene && stats.cutscene_beat >= 6)
        || (*state.get() == GameState::HeadphoneCutscene && stats.boss_mode);
    if !should_wear || !existing_q.is_empty() {
        return;
    }

    let mut torso_ent = None;
    let mut head_ent = None;
    let mut r_arm_ent = None;
    for (ent, name) in &part_q {
        match name.as_str() {
            "HeroTorso" => torso_ent = Some(ent),
            "HeroHead" => head_ent = Some(ent),
            "HeroRightArm" => r_arm_ent = Some(ent),
            _ => {}
        }
    }
    let (Some(torso), Some(head), Some(r_arm)) = (torso_ent, head_ent, r_arm_ent) else {
        return;
    };

    let xmas_green = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.44, 0.16),
        perceptual_roughness: 0.72,
        ..default()
    });
    let xmas_red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.10, 0.14),
        perceptual_roughness: 0.72,
        ..default()
    });
    let fur_white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.97, 0.96, 0.94),
        perceptual_roughness: 0.9,
        ..default()
    });
    let vikings_purple = materials.add(StandardMaterial {
        base_color: Color::srgb(0.31, 0.15, 0.51),
        perceptual_roughness: 0.38,
        metallic: 0.15,
        ..default()
    });
    let vikings_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.78, 0.10),
        emissive: LinearRgba::new(0.45, 0.30, 0.02, 1.0),
        metallic: 0.65,
        perceptual_roughness: 0.25,
        ..default()
    });
    let hilt_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.22, 0.23, 0.28),
        metallic: 0.85,
        perceptual_roughness: 0.2,
        ..default()
    });
    let saber_core = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.92, 1.0),
        emissive: LinearRgba::new(2.2, 1.5, 4.5, 1.0),
        unlit: true,
        ..default()
    });
    let saber_aura = materials.add(StandardMaterial {
        base_color: Color::srgba(0.62, 0.28, 1.0, 0.42),
        emissive: LinearRgba::new(1.1, 0.4, 2.4, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // Attach Christmas + Vikings Onesie overlay and chest/back crests directly to HeroTorso
    commands.entity(torso).with_children(|t| {
        // Festive green onesie tunic shell conforming cleanly outside HeroTorso plaid bounds + white fur trim
        t.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Sphere::new(0.44))),
            MeshMaterial3d(xmas_green.clone()),
            Transform::from_xyz(0.0, 0.29, 0.0).with_scale(Vec3::new(1.05, 1.22, 0.89)),
        ));
        t.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Sphere::new(0.27))),
            MeshMaterial3d(xmas_green.clone()),
            Transform::from_xyz(0.0, 0.63, -0.01).with_scale(Vec3::new(1.64, 0.78, 0.98)),
        ));
        t.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Cylinder::new(0.445, 0.13))),
            MeshMaterial3d(xmas_red.clone()),
            Transform::from_xyz(0.0, 0.48, 0.0).with_scale(Vec3::new(1.05, 1.0, 0.89)),
        ));
        t.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Torus::new(0.055, 0.34))),
            MeshMaterial3d(fur_white.clone()),
            Transform::from_xyz(0.0, 0.72, 0.0),
        ));
        t.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Cylinder::new(0.445, 0.09))),
            MeshMaterial3d(vikings_gold.clone()),
            Transform::from_xyz(0.0, 0.14, 0.0).with_scale(Vec3::new(1.05, 1.0, 0.89)),
        ));

        // Vikings Horned Shield Emblem on BOTH front (-Z) and back (+Z) so visible from 3rd-person cam!
        for z_dir in [-1.0_f32, 1.0_f32] {
            let z_off = z_dir * 0.395;
            t.spawn((
                LevelEntity,
                HeroCosmetic,
                Mesh3d(meshes.add(Cylinder::new(0.185, 0.03))),
                MeshMaterial3d(vikings_gold.clone()),
                Transform::from_xyz(0.0, 0.35, z_off)
                    .with_rotation(Quat::from_rotation_x(PI * 0.5)),
            ));
            t.spawn((
                LevelEntity,
                HeroCosmetic,
                Mesh3d(meshes.add(Cylinder::new(0.155, 0.038))),
                MeshMaterial3d(vikings_purple.clone()),
                Transform::from_xyz(0.0, 0.35, z_off + z_dir * 0.008)
                    .with_rotation(Quat::from_rotation_x(PI * 0.5)),
            ));
            // Gold & white Viking Horns left and right of badge
            for sx in [-1.0_f32, 1.0_f32] {
                t.spawn((
                    LevelEntity,
                    HeroCosmetic,
                    Mesh3d(meshes.add(Cone::new(0.045, 0.19))),
                    MeshMaterial3d(fur_white.clone()),
                    Transform::from_xyz(sx * 0.16, 0.43, z_off + z_dir * 0.02)
                        .with_rotation(Quat::from_rotation_z(-sx * 0.65)),
                ));
                t.spawn((
                    LevelEntity,
                    HeroCosmetic,
                    Mesh3d(meshes.add(Sphere::new(0.052))),
                    MeshMaterial3d(vikings_gold.clone()),
                    Transform::from_xyz(sx * 0.11, 0.37, z_off + z_dir * 0.02),
                ));
            }
        }
    });

    // Attach Festive Christmas-Viking Horned Hat on HeroHead
    commands.entity(head).with_children(|h| {
        h.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Torus::new(0.055, 0.28))),
            MeshMaterial3d(fur_white.clone()),
            Transform::from_xyz(0.0, 0.38, 0.02),
        ));
        h.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Cone::new(0.28, 0.46))),
            MeshMaterial3d(xmas_red.clone()),
            Transform::from_xyz(0.0, 0.60, 0.02),
        ));
        h.spawn((
            LevelEntity,
            HeroCosmetic,
            Mesh3d(meshes.add(Sphere::new(0.075))),
            MeshMaterial3d(fur_white.clone()),
            Transform::from_xyz(0.0, 0.84, 0.02),
        ));
        for sx in [-1.0_f32, 1.0_f32] {
            h.spawn((
                LevelEntity,
                HeroCosmetic,
                Mesh3d(meshes.add(Cone::new(0.055, 0.24))),
                MeshMaterial3d(vikings_gold.clone()),
                Transform::from_xyz(sx * 0.25, 0.50, 0.02)
                    .with_rotation(Quat::from_rotation_z(-sx * 0.85)),
            ));
        }
    });

    // Attach glowing purple-gold Lightsaber to Stan's HeroRightArm hand position
    commands.entity(r_arm).with_children(|arm| {
        arm.spawn((
            LevelEntity,
            HeroCosmetic,
            HeroSaberBlade,
            Transform::from_xyz(0.23, -0.46, -0.06)
                .with_rotation(Quat::from_rotation_x(-1.18)),
        ))
        .with_children(|saber| {
            // Metallic hilt
            saber.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.036, 0.26))),
                MeshMaterial3d(hilt_mat),
                Transform::from_xyz(0.0, 0.05, 0.0),
            ));
            saber.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.05, 0.04))),
                MeshMaterial3d(vikings_gold),
                Transform::from_xyz(0.0, 0.18, 0.0),
            ));
            // Inner blade core
            saber.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.032, 1.48))),
                MeshMaterial3d(saber_core),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            // Outer energy glow sheath
            saber.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.072, 1.54))),
                MeshMaterial3d(saber_aura),
                Transform::from_xyz(0.0, 0.92, 0.0),
            ));
            saber.spawn(PointLight {
                color: Color::srgb(0.68, 0.38, 1.0),
                intensity: 38000.0,
                range: 14.0,
                shadows_enabled: false,
                ..default()
            });
        });
    });
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    size: Vec3,
    color: Color,
    emissive: Option<LinearRgba>,
) {
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::from_size(size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            emissive: emissive.unwrap_or(LinearRgba::BLACK),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_translation(pos),
    ));
}

#[derive(Clone, Copy)]
enum GundamStyle {
    Rx78,
    WingZero,
    RedComet,
}

#[derive(Clone, Copy)]
enum AnimeFigurineStyle {
    TwinTailMagical,
    SpikyShonen,
    CyberIdol,
}

/// Attach tiny emissive ESXi DCUI + vSphere reflections onto Stan's HeroHead pupils during cutscene shots.
pub fn ensure_hero_eye_reflections(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    state: Res<State<GameState>>,
    existing_q: Query<Entity, With<CutsceneEyeReflection>>,
    head_q: Query<(Entity, &Name), Without<CutsceneEyeReflection>>,
) {
    if *state.get() != GameState::Cutscene || !existing_q.is_empty() {
        return;
    }

    let Some((head_ent, _)) = head_q.iter().find(|(_, n)| n.as_str() == "HeroHead") else {
        return;
    };

    let screen_navy = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.58, 0.98),
        emissive: LinearRgba::new(0.25, 0.90, 1.85, 1.0),
        unlit: true,
        ..default()
    });
    let esxi_gold = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.16),
        emissive: LinearRgba::new(2.4, 1.75, 0.14, 1.0),
        unlit: true,
        ..default()
    });
    let vsphere_cyan = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.95, 1.0),
        emissive: LinearRgba::new(0.45, 2.1, 2.7, 1.0),
        unlit: true,
        ..default()
    });
    let gleam_white = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(3.2, 3.2, 3.2, 1.0),
        unlit: true,
        ..default()
    });

    commands.entity(head_ent).with_children(|head| {
        head.spawn((
            CutsceneEntity,
            CutsceneEyeReflection,
            Visibility::Visible,
            Transform::IDENTITY,
        ))
        .with_children(|refl| {
            // Eye centers in HeroHead local space sit at (sx * 0.10, 0.32, -0.317)
            for sx in [-1.0_f32, 1.0_f32] {
                let ex = sx * 0.10;
                refl.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.022, 0.020, 0.004)))),
                    MeshMaterial3d(screen_navy.clone()),
                    Transform::from_xyz(ex, 0.320, -0.318),
                ));
                refl.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.020, 0.0055, 0.004)))),
                    MeshMaterial3d(esxi_gold.clone()),
                    Transform::from_xyz(ex, 0.327, -0.320),
                ));
                refl.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.015, 0.004, 0.004)))),
                    MeshMaterial3d(vsphere_cyan.clone()),
                    Transform::from_xyz(ex - 0.002, 0.318, -0.320),
                ));
                refl.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.0045))),
                    MeshMaterial3d(gleam_white.clone()),
                    Transform::from_xyz(ex + 0.007, 0.328, -0.321),
                ));
            }
        });
    });
}

fn spawn_gundam_mecha(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    style: GundamStyle,
) {
    let white = materials.add(StandardMaterial {
        base_color: match style {
            GundamStyle::RedComet => Color::srgb(0.88, 0.20, 0.22),
            _ => Color::srgb(0.95, 0.96, 0.98),
        },
        perceptual_roughness: 0.32,
        metallic: 0.15,
        ..default()
    });
    let chest_mat = materials.add(StandardMaterial {
        base_color: match style {
            GundamStyle::Rx78 => Color::srgb(0.12, 0.30, 0.85),
            GundamStyle::WingZero => Color::srgb(0.10, 0.22, 0.62),
            GundamStyle::RedComet => Color::srgb(0.45, 0.08, 0.12),
        },
        perceptual_roughness: 0.35,
        ..default()
    });
    let accent_red = materials.add(StandardMaterial {
        base_color: match style {
            GundamStyle::WingZero => Color::srgb(0.18, 0.42, 0.92),
            GundamStyle::RedComet => Color::srgb(0.20, 0.20, 0.24),
            GundamStyle::Rx78 => Color::srgb(0.86, 0.14, 0.18),
        },
        perceptual_roughness: 0.35,
        ..default()
    });
    let gold = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.80, 0.15),
        emissive: LinearRgba::new(0.35, 0.25, 0.02, 1.0),
        metallic: 0.65,
        perceptual_roughness: 0.25,
        ..default()
    });
    let gunmetal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.18, 0.22),
        metallic: 0.72,
        perceptual_roughness: 0.35,
        ..default()
    });
    let visor_glow = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: match style {
            GundamStyle::RedComet => LinearRgba::new(3.2, 0.4, 1.6, 1.0),
            _ => LinearRgba::new(0.4, 3.0, 1.8, 1.0),
        },
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            CutsceneEntity,
            LevelEntity,
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|g| {
            // Stand plinth
            g.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.32, 0.03))),
                MeshMaterial3d(gunmetal.clone()),
                Transform::from_xyz(0.0, 0.015, 0.0),
            ));
            // Legs, boots & angular knee armor
            for sx in [-1.0_f32, 1.0_f32] {
                let lx = sx * 0.11;
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.12, 0.08, 0.22)))),
                    MeshMaterial3d(accent_red.clone()),
                    Transform::from_xyz(lx, 0.07, -0.02),
                ));
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.10, 0.44, 0.12)))),
                    MeshMaterial3d(white.clone()),
                    Transform::from_xyz(lx, 0.33, 0.0),
                ));
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.09, 0.11, 0.05)))),
                    MeshMaterial3d(white.clone()),
                    Transform::from_xyz(lx, 0.38, -0.075),
                ));
            }
            // Waist skirt & chest core
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.34, 0.12, 0.19)))),
                MeshMaterial3d(white.clone()),
                Transform::from_xyz(0.0, 0.60, 0.0),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.10, 0.10, 0.21)))),
                MeshMaterial3d(accent_red.clone()),
                Transform::from_xyz(0.0, 0.60, -0.01),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.38, 0.28, 0.22)))),
                MeshMaterial3d(chest_mat.clone()),
                Transform::from_xyz(0.0, 0.80, 0.0),
            ));
            // Chest twin vents / WingZero zero-system orb
            for sx in [-1.0_f32, 1.0_f32] {
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.09, 0.06, 0.04)))),
                    MeshMaterial3d(gold.clone()),
                    Transform::from_xyz(sx * 0.10, 0.83, -0.11),
                ));
            }
            if matches!(style, GundamStyle::WingZero) {
                g.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.048))),
                    MeshMaterial3d(visor_glow.clone()),
                    Transform::from_xyz(0.0, 0.80, -0.11),
                ));
            }
            // Flared shoulder pauldrons + arms
            for sx in [-1.0_f32, 1.0_f32] {
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.18, 0.16, 0.20)))),
                    MeshMaterial3d(white.clone()),
                    Transform::from_xyz(sx * 0.27, 0.88, 0.0)
                        .with_rotation(Quat::from_rotation_z(sx * -0.22)),
                ));
                g.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.09, 0.30, 0.10)))),
                    MeshMaterial3d(white.clone()),
                    Transform::from_xyz(sx * 0.28, 0.68, -0.02),
                ));
            }
            // Left Heater Shield + Right Beam Rifle
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.04, 0.42, 0.24)))),
                MeshMaterial3d(accent_red.clone()),
                Transform::from_xyz(-0.36, 0.72, -0.05)
                    .with_rotation(Quat::from_rotation_y(0.28)),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.05, 0.08, 0.44)))),
                MeshMaterial3d(gunmetal.clone()),
                Transform::from_xyz(0.30, 0.64, -0.22)
                    .with_rotation(Quat::from_rotation_x(0.20)),
            ));
            // Head Helmet + Glowing Visor + Iconic V-Fin / Commander Horn
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.20, 0.20, 0.20)))),
                MeshMaterial3d(white.clone()),
                Transform::from_xyz(0.0, 1.05, 0.0),
            ));
            g.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.15, 0.045, 0.04)))),
                MeshMaterial3d(visor_glow.clone()),
                Transform::from_xyz(0.0, 1.05, -0.095),
            ));
            match style {
                GundamStyle::RedComet => {
                    g.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.03, 0.18, 0.04)))),
                        MeshMaterial3d(gold),
                        Transform::from_xyz(0.0, 1.19, -0.07)
                            .with_rotation(Quat::from_rotation_x(-0.35)),
                    ));
                }
                _ => {
                    for sx in [-1.0_f32, 1.0_f32] {
                        g.spawn((
                            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.18, 0.035, 0.03)))),
                            MeshMaterial3d(gold.clone()),
                            Transform::from_xyz(sx * 0.08, 1.16, -0.10)
                                .with_rotation(Quat::from_rotation_z(sx * 0.46)),
                        ));
                    }
                }
            }
            // Backpack / Beam Sabers / Wing Zero Angel Wings
            match style {
                GundamStyle::WingZero => {
                    for sx in [-1.0_f32, 1.0_f32] {
                        g.spawn((
                            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.56, 0.16, 0.03)))),
                            MeshMaterial3d(white.clone()),
                            Transform::from_xyz(sx * 0.34, 1.02, 0.14)
                                .with_rotation(Quat::from_rotation_z(sx * 0.48) * Quat::from_rotation_y(sx * 0.22)),
                        ));
                        g.spawn((
                            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.42, 0.12, 0.03)))),
                            MeshMaterial3d(white.clone()),
                            Transform::from_xyz(sx * 0.26, 0.78, 0.15)
                                .with_rotation(Quat::from_rotation_z(sx * -0.45)),
                        ));
                    }
                }
                GundamStyle::Rx78 => {
                    for sx in [-1.0_f32, 1.0_f32] {
                        g.spawn((
                            Mesh3d(meshes.add(Cylinder::new(0.025, 0.26))),
                            MeshMaterial3d(visor_glow.clone()),
                            Transform::from_xyz(sx * 0.11, 1.06, 0.13)
                                .with_rotation(Quat::from_rotation_z(sx * 0.22)),
                        ));
                    }
                }
                GundamStyle::RedComet => {
                    g.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.22, 0.22, 0.10)))),
                        MeshMaterial3d(gunmetal),
                        Transform::from_xyz(0.0, 0.84, 0.14),
                    ));
                }
            }
        });
}

fn spawn_anime_figurine(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    scale: f32,
    style: AnimeFigurineStyle,
) {
    let base_mat = materials.add(StandardMaterial {
        base_color: match style {
            AnimeFigurineStyle::CyberIdol => Color::srgb(0.10, 0.82, 0.95),
            AnimeFigurineStyle::SpikyShonen => Color::srgb(0.95, 0.65, 0.12),
            AnimeFigurineStyle::TwinTailMagical => Color::srgb(0.95, 0.36, 0.72),
        },
        emissive: match style {
            AnimeFigurineStyle::CyberIdol => LinearRgba::new(0.1, 0.9, 1.3, 1.0),
            AnimeFigurineStyle::SpikyShonen => LinearRgba::new(1.1, 0.55, 0.05, 1.0),
            AnimeFigurineStyle::TwinTailMagical => LinearRgba::new(1.1, 0.2, 0.65, 1.0),
        },
        metallic: 0.4,
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.98, 0.84, 0.74),
        perceptual_roughness: 0.45,
        ..default()
    });
    let hair_color = match style {
        AnimeFigurineStyle::TwinTailMagical => Color::srgb(0.98, 0.84, 0.22),
        AnimeFigurineStyle::SpikyShonen => Color::srgb(0.09, 0.08, 0.12),
        AnimeFigurineStyle::CyberIdol => Color::srgb(0.12, 0.92, 0.88),
    };
    let hair_mat = materials.add(StandardMaterial {
        base_color: hair_color,
        emissive: if matches!(style, AnimeFigurineStyle::CyberIdol) {
            LinearRgba::new(0.05, 0.35, 0.35, 1.0)
        } else {
            LinearRgba::BLACK
        },
        perceptual_roughness: 0.40,
        ..default()
    });
    let outfit_main = materials.add(StandardMaterial {
        base_color: match style {
            AnimeFigurineStyle::TwinTailMagical => Color::srgb(0.16, 0.45, 0.95),
            AnimeFigurineStyle::SpikyShonen => Color::srgb(0.96, 0.40, 0.08),
            AnimeFigurineStyle::CyberIdol => Color::srgb(0.88, 0.90, 0.96),
        },
        perceptual_roughness: 0.42,
        ..default()
    });
    let outfit_trim = materials.add(StandardMaterial {
        base_color: match style {
            AnimeFigurineStyle::TwinTailMagical => Color::srgb(0.95, 0.16, 0.45),
            AnimeFigurineStyle::SpikyShonen => Color::srgb(0.12, 0.22, 0.65),
            AnimeFigurineStyle::CyberIdol => Color::srgb(0.58, 0.18, 0.92),
        },
        ..default()
    });
    let eye_iris = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.68, 0.98),
        emissive: LinearRgba::new(0.1, 0.35, 0.7, 1.0),
        ..default()
    });

    commands
        .spawn((
            CutsceneEntity,
            LevelEntity,
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(yaw))
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|f| {
            // Glossy acrylic collector plinth
            f.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.24, 0.04))),
                MeshMaterial3d(base_mat.clone()),
                Transform::from_xyz(0.0, 0.02, 0.0),
            ));
            // Legs
            for sx in [-1.0_f32, 1.0_f32] {
                f.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.042, 0.40))),
                    MeshMaterial3d(if matches!(style, AnimeFigurineStyle::SpikyShonen) {
                        outfit_main.clone()
                    } else {
                        skin.clone()
                    }),
                    Transform::from_xyz(sx * 0.065, 0.24, 0.0),
                ));
            }
            // Pleated Skirt or Martial Gi Sash
            if !matches!(style, AnimeFigurineStyle::SpikyShonen) {
                f.spawn((
                    Mesh3d(meshes.add(Cone::new(0.19, 0.22))),
                    MeshMaterial3d(outfit_main.clone()),
                    Transform::from_xyz(0.0, 0.49, 0.0),
                ));
            }
            // Torso + Ribbon / Belt + Arms
            f.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.095, 0.30))),
                MeshMaterial3d(outfit_main.clone()),
                Transform::from_xyz(0.0, 0.66, 0.0),
            ));
            f.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.16, 0.08, 0.14)))),
                MeshMaterial3d(outfit_trim.clone()),
                Transform::from_xyz(0.0, 0.57, 0.0),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                f.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.035, 0.26))),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(sx * 0.13, 0.68, -0.02)
                        .with_rotation(Quat::from_rotation_z(sx * 0.45)),
                ));
            }
            // Anime Head + Large Stylized Eyes
            f.spawn((
                Mesh3d(meshes.add(Sphere::new(0.15))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 0.94, 0.0),
            ));
            for sx in [-1.0_f32, 1.0_f32] {
                f.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.032))),
                    MeshMaterial3d(eye_iris.clone()),
                    Transform::from_xyz(sx * 0.055, 0.94, -0.138),
                ));
            }
            // Distinct Anime Hair + Iconic Prop
            f.spawn((
                Mesh3d(meshes.add(Sphere::new(0.158))),
                MeshMaterial3d(hair_mat.clone()),
                Transform::from_xyz(0.0, 0.99, 0.055),
            ));
            match style {
                AnimeFigurineStyle::TwinTailMagical | AnimeFigurineStyle::CyberIdol => {
                    for sx in [-1.0_f32, 1.0_f32] {
                        f.spawn((
                            Mesh3d(meshes.add(Cone::new(0.075, 0.48))),
                            MeshMaterial3d(hair_mat.clone()),
                            Transform::from_xyz(sx * 0.19, 0.84, 0.05)
                                .with_rotation(Quat::from_rotation_z(sx * 2.85)),
                        ));
                    }
                    // Magical star wand or cyber microphone
                    f.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.015, 0.34))),
                        MeshMaterial3d(base_mat.clone()),
                        Transform::from_xyz(0.22, 0.82, -0.10)
                            .with_rotation(Quat::from_rotation_z(-0.25)),
                    ));
                    f.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.055))),
                        MeshMaterial3d(base_mat),
                        Transform::from_xyz(0.26, 0.99, -0.10),
                    ));
                }
                AnimeFigurineStyle::SpikyShonen => {
                    for angle in [-0.75_f32, -0.35, 0.0, 0.35, 0.75] {
                        f.spawn((
                            Mesh3d(meshes.add(Cone::new(0.065, 0.24))),
                            MeshMaterial3d(hair_mat.clone()),
                            Transform::from_xyz(angle * 0.14, 1.15, 0.02)
                                .with_rotation(Quat::from_rotation_z(-angle * 0.7)),
                        ));
                    }
                }
            }
        });
}

pub fn enter_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    cosmetic_q: Query<Entity, With<HeroCosmetic>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    for e in &cosmetic_q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::None;
    }

    let start_t = crate::audio::take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;
    stats.boss_mode = false;
    stats.hint_text.clear();

    crate::audio::set_touch_playing(false);
    crate::audio::set_touch_boss_mode(false);
    crate::audio::play_cutscene_music();
    crate::audio::play_sfx("thunder");

    // Dimly lit, spooky atmosphere for Stan's bedroom backstory
    commands.insert_resource(ClearColor(Color::srgb(0.015, 0.01, 0.035)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.26, 0.20, 0.46),
        brightness: 46.0,
    });

    let base_y = 40.0_f32;
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(-0.3, base_y, -1.60);
        p_tf.rotation = Quat::IDENTITY;
        p_tf.scale = Vec3::splat(1.35);
        player.velocity = Vec3::ZERO;
        player.grounded = true;
        player.roll_timer = 0.0;
        player.attack_timer = 0.0;
    }

    // Spooky dark bedroom floor, back/side walls, rug, and bed
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y - 0.15, 0.0),
        Vec3::new(16.0, 0.3, 14.0),
        Color::srgb(0.06, 0.05, 0.09),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y + 3.2, -5.0),
        Vec3::new(16.0, 6.8, 0.3),
        Color::srgb(0.07, 0.06, 0.12),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-6.8, base_y + 3.2, 0.0),
        Vec3::new(0.3, 6.8, 14.0),
        Color::srgb(0.06, 0.05, 0.10),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y + 0.03, -0.5),
        Vec3::new(6.8, 0.05, 5.2),
        Color::srgb(0.24, 0.09, 0.16),
        None,
    );
    // Cozy Bed on left
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-4.5, base_y + 0.45, -2.5),
        Vec3::new(2.6, 0.9, 4.2),
        Color::srgb(0.22, 0.14, 0.10),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-4.5, base_y + 0.95, -2.2),
        Vec3::new(2.4, 0.35, 3.6),
        Color::srgb(0.20, 0.12, 0.36),
        None,
    );

    // Work-from-home Desk + Detailed Dual VMware ESXi DCUI & vSphere Monitors
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y + 1.0, -3.2),
        Vec3::new(4.8, 0.16, 1.8),
        Color::srgb(0.24, 0.16, 0.11),
        None,
    );
    // Left Monitor: VMware ESXi 8 DCUI Console
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-0.98, base_y + 1.85, -3.55),
        Vec3::new(1.65, 1.08, 0.10),
        Color::srgb(0.06, 0.06, 0.08),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-0.98, base_y + 2.20, -3.48),
        Vec3::new(1.50, 0.24, 0.03),
        Color::srgb(0.98, 0.78, 0.08),
        Some(LinearRgba::new(1.4, 1.05, 0.08, 1.0)),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-0.98, base_y + 1.72, -3.48),
        Vec3::new(1.50, 0.66, 0.03),
        Color::srgb(0.07, 0.14, 0.25),
        Some(LinearRgba::new(0.14, 0.42, 0.72, 1.0)),
    );
    for (row_idx, w) in [0.55_f32, 0.42, 0.50, 0.34].iter().enumerate() {
        let ry = base_y + 1.92 - (row_idx as f32) * 0.11;
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(-1.32, ry, -3.455),
            Vec3::new(0.42, 0.04, 0.02),
            Color::srgb(0.85, 0.92, 1.0),
            Some(LinearRgba::new(0.9, 1.0, 1.1, 1.0)),
        );
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(-0.62, ry, -3.455),
            Vec3::new(*w, 0.04, 0.02),
            Color::srgb(0.98, 0.82, 0.20),
            Some(LinearRgba::new(1.1, 0.85, 0.15, 1.0)),
        );
    }

    // Right Monitor: VMware vSphere Cluster Telemetry
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.98, base_y + 1.85, -3.55),
        Vec3::new(1.65, 1.08, 0.10),
        Color::srgb(0.06, 0.06, 0.08),
        None,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.98, base_y + 2.20, -3.48),
        Vec3::new(1.50, 0.22, 0.03),
        Color::srgb(0.10, 0.55, 0.92),
        Some(LinearRgba::new(0.16, 0.85, 1.45, 1.0)),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.98, base_y + 1.72, -3.48),
        Vec3::new(1.50, 0.68, 0.03),
        Color::srgb(0.06, 0.10, 0.18),
        Some(LinearRgba::new(0.08, 0.24, 0.45, 1.0)),
    );
    for (idx, (bx, bw, col, em)) in [
        (0.55_f32, 0.52_f32, Color::srgb(0.18, 0.95, 0.52), LinearRgba::new(0.25, 1.4, 0.7, 1.0)),
        (0.62, 0.68, Color::srgb(0.22, 0.82, 1.0), LinearRgba::new(0.3, 1.2, 1.6, 1.0)),
        (0.52, 0.45, Color::srgb(0.98, 0.72, 0.16), LinearRgba::new(1.3, 0.8, 0.15, 1.0)),
        (1.35, 0.42, Color::srgb(0.68, 0.42, 1.0), LinearRgba::new(0.9, 0.45, 1.5, 1.0)),
    ]
    .iter()
    .enumerate()
    {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(*bx, base_y + 1.94 - (idx as f32) * 0.12, -3.455),
            Vec3::new(*bw, 0.055, 0.02),
            *col,
            Some(*em),
        );
    }

    // Monitor stands + RGB Mechanical Keyboard + Gaming Mouse on Desk
    for mx in [-0.98_f32, 0.98_f32] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(mx, base_y + 1.22, -3.60),
            Vec3::new(0.14, 0.36, 0.12),
            Color::srgb(0.12, 0.13, 0.16),
            None,
        );
    }
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-0.18, base_y + 1.09, -2.65),
        Vec3::new(1.02, 0.05, 0.34),
        Color::srgb(0.08, 0.09, 0.12),
        None,
    );
    for (kx, col, em) in [
        (-0.52_f32, Color::srgb(0.2, 0.9, 1.0), LinearRgba::new(0.25, 1.1, 1.5, 1.0)),
        (-0.18, Color::srgb(0.75, 0.3, 1.0), LinearRgba::new(0.9, 0.35, 1.4, 1.0)),
        (0.16, Color::srgb(1.0, 0.45, 0.2), LinearRgba::new(1.3, 0.45, 0.2, 1.0)),
    ] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(kx, base_y + 1.12, -2.65),
            Vec3::new(0.30, 0.02, 0.22),
            col,
            Some(em),
        );
    }
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.62, base_y + 1.10, -2.65),
        Vec3::new(0.14, 0.05, 0.22),
        Color::srgb(0.14, 0.85, 1.0),
        Some(LinearRgba::new(0.2, 0.95, 1.4, 1.0)),
    );

    // 2-Tier Collector Display Shelf above Bed / Left of Desk (Gundams & Anime Figurines)
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-3.40, base_y + 2.55, -3.98),
        Vec3::new(2.36, 2.25, 0.08),
        Color::srgb(0.10, 0.09, 0.16),
        None,
    );
    for sy in [base_y + 1.88, base_y + 2.86] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(-3.40, sy, -3.62),
            Vec3::new(2.26, 0.06, 0.66),
            Color::srgb(0.24, 0.16, 0.12),
            None,
        );
    }
    for sx in [-4.52_f32, -2.28_f32] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(sx, base_y + 2.65, -3.62),
            Vec3::new(0.08, 2.05, 0.66),
            Color::srgb(0.20, 0.14, 0.10),
            None,
        );
    }

    // Top Shelf: Wing Zero & Classic RX-78 Gundams (front -Z rotated by PI to face south +Z camera)
    spawn_gundam_mecha(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-3.88, base_y + 2.89, -3.56),
        PI + 0.18,
        0.54,
        GundamStyle::Rx78,
    );
    spawn_gundam_mecha(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-2.90, base_y + 2.89, -3.56),
        PI - 0.18,
        0.54,
        GundamStyle::WingZero,
    );
    // Bottom Shelf: 3 Anime Collector Figurines
    spawn_anime_figurine(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-4.05, base_y + 1.91, -3.56),
        PI + 0.22,
        0.55,
        AnimeFigurineStyle::TwinTailMagical,
    );
    spawn_anime_figurine(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-3.40, base_y + 1.91, -3.56),
        PI + 0.04,
        0.55,
        AnimeFigurineStyle::SpikyShonen,
    );
    spawn_anime_figurine(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-2.75, base_y + 1.91, -3.56),
        PI - 0.20,
        0.55,
        AnimeFigurineStyle::CyberIdol,
    );
    // Bonus Red Comet Chibi Mecha on Desk corner
    spawn_gundam_mecha(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(-2.02, base_y + 1.08, -2.68),
        PI + 0.42,
        0.38,
        GundamStyle::RedComet,
    );

    // Bare-metal ESXi Host Server Rack beside desk with blinking LEDs
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(2.82, base_y + 1.3, -3.1),
        Vec3::new(1.15, 2.6, 1.3),
        Color::srgb(0.09, 0.10, 0.13),
        None,
    );
    for row in 0..4 {
        let ry = base_y + 0.65 + (row as f32) * 0.45;
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(2.82, ry, -2.43),
            Vec3::new(0.95, 0.26, 0.04),
            Color::srgb(0.16, 0.18, 0.22),
            None,
        );
        commands.spawn((
            CutsceneEntity,
            LevelEntity,
            CutsceneEsxiLed(row as f32 * 0.8),
            Mesh3d(meshes.add(Sphere::new(0.06))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.1, 1.0, 0.45),
                emissive: LinearRgba::new(0.3, 2.5, 0.9, 1.0),
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(3.12, ry, -2.38),
        ));
    }

    // Eerie monitor cyan glow + spooky purple doorway backlight + collector shelf spot + close-up key light
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        PointLight {
            color: Color::srgb(0.32, 0.72, 1.0),
            intensity: 28000.0,
            range: 9.5,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, base_y + 2.35, -2.4),
    ));
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        PointLight {
            color: Color::srgb(0.55, 0.20, 0.90),
            intensity: 24000.0,
            range: 14.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(3.2, base_y + 3.4, 2.2),
    ));
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        PointLight {
            color: Color::srgb(0.95, 0.90, 1.0),
            intensity: 30000.0,
            range: 6.5,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.40, base_y + 3.15, -2.45),
    ));
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        PointLight {
            color: Color::srgb(0.88, 0.88, 1.0),
            intensity: 40000.0,
            range: 12.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-0.1, base_y + 3.8, 1.9),
    ));

    // Spooky Mailman carrying the Christmas Vikings Onesie Parcel (enters left during Beat 5)
    let mail_blue = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.30, 0.65),
        perceptual_roughness: 0.65,
        ..default()
    });
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.72, 0.58),
        perceptual_roughness: 0.5,
        ..default()
    });
    commands
        .spawn((
            CutsceneEntity,
            LevelEntity,
            CutsceneMailman,
            Transform::from_xyz(-10.0, base_y, -0.35).with_rotation(Quat::from_rotation_y(-PI * 0.5)),
        ))
        .with_children(|m| {
            m.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.38, 1.3))),
                MeshMaterial3d(mail_blue.clone()),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ));
            m.spawn((
                Mesh3d(meshes.add(Sphere::new(0.28))),
                MeshMaterial3d(skin),
                Transform::from_xyz(0.0, 1.72, 0.0),
            ));
            m.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.30, 0.16))),
                MeshMaterial3d(mail_blue),
                Transform::from_xyz(0.0, 1.94, 0.0),
            ));
            m.spawn((
                CutscenePackage,
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.68, 0.48, 0.56)))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.84, 0.12, 0.16),
                    emissive: LinearRgba::new(0.25, 0.08, 0.02, 1.0),
                    ..default()
                })),
                Transform::from_xyz(0.0, 1.05, -0.58),
            ))
            .with_children(|pkg| {
                pkg.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.71, 0.12, 0.59)))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.35, 0.16, 0.56),
                        emissive: LinearRgba::new(0.4, 0.25, 0.8, 1.0),
                        ..default()
                    })),
                    Transform::IDENTITY,
                ));
            });
        });

    // Mom enters in Beat 7 from right doorway wielding Crimson Lightsaber
    let mom_root = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(9.5, base_y, 0.35),
        1.55,
    );
    commands.entity(mom_root).insert((CutsceneEntity, CutsceneMom));

    // Clean Unskippable Letterbox UI + Subtitles (zero HUD / zero input buttons)
    commands
        .spawn((
            CutsceneOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(13.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.86)),
            ))
            .with_children(|top| {
                top.spawn((
                    CutsceneBeatTitle,
                    Text::new("CHAPTER I: 3:00 AM ON THE ESXi HOST"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextLayout::new_with_justify(JustifyText::Center),
                    TextColor(Color::srgb(0.78, 0.62, 1.0)),
                    Node {
                        max_width: Val::Percent(94.0),
                        ..default()
                    },
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(16.0)),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.88)),
            ))
            .with_children(|bot| {
                bot.spawn((
                    CutsceneSubtitleText,
                    Text::new("3:00 AM. In the dark solitude of his bedroom, Stan worked from home..."),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextLayout::new_with_justify(JustifyText::Center),
                    TextColor(Color::srgb(0.96, 0.94, 0.88)),
                    Node {
                        max_width: Val::Percent(94.0),
                        ..default()
                    },
                ));
            });
        });
}

fn cutscene_beat_script(beat: u8) -> (&'static str, &'static str, &'static str, &'static str) {
    match beat {
        1 => (
            "CHAPTER I: 3:00 AM ON THE ESXi HOST",
            "3:00 AM. In the dark solitude of his bedroom, Stan worked from home...",
            "In the dead of night, in a dimly lit bedroom, Stan worked tirelessly from home.",
            "narrator",
        ),
        2 => (
            "HYPERVISOR WORKLOADS: BARE METAL",
            "Virtual machines hummed across his VMware ESXi monitors and rack.",
            "Managing critical virtual machines across his glowing VMware ESXi screens and bare metal server rack.",
            "narrator",
        ),
        3 => (
            "HYPERSCALE FOCUS: MONITOR GLOW",
            "The cold glow of his ESXi screens reflected across Stan's focused eyes.",
            "The cold blue and gold glow of the hypervisor screens reflected sharply across Stan's tired eyes.",
            "narrator",
        ),
        4 => (
            "STAN'S SANCTUARY: GUNDAMS & FIGURES",
            "Surrounded by his prized Gundam models and anime figures, night grew late...",
            "Guarded by his prized Gundam model kits and anime figurines lining the shelves, midnight slipped by.",
            "narrator",
        ),
        5 => (
            "CHAPTER II: THE MAILMAN'S MYSTERIOUS PARCEL",
            "Until a mysterious mailman appeared with Stan's new Christmas Vikings Onesie!",
            "Until a mysterious mailman stepped out of the shadows, carrying Stan's newly ordered Christmas Vikings onesie!",
            "narrator",
        ),
        6 => (
            "CHAPTER III: AWAKENING OF THE VIKINGS ONESIE",
            "STAN: \"Whoa... Putting on this Vikings onesie unlocked Dodge & Roll AND a Lightsaber?!\"",
            "Whoa... putting on this Christmas Vikings onesie just unlocked ancient Dodge and Roll powers, and a Lightsaber?!",
            "stan",
        ),
        7 => (
            "FINAL BOSS: MOM STORMS IN!",
            "MOM: \"STAN! DROP THAT LIGHTSABER AND EAT YOUR VEGETABLES RIGHT NOW!\"",
            "Stan! Drop that lightsaber and eat your vegetables right this second!",
            "mom",
        ),
        _ => (
            "THE TENDY DUEL BEGINS",
            "STAN: \"Not until I finish my tendies, Mom! VIKINGS ONESIE... ENGAGE!\"",
            "Not until I finish my chicken tendies, Mom! Vikings onesie, ignite!",
            "stan",
        ),
    }
}

pub fn update_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<(&mut Transform, &mut Player), With<Player>>,
    mut mailman_q: Query<
        &mut Transform,
        (
            With<CutsceneMailman>,
            Without<Player>,
            Without<CutsceneMom>,
            Without<Camera3d>,
        ),
    >,
    mut mom_q: Query<
        &mut Transform,
        (
            With<CutsceneMom>,
            Without<Player>,
            Without<CutsceneMailman>,
            Without<Camera3d>,
        ),
    >,
    mut cam_q: Query<
        &mut Transform,
        (
            With<Camera3d>,
            Without<Player>,
            Without<CutsceneMailman>,
            Without<CutsceneMom>,
        ),
    >,
    mut led_q: Query<
        (&CutsceneEsxiLed, &mut Transform),
        (
            Without<Camera3d>,
            Without<Player>,
            Without<CutsceneMailman>,
            Without<CutsceneMom>,
        ),
    >,
    mut eye_refl_q: Query<&mut Visibility, With<CutsceneEyeReflection>>,
    mut title_q: Query<&mut Text, With<CutsceneBeatTitle>>,
    mut sub_q: Query<&mut Text, (With<CutsceneSubtitleText>, Without<CutsceneBeatTitle>)>,
) {
    const BEAT_ENDS: [f32; 8] = [5.5, 13.0, 20.0, 26.5, 33.5, 41.5, 47.5, 54.0];

    let dt = time.delta_secs().min(0.05);
    let mut t = stats.cutscene_timer + dt;

    let mut target_beat: u8 = 9;
    for (idx, &end_t) in BEAT_ENDS.iter().enumerate() {
        if t < end_t {
            target_beat = (idx as u8) + 1;
            break;
        }
    }

    // Wait cleanly for TTS to finish before advancing to the next beat or leaving cutscene
    if stats.cutscene_beat > 0 && target_beat > stats.cutscene_beat {
        if crate::audio::is_voice_speaking() {
            let hold_idx = (stats.cutscene_beat.saturating_sub(1) as usize).min(7);
            t = BEAT_ENDS[hold_idx] - 0.01;
            target_beat = stats.cutscene_beat;
        } else if target_beat == 9 {
            stats.cutscene_timer = t;
            next_state.set(GameState::BossFight);
            return;
        }
    } else if target_beat == 9 && !crate::audio::is_voice_speaking() {
        stats.cutscene_timer = t;
        next_state.set(GameState::BossFight);
        return;
    }

    stats.cutscene_timer = t;

    // Pulse ESXi rack activity LEDs
    for (led, mut tf) in &mut led_q {
        let pulse = 0.6 + 0.5 * ((t * 9.0 + led.0).sin().abs());
        tf.scale = Vec3::splat(pulse);
    }

    if target_beat != stats.cutscene_beat && target_beat <= 8 {
        stats.cutscene_beat = target_beat;
        let (title, sub, voice, role) = cutscene_beat_script(target_beat);
        if let Ok(mut txt) = title_q.get_single_mut() {
            txt.0 = title.to_string();
        }
        if let Ok(mut txt) = sub_q.get_single_mut() {
            txt.0 = sub.to_string();
        }
        if target_beat == 6 || target_beat == 8 {
            crate::audio::play_sfx("saber_ignite");
        } else if target_beat == 7 {
            crate::audio::play_sfx("mom_yell");
        }
        crate::audio::speak_voice(voice, role);
    }

    for mut vis in &mut eye_refl_q {
        *vis = if stats.cutscene_beat <= 4 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    // Animate Stan across beats 1..8
    let base_y = 40.0_f32;
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        player.velocity = Vec3::ZERO;
        match stats.cutscene_beat {
            1..=4 => {
                p_tf.translation = Vec3::new(-0.3, base_y, -1.60);
                p_tf.rotation = Quat::from_rotation_y((t * 1.4).sin() * 0.025);
                player.roll_timer = 0.0;
                player.attack_timer = 0.0;
            }
            5 => {
                p_tf.translation = Vec3::new(-0.15, base_y, -0.70);
                p_tf.rotation = Quat::from_rotation_y(PI * 0.55);
                player.roll_timer = 0.0;
                player.attack_timer = 0.0;
            }
            6 => {
                let p = (t - 33.5).clamp(0.0, 8.0);
                if p < 3.0 {
                    p_tf.translation =
                        Vec3::new((p * 1.9).sin() * 0.9, base_y + (p * 3.2).sin().abs() * 0.35, 0.20);
                    p_tf.rotation = Quat::from_rotation_y(PI + (p * 1.8).sin() * 0.45);
                    player.roll_timer = if (p % 1.4) < 0.42 { 0.42 - (p % 1.4) } else { 0.0 };
                    player.attack_timer = if (p % 1.4) > 0.65 && (p % 1.4) < 1.05 { 0.25 } else { 0.0 };
                } else {
                    // Plant heroically facing camera + raised saber guard while Stan speaks
                    p_tf.translation = Vec3::new(-0.30, base_y + (t * 8.0).sin().abs() * 0.018, 0.25);
                    p_tf.rotation = Quat::from_rotation_y(PI * 0.90);
                    player.roll_timer = 0.0;
                    player.attack_timer = 0.22;
                }
            }
            7 => {
                p_tf.translation = Vec3::new(-1.35, base_y, 0.45);
                p_tf.rotation = Quat::from_rotation_y(-PI * 0.55);
                player.roll_timer = 0.0;
                player.attack_timer = 0.0;
            }
            _ => {
                let p = (t - 47.5).clamp(0.0, 6.5);
                let bob = if p < 4.5 { (t * 10.0).sin().abs() * 0.02 } else { 0.0 };
                p_tf.translation = Vec3::new(-1.35, base_y + bob, 0.40);
                p_tf.rotation = Quat::from_rotation_y(-PI * 0.68);
                player.roll_timer = 0.0;
                player.attack_timer = if p < 4.5 { 0.21 } else { 0.0 };
            }
        }
    }

    // Mailman delivers parcel in Beat 5, then exits offstage for Beat 6+
    if let Ok(mut m_tf) = mailman_q.get_single_mut() {
        if stats.cutscene_beat == 5 {
            let walk_t = ((t - 26.5) / 2.6).clamp(0.0, 1.0);
            m_tf.translation = Vec3::new(-6.8 + walk_t * 4.9, base_y, -0.35);
            m_tf.rotation = Quat::from_rotation_y(-PI * 0.5);
        } else {
            m_tf.translation = Vec3::new(-12.0, base_y, -0.35);
        }
    }

    // Mom storms in from right doorway in Beat 7 & faces off in Beat 8
    if let Ok(mut mom_tf) = mom_q.get_single_mut() {
        if stats.cutscene_beat == 7 {
            let enter_t = ((t - 41.5) / 1.0).clamp(0.0, 1.0);
            let yell_bob = if enter_t >= 1.0 { (t * 15.0).sin().abs() * 0.03 } else { 0.0 };
            mom_tf.translation = Vec3::new(6.5 - enter_t * 4.95, base_y + yell_bob, 0.35);
            mom_tf.rotation = Quat::from_rotation_y(PI * 0.70);
        } else if stats.cutscene_beat == 8 {
            mom_tf.translation = Vec3::new(1.55, base_y, 0.35);
            mom_tf.rotation = Quat::from_rotation_y(PI * 0.55);
        } else {
            mom_tf.translation = Vec3::new(9.5, base_y, 0.35);
        }
    }

    // Choreographed 8-shot cinematic camera (pans, screen focus, eye reflections, shelf, speaker close-ups)
    if let Ok(mut c_tf) = cam_q.get_single_mut() {
        let (cam_pos, look_at) = match stats.cutscene_beat {
            1 => {
                let p = (t / 5.5).clamp(0.0, 1.0);
                (
                    Vec3::new(-3.2 + p * 1.3, base_y + 3.2 - p * 0.5, 2.7 - p * 1.5),
                    Vec3::new(0.1, base_y + 1.85, -2.5),
                )
            }
            2 => {
                let p = ((t - 5.5) / 7.5).clamp(0.0, 1.0);
                if p < 0.72 {
                    let s = p / 0.72;
                    (
                        Vec3::new(0.55 + s * 0.55, base_y + 2.04, -1.85),
                        Vec3::new(-0.15 + s * 0.85, base_y + 1.82, -3.48),
                    )
                } else {
                    let s = (p - 0.72) / 0.28;
                    (
                        Vec3::new(1.10 + s * 0.95, base_y + 1.96 - s * 0.12, -1.85 + s * 0.30),
                        Vec3::new(1.55 + s * 1.25, base_y + 1.65, -2.65),
                    )
                }
            }
            3 => {
                // Extreme macro zoom-in onto Stan's glowing ESXi eye reflections
                let p = ((t - 13.0) / 7.0).clamp(0.0, 1.0);
                (
                    Vec3::new(-0.44 + p * 0.18, base_y + 2.43 + p * 0.05, -2.66 + p * 0.12),
                    Vec3::new(-0.30, base_y + 2.484, -2.055),
                )
            }
            4 => {
                // Smooth collector pan across Anime Figurines & Gundams
                let p = ((t - 20.0) / 6.5).clamp(0.0, 1.0);
                (
                    Vec3::new(-4.05 + p * 1.30, base_y + 2.38 + p * 0.56, -1.30),
                    Vec3::new(-3.75 + p * 0.70, base_y + 2.55 + p * 0.32, -3.55),
                )
            }
            5 => {
                let p = ((t - 26.5) / 7.0).clamp(0.0, 1.0);
                (
                    Vec3::new(0.80 - p * 0.40, base_y + 2.45, 2.95 - p * 0.65),
                    Vec3::new(-1.05, base_y + 1.55, -0.45),
                )
            }
            6 => {
                let p = (t - 33.5).clamp(0.0, 8.0);
                if p < 3.0 {
                    let s = p / 3.0;
                    (
                        Vec3::new((p * 1.1).sin() * 2.1, base_y + 2.55 - s * 0.25, 4.1 - s * 0.9),
                        Vec3::new(-0.1, base_y + 1.55, 0.1),
                    )
                } else {
                    let s = ((p - 3.0) / 5.0).clamp(0.0, 1.0);
                    (
                        Vec3::new(-0.42 - s * 0.08, base_y + 2.34 - s * 0.04, 2.85 - s * 0.22),
                        Vec3::new(-0.25, base_y + 2.22, 0.25),
                    )
                }
            }
            7 => {
                // Dramatic villain zoom-in on Mom yelling while waving her Crimson Lightsaber
                let p = ((t - 41.5) / 6.0).clamp(0.0, 1.0);
                (
                    Vec3::new(0.10 + p * 0.20, base_y + 2.62 - p * 0.07, 2.40 - p * 0.50),
                    Vec3::new(1.52, base_y + 2.54, 0.35),
                )
            }
            _ => {
                // Stan talks back in intense close-up, then dramatic wide duel pull-out
                let p = ((t - 47.5) / 6.5).clamp(0.0, 1.0);
                if p < 0.70 {
                    let s = p / 0.70;
                    (
                        Vec3::new(-0.05 - s * 0.18, base_y + 2.46, 2.35 - s * 0.25),
                        Vec3::new(-1.32, base_y + 2.32, 0.40),
                    )
                } else {
                    let s = (p - 0.70) / 0.30;
                    let close_pos = Vec3::new(-0.23, base_y + 2.46, 2.10);
                    let close_look = Vec3::new(-1.32, base_y + 2.32, 0.40);
                    (
                        close_pos.lerp(Vec3::new(0.10, base_y + 2.70, 5.20), s),
                        close_look.lerp(Vec3::new(0.10, base_y + 1.80, 0.35), s),
                    )
                }
            }
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, Vec3::Y);
    }
}

pub fn exit_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<CutsceneEntity>>,
    ui_q: Query<Entity, With<CutsceneOverlay>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }
}

#[derive(Component)]
pub struct HeadphoneCutsceneEntity;

#[derive(Component)]
pub struct HeadphoneCutsceneOverlay;

#[derive(Component)]
pub struct HeadphoneSceneBadgeText;

#[derive(Component)]
pub struct HeadphoneSpeakerText;

#[derive(Component)]
pub struct HeadphoneSubtitleText;

#[derive(Component)]
pub struct HeadphoneUzumakiVortex(pub f32);

fn spawn_cracked_ipad(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    rot: Quat,
    scale: f32,
) {
    let bezel_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.57, 0.62),
        metallic: 0.72,
        perceptual_roughness: 0.28,
        ..default()
    });
    let screen_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.16, 0.30),
        emissive: LinearRgba::new(0.18, 0.42, 0.85, 1.0),
        perceptual_roughness: 0.15,
        ..default()
    });
    let crack_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(1.8, 2.2, 2.8, 1.0),
        unlit: true,
        ..default()
    });
    let boot_scuff = materials.add(StandardMaterial {
        base_color: Color::srgba(0.10, 0.06, 0.04, 0.85),
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    commands
        .spawn((
            HeadphoneCutsceneEntity,
            LevelEntity,
            Transform::from_translation(pos)
                .with_rotation(rot)
                .with_scale(Vec3::splat(scale)),
        ))
        .with_children(|pad| {
            // Pedestal glow base
            pad.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.85, 0.16))),
                MeshMaterial3d(bezel_mat.clone()),
                Transform::from_xyz(0.0, -0.92, 0.0),
            ));
            // Aluminum body + Home button + Glowing cracked screen
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(1.38, 1.82, 0.09)))),
                MeshMaterial3d(bezel_mat),
            ));
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(1.18, 1.50, 0.03)))),
                MeshMaterial3d(screen_mat),
                Transform::from_xyz(0.0, 0.04, 0.045),
            ));
            pad.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.055, 0.02))),
                MeshMaterial3d(crack_mat.clone()),
                Transform::from_xyz(0.0, -0.78, 0.055)
                    .with_rotation(Quat::from_rotation_x(PI * 0.5)),
            ));
            // Boot print impact mark
            pad.spawn((
                Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.32, 0.54, 0.02)))),
                MeshMaterial3d(boot_scuff),
                Transform::from_xyz(-0.04, 0.05, 0.062)
                    .with_rotation(Quat::from_rotation_z(-0.22)),
            ));
            // Spiderweb crack fractures across screen
            for (cx, cy, len, angle) in [
                (0.0_f32, 0.08_f32, 1.35_f32, 0.68_f32),
                (0.02, 0.06, 1.25, -0.82),
                (-0.18, 0.30, 0.72, 1.28),
                (0.24, -0.22, 0.78, 0.18),
                (-0.14, -0.34, 0.66, -0.25),
            ] {
                pad.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(len, 0.024, 0.025)))),
                    MeshMaterial3d(crack_mat.clone()),
                    Transform::from_xyz(cx, cy, 0.068)
                        .with_rotation(Quat::from_rotation_z(angle)),
                ));
            }
        });
}

fn spawn_cutscene_headphones(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
    snapped: bool,
) {
    let frame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.11, 0.15),
        perceptual_roughness: 0.35,
        metallic: 0.35,
        ..default()
    });
    let pad_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.25, 0.28),
        perceptual_roughness: 0.85,
        ..default()
    });
    let spark_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.52, 0.15),
        emissive: LinearRgba::new(2.4, 0.75, 0.15, 1.0),
        unlit: true,
        ..default()
    });

    commands
        .spawn((
            HeadphoneCutsceneEntity,
            LevelEntity,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        ))
        .with_children(|hp| {
            let split = if snapped { 0.35 } else { 0.0 };
            for sx in [-1.0_f32, 1.0_f32] {
                hp.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.32, 0.22))),
                    MeshMaterial3d(pad_mat.clone()),
                    Transform::from_xyz(sx * (0.48 + split), 0.12, sx * 0.12 * split)
                        .with_rotation(Quat::from_rotation_z(if snapped { sx * 1.15 } else { PI * 0.5 })),
                ));
                hp.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.42, 0.12, 0.12)))),
                    MeshMaterial3d(frame_mat.clone()),
                    Transform::from_xyz(sx * (0.28 + split * 0.6), 0.16, -0.35)
                        .with_rotation(Quat::from_rotation_y(sx * (0.45 + split))),
                ));
            }
            if snapped {
                for off in [Vec3::new(0.0, 0.24, -0.22), Vec3::new(-0.26, 0.18, 0.14), Vec3::new(0.32, 0.20, -0.05)] {
                    hp.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.055))),
                        MeshMaterial3d(spark_mat.clone()),
                        Transform::from_translation(off),
                    ));
                }
            }
        });
}

fn headphone_cutscene_script(
    beat: u8,
    rage_level: u8,
) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    if rage_level >= 3 {
        match beat {
            1 => (
                "UZUMAKI ABYSS #3+ | SCENE 1/4",
                "UNHINGED NARRATOR",
                "\"CRUNCH!! AGAIN?! THE SPIRAL ABYSS SWALLOWS STAN AS ANOTHER INNOCENT HEADSET EXPLODES UNDERFOOT!\"",
                "Crunch! Again?! The spiral abyss swallows Stan as another innocent headset explodes underfoot!",
                "narrator_cosmic",
            ),
            2 => (
                "UZUMAKI ABYSS #3+ | SCENE 2/4",
                "STAN (BEYOND SAVING)",
                "\"AM I LIVING IN AN ELECTRONIC LANDFILL?! WHY IS NINETY PERCENT OF MY FLOOR MADE OF IPADS AND HEADPHONES?!\"",
                "Am I living in an electronic landfill?! Why is ninety percent of my floor made of iPads and headphones?!",
                "stan_cosmic",
            ),
            3 => (
                "UZUMAKI ABYSS #3+ | SCENE 3/4",
                "STAN (SPIRAL DELIRIUM)",
                "\"I DO NOT EVEN HAVE SIX EARS! WHY DO I KEEP BUYING THEM AND DROPPING EVERY IPAD LIKE FLOOR TILE?!\"",
                "I do not even have six ears! Why do I keep buying them and dropping every iPad like floor tile?!",
                "stan_cosmic",
            ),
            _ => (
                "UZUMAKI ABYSS #3+ | SCENE 4/4",
                "STAN - COSMIC SPIRAL VOW",
                "\"NO MORE SPIRALS! NO MORE CRUSHED TECH! ON MY SWELLED RED SOUL, NOT ONE MORE HEADPHONE SHALL DIE!!\"",
                "No more spirals! No more crushed tech! On my swelled red soul, not one more headphone shall die!!",
                "stan_vow_3",
            ),
        }
    } else if rage_level == 2 {
        match beat {
            1 => (
                "UZUMAKI SPIRAL CRISIS | SCENE 1/4",
                "NARRATOR (SPIRAL DREAD)",
                "\"GASHUNK!! THE SPIRAL CURSE RETURNS! Stan has crushed YET ANOTHER pair of headphones into the floor!\"",
                "Gashunk!! The spiral curse returns! Stan has crushed yet another pair of headphones into the floor!",
                "narrator_uzumaki",
            ),
            2 => (
                "UZUMAKI SPIRAL CRISIS | SCENE 2/4",
                "STAN (EXISTENTIAL CRISIS)",
                "\"MY MIND IS SPIRALING! WHY DO I EVEN HAVE TWENTY PAIRS OF HEADPHONES?! WHO NEEDS THIS MANY HEADSETS?!\"",
                "My mind is spiraling! Why do I even have twenty pairs of headphones?! Who needs this many headsets?!",
                "stan_uzumaki",
            ),
            3 => (
                "UZUMAKI SPIRAL CRISIS | SCENE 3/4",
                "STAN (SPIRALING MADNESS)",
                "\"AND WHY DO I ACTUALLY LEAVE BOTH HEADPHONES AND IPADS ALL OVER THE FLOOR ALL THE TIME?! TABLES EXIST! DESKS EXIST!\"",
                "And why do I actually leave both headphones and iPads all over the floor all the time?! Tables exist! Desks exist!",
                "stan_uzumaki",
            ),
            _ => (
                "UZUMAKI SPIRAL CRISIS | SCENE 4/4",
                "STAN - UZUMAKI VOW",
                "\"BY THE TWISTING SPIRAL OF MADNESS - HEAR MY UNHINGED VOW! NEVER AGAIN SHALL AN IPAD OR HEADSET TOUCH THIS FLOOR!\"",
                "By the twisting spiral of madness, hear my unhinged vow! Never again shall an iPad or headset touch this floor!",
                "stan_vow_2",
            ),
        }
    } else {
        match beat {
            1 => (
                "HEADPHONE FLASHBACK | SCENE 1/4",
                "NARRATOR",
                "\"CRUNCH! Time freezes! Beneath Stan's foot lies a crushed pair of headphones... awakening his darkest memories!\"",
                "Crunch! Time freezes! Beneath Stan's foot lies a crushed pair of headphones, awakening his darkest memories.",
                "narrator",
            ),
            2 => (
                "HEADPHONE FLASHBACK | SCENE 2/4",
                "NARRATOR",
                "\"Every cracked iPad screen and snapped headset from his past rises before his eyes! So many fallen devices!\"",
                "Every cracked iPad screen and snapped headset from his past rises before his eyes. So many fallen devices!",
                "narrator",
            ),
            3 => (
                "HEADPHONE FLASHBACK | SCENE 3/4",
                "STAN (HEARTBROKEN)",
                "\"No, not again! Wait... why do I even own this many headphones, and why do I leave them on the floor?!\"",
                "No, not again! Wait, why do I even own this many headphones, and why do I leave them on the floor?",
                "stan",
            ),
            _ => (
                "HEADPHONE FLASHBACK | SCENE 4/4",
                "STAN - SACRED VOW",
                "\"Hear my sacred vow! From this day forth, I will NEVER step on another iPad or pair of headphones again!\"",
                "Hear my sacred vow! From this day forth, I will never step on another iPad or pair of headphones again!",
                "stan_vow_1",
            ),
        }
    }
}

pub fn enter_headphone_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
) {
    for mut node in &mut hud_q {
        node.display = Display::None;
    }

    let start_t = crate::audio::take_cutscene_start_time();
    stats.cutscene_timer = if start_t > 0.0 { start_t } else { 0.0 };
    stats.cutscene_beat = 0;
    stats.headphones_vow_taken = true;
    stats.enraged = true;
    stats.rage_level = stats.rage_level.saturating_add(1);
    stats.rage_swell_timer = 0.0;
    stats.rage_swell_ticks = stats.rage_swell_ticks.saturating_add(1);
    stats.rage_pulse_flash = 1.0;
    stats.rage_cooldown_timer = 10.0;
    stats.hint_text.clear();

    let uzumaki = stats.rage_level >= 2;
    let rage_u8 = stats.rage_level.min(255) as u8;
    crate::audio::set_touch_playing(false);
    crate::audio::play_sfx("headphone_crunch");
    crate::audio::play_sfx("thunder");
    crate::audio::play_headphone_music(rage_u8);
    crate::audio::set_uzumaki_mode(if uzumaki { rage_u8.saturating_sub(1) } else { 0 });

    // Moody dramatic memory void atmosphere (stark manga void in 2nd+ Uzumaki mode)
    commands.insert_resource(ClearColor(if uzumaki {
        Color::srgb(0.01, 0.01, 0.01)
    } else {
        Color::srgb(0.02, 0.015, 0.05)
    }));
    commands.insert_resource(AmbientLight {
        color: if uzumaki {
            Color::WHITE
        } else {
            Color::srgb(0.36, 0.28, 0.56)
        },
        brightness: if uzumaki { 160.0 } else { 90.0 },
    });

    let base_y = 120.0_f32;
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(0.0, base_y, 0.0);
        // Face +Z toward the camera so the viewer sees Stan's remorseful expression & Sacred Vow salute
        p_tf.rotation = Quat::from_rotation_y(PI);
        p_tf.scale = Vec3::splat(1.45);
        player.velocity = Vec3::ZERO;
        player.grounded = true;
        player.roll_timer = 0.0;
        player.attack_timer = 0.0;
    }

    // Dark memory floor & dramatic spotlights
    commands.spawn((
        HeadphoneCutsceneEntity,
        LevelEntity,
        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(26.0, 0.3, 22.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: if uzumaki {
                Color::srgb(0.02, 0.02, 0.02)
            } else {
                Color::srgb(0.04, 0.04, 0.08)
            },
            perceptual_roughness: 0.25,
            metallic: 0.35,
            ..default()
        })),
        Transform::from_xyz(0.0, base_y - 0.15, 0.0),
    ));
    let lights = if uzumaki {
        [
            (0.0_f32, base_y + 4.8, 3.6_f32, Color::WHITE, 210000.0),
            (-3.4, base_y + 3.6, -1.6, Color::srgb(0.92, 0.92, 0.92), 140000.0),
            (3.4, base_y + 3.6, -1.6, Color::srgb(0.92, 0.92, 0.92), 140000.0),
        ]
    } else {
        [
            (0.0_f32, base_y + 4.5, 3.2_f32, Color::srgb(0.45, 0.78, 1.0), 140000.0),
            (-3.2, base_y + 3.6, -1.8, Color::srgb(1.0, 0.28, 0.35), 110000.0),
            (3.2, base_y + 3.6, -1.8, Color::srgb(1.0, 0.82, 0.25), 110000.0),
        ]
    };
    for (lx, ly, lz, col, intensity) in lights {
        commands.spawn((
            HeadphoneCutsceneEntity,
            LevelEntity,
            PointLight {
                color: col,
                intensity,
                range: 26.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(lx, ly, lz),
        ));
    }

    // Crushed headphones directly beneath Stan's right foot
    spawn_cutscene_headphones(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.28, base_y + 0.02, 0.42),
        0.2,
        true,
    );

    // Graveyard of 5 Cracked iPads & 4 Snapped Past Headsets behind Stan (Z in [-3.4, -1.5])
    for (x, y, z, yaw, sc) in [
        (-4.0_f32, base_y + 1.05, -2.5_f32, 0.35_f32, 1.05_f32),
        (-2.0, base_y + 1.25, -3.1, 0.16, 1.18),
        (0.0, base_y + 1.45, -3.4, 0.0, 1.25),
        (2.0, base_y + 1.25, -3.1, -0.16, 1.18),
        (4.0, base_y + 1.05, -2.5, -0.35, 1.05),
    ] {
        spawn_cracked_ipad(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(x, y, z),
            Quat::from_rotation_y(yaw),
            sc,
        );
    }
    for (x, z, yaw) in [
        (-3.0_f32, -1.6_f32, 0.6_f32),
        (-1.1, -1.9, -0.4),
        (1.2, -1.9, 0.8),
        (3.1, -1.6, -0.5),
    ] {
        spawn_cutscene_headphones(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(x, base_y + 0.02, z),
            yaw,
            true,
        );
    }

    // 2nd+ trigger: Towering Junji Ito Uzumaki Spiral Vortex & orbiting tech graveyard behind Stan!
    if uzumaki {
        let ink_white = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(2.8, 2.8, 2.8, 1.0),
            unlit: true,
            ..default()
        });
        let ink_black = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::BLACK,
            unlit: true,
            ..default()
        });
        commands
            .spawn((
                HeadphoneCutsceneEntity,
                HeadphoneUzumakiVortex(2.2),
                LevelEntity,
                Transform::from_xyz(0.0, base_y + 2.35, -4.6),
            ))
            .with_children(|v| {
                for r in 0..10 {
                    let inner = 0.55 + (r as f32) * 0.55;
                    let outer = inner + 0.28;
                    let mat = if r % 2 == 0 {
                        ink_white.clone()
                    } else {
                        ink_black.clone()
                    };
                    v.spawn((
                        Mesh3d(meshes.add(Torus::new(inner, outer))),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(0.0, 0.0, (r as f32) * 0.02)
                            .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                    ));
                }
                for a in 0..16 {
                    let ang = (a as f32) * (std::f32::consts::TAU / 16.0);
                    v.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.14, 12.2, 0.06)))),
                        MeshMaterial3d(if a % 2 == 0 {
                            ink_white.clone()
                        } else {
                            ink_black.clone()
                        }),
                        Transform::from_xyz(ang.cos() * 0.25, ang.sin() * 0.25, -0.05)
                            .with_rotation(Quat::from_rotation_z(ang + 0.28)),
                    ));
                }
            });

        // Counter-rotating swirling spiral crossbars + orbiting cracked iPads & headsets
        commands
            .spawn((
                HeadphoneCutsceneEntity,
                HeadphoneUzumakiVortex(-3.2),
                LevelEntity,
                Transform::from_xyz(0.0, base_y + 2.35, -4.25),
            ))
            .with_children(|v| {
                for i in 0..8 {
                    let ang = (i as f32) * (std::f32::consts::TAU / 8.0);
                    let r = 1.6 + (i as f32) * 0.42;
                    v.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.24, 1.35, 0.08)))),
                        MeshMaterial3d(ink_white.clone()),
                        Transform::from_xyz(ang.cos() * r, ang.sin() * r, 0.0)
                            .with_rotation(Quat::from_rotation_z(ang - 0.55)),
                    ));
                }
            });

        for i in 0..6 {
            let ang = (i as f32) * (std::f32::consts::TAU / 6.0) + 0.3;
            let rad = 2.6 + (i as f32) * 0.45;
            spawn_cracked_ipad(
                &mut commands,
                &mut meshes,
                &mut materials,
                Vec3::new(ang.cos() * rad, base_y + 2.4 + ang.sin() * 1.4, -3.7),
                Quat::from_rotation_z(ang) * Quat::from_rotation_y(ang * 0.4),
                0.78,
            );
            spawn_cutscene_headphones(
                &mut commands,
                &mut meshes,
                &mut materials,
                Vec3::new((ang + 0.5).cos() * 3.4, base_y + 1.8 + (ang + 0.5).sin() * 1.2, -3.2),
                ang,
                true,
            );
        }
    }

    // Bottom cinematic flashback dialogue box
    let (badge, speaker, sub, _, _) = headphone_cutscene_script(1, stats.rage_level.min(255) as u8);
    commands
        .spawn((
            HeadphoneCutsceneOverlay,
            LevelEntity,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                padding: UiRect::bottom(Val::Px(22.0)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(92.0),
                    max_width: Val::Px(780.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                    border: UiRect::all(Val::Px(if uzumaki { 3.0 } else { 2.0 })),
                    ..default()
                },
                BackgroundColor(if uzumaki {
                    Color::srgba(0.02, 0.02, 0.02, 0.96)
                } else {
                    Color::srgba(0.04, 0.03, 0.09, 0.90)
                }),
                BorderColor(if uzumaki {
                    Color::WHITE
                } else {
                    Color::srgb(0.45, 0.82, 1.0)
                }),
                BorderRadius::all(Val::Px(if uzumaki { 4.0 } else { 12.0 })),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    width: Val::Percent(100.0),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        HeadphoneSpeakerText,
                        Text::new(speaker),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(if uzumaki {
                            Color::WHITE
                        } else {
                            Color::srgb(0.45, 0.88, 1.0)
                        }),
                    ));
                    row.spawn((
                        HeadphoneSceneBadgeText,
                        Text::new(badge),
                        TextFont {
                            font_size: 13.0,
                            ..default()
                        },
                        TextColor(if uzumaki {
                            Color::srgb(0.88, 0.88, 0.88)
                        } else {
                            Color::srgb(1.0, 0.72, 0.35)
                        }),
                    ));
                });
                card.spawn((
                    HeadphoneSubtitleText,
                    Text::new(sub),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });
}

pub fn update_headphone_cutscene(
    time: Res<Time>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<
        &mut Transform,
        (With<Player>, Without<Camera3d>, Without<HeadphoneUzumakiVortex>),
    >,
    mut cam_q: Query<
        &mut Transform,
        (With<Camera3d>, Without<Player>, Without<HeadphoneUzumakiVortex>),
    >,
    mut vortex_q: Query<
        (&HeadphoneUzumakiVortex, &mut Transform),
        (Without<Camera3d>, Without<Player>),
    >,
    mut badge_q: Query<&mut Text, With<HeadphoneSceneBadgeText>>,
    mut spk_q: Query<&mut Text, (With<HeadphoneSpeakerText>, Without<HeadphoneSceneBadgeText>)>,
    mut sub_q: Query<
        &mut Text,
        (
            With<HeadphoneSubtitleText>,
            Without<HeadphoneSceneBadgeText>,
            Without<HeadphoneSpeakerText>,
        ),
    >,
) {
    const BEAT_ENDS: [f32; 4] = [6.2, 12.8, 19.4, 26.0];
    let dt = time.delta_secs().min(0.05);
    let mut t = stats.cutscene_timer + dt;

    let mut target_beat: u8 = 5;
    for (idx, &end_t) in BEAT_ENDS.iter().enumerate() {
        if t < end_t {
            target_beat = (idx as u8) + 1;
            break;
        }
    }

    if stats.cutscene_beat > 0 && target_beat > stats.cutscene_beat {
        if crate::audio::is_voice_speaking() {
            let hold_idx = (stats.cutscene_beat.saturating_sub(1) as usize).min(3);
            t = BEAT_ENDS[hold_idx] - 0.01;
            target_beat = stats.cutscene_beat;
        } else if target_beat == 5 {
            stats.cutscene_timer = t;
            next_state.set(if stats.headphone_return_boss {
                GameState::BossFight
            } else {
                GameState::Playing
            });
            return;
        }
    } else if target_beat == 5 && !crate::audio::is_voice_speaking() {
        stats.cutscene_timer = t;
        next_state.set(if stats.headphone_return_boss {
            GameState::BossFight
        } else {
            GameState::Playing
        });
        return;
    }

    stats.cutscene_timer = t;

    if target_beat != stats.cutscene_beat && target_beat <= 4 {
        stats.cutscene_beat = target_beat;
        let (badge, speaker, sub, voice, role) =
            headphone_cutscene_script(target_beat, stats.rage_level.min(255) as u8);
        if let Ok(mut b) = badge_q.get_single_mut() {
            b.0 = badge.to_string();
        }
        if let Ok(mut sp) = spk_q.get_single_mut() {
            sp.0 = speaker.to_string();
        }
        if let Ok(mut sb) = sub_q.get_single_mut() {
            sb.0 = sub.to_string();
        }
        crate::audio::speak_voice(voice, role);
    }

    let uzumaki = stats.rage_level >= 2;
    for (v, mut v_tf) in &mut vortex_q {
        v_tf.rotate_z(dt * v.0);
    }

    if stats.cutscene_beat >= 3 || uzumaki {
        stats.rage_pulse_flash = (stats.rage_pulse_flash - dt * 2.2).max(0.0);
        stats.rage_swell_timer += dt;
        while stats.rage_swell_timer >= 0.5 {
            stats.rage_swell_timer -= 0.5;
            stats.rage_swell_ticks = stats.rage_swell_ticks.saturating_add(1);
            stats.rage_pulse_flash = 1.0;
            crate::audio::play_sfx("rage_swell");
        }
        if let Ok(mut p_tf) = player_q.get_single_mut() {
            let swell_s = 1.45
                + ((stats.rage_swell_ticks.saturating_sub(1)).min(6) as f32) * 0.14
                + stats.rage_pulse_flash * 0.16;
            let shake = if uzumaki {
                (t * 24.0).sin() * 0.08
            } else {
                0.0
            };
            p_tf.scale = Vec3::new(
                swell_s * (1.0 + stats.rage_pulse_flash * 0.08),
                swell_s * (1.0 + stats.rage_pulse_flash * 0.12),
                swell_s * (1.0 + stats.rage_pulse_flash * 0.08),
            );
            p_tf.rotation = Quat::from_rotation_y(PI + shake);
        }
    } else if let Ok(mut p_tf) = player_q.get_single_mut() {
        p_tf.scale = Vec3::splat(1.45);
    }

    let base_y = 120.0_f32;
    if let Ok(mut c_tf) = cam_q.get_single_mut() {
        let (cam_pos, look_at) = match stats.cutscene_beat {
            1 => {
                // Close-up on Stan's foot crushing the headphones
                let u = (t / 6.2).clamp(0.0, 1.0);
                (
                    Vec3::new(1.35 + u * 0.50, base_y + 0.65 + u * 0.30, 2.15 + u * 0.70),
                    Vec3::new(0.20, base_y + 0.25, 0.35),
                )
            }
            2 => {
                // Elevated sweeping pan across all 5 Cracked iPads & Uzumaki Spiral
                let u = ((t - 6.2) / 6.6).clamp(0.0, 1.0);
                (
                    Vec3::new(-4.0 + u * 8.0, base_y + 3.05, 5.2),
                    Vec3::new(-0.8 + u * 1.6, base_y + 1.85, -3.4),
                )
            }
            3 => {
                // Existential-crisis push-in on Stan's face with spiral & cracked iPads behind him
                let u = ((t - 12.8) / 6.6).clamp(0.0, 1.0);
                (
                    Vec3::new(-1.55 + u * 0.70, base_y + 2.45, 4.35 - u * 0.35),
                    Vec3::new(0.0, base_y + 2.10, -0.8),
                )
            }
            _ => {
                // Heroic / unhinged low-angle crane orbit as Stan swears his Sacred Vow
                let u = ((t - 19.4) / 6.6).clamp(0.0, 1.0);
                (
                    Vec3::new(2.5 - u * 1.3, base_y + 1.95 + u * 0.75, 5.4 - u * 0.3),
                    Vec3::new(0.0, base_y + 1.95, -0.9),
                )
            }
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, Vec3::Y);
        if uzumaki {
            c_tf.rotate_local_z((t * 1.8).sin() * 0.16);
        }
    }
}

pub fn exit_headphone_cutscene(
    mut commands: Commands,
    mut stats: ResMut<PlayerStats>,
    ent_q: Query<Entity, With<HeadphoneCutsceneEntity>>,
    ui_q: Query<Entity, With<HeadphoneCutsceneOverlay>>,
    mut hud_q: Query<&mut Node, With<HudOverlay>>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
) {
    crate::audio::set_uzumaki_mode(0);
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
    for mut node in &mut hud_q {
        node.display = Display::Flex;
    }

    let next_s = stats.saved_apartment_scale.max(stats.target_scale());
    stats.player_scale = next_s;
    stats.rage_cooldown_timer = 10.0;
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = stats.saved_apartment_pos;
        p_tf.rotation = Quat::from_rotation_y(stats.saved_apartment_yaw);
        p_tf.scale = Vec3::splat(next_s);
        player.velocity = Vec3::ZERO;
        player.facing_yaw = stats.saved_apartment_yaw;
        player.invuln_timer = 1.5;
        player.roll_timer = 0.0;
        player.attack_timer = 0.0;
    }

    if !stats.headphone_return_boss {
        commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
        commands.insert_resource(AmbientLight {
            color: Color::srgb(1.0, 0.96, 0.90),
            brightness: 620.0,
        });
        crate::audio::play_bgm();
        crate::audio::set_touch_playing(true);
        stats.hint_text = format!(
            "HEADPHONE RAGE (x{}, 10s)! Stan is emanating anger & swelling bright red every 0.5s!",
            stats.rage_level.max(1)
        );
    }
}

/// Emanating Crimson Rage Aura, rising anger sparks, shockwave rings & bright-red pulsing Stan materials.
pub fn update_enraged_visuals(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<State<GameState>>,
    stats: Res<PlayerStats>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_q: Query<(Entity, &Children), With<Player>>,
    children_q: Query<&Children>,
    name_q: Query<&Name>,
    saber_q: Query<(), With<HeroSaberBlade>>,
    mut aura_root_q: Query<&mut Visibility, With<HeroRageAuraRoot>>,
    mut vein_q: Query<
        (&mut Visibility, &mut Transform),
        (
            With<HeroRageVein>,
            Without<HeroRageAuraRoot>,
            Without<HeroRageShell>,
            Without<HeroRageRing>,
            Without<HeroRageSpark>,
        ),
    >,
    mut shell_q: Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        (
            With<HeroRageShell>,
            Without<HeroRageVein>,
            Without<HeroRageRing>,
            Without<HeroRageSpark>,
        ),
    >,
    mut ring_q: Query<
        (&HeroRageRing, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        (
            Without<HeroRageVein>,
            Without<HeroRageShell>,
            Without<HeroRageSpark>,
        ),
    >,
    mut spark_q: Query<
        (&HeroRageSpark, &mut Transform, &MeshMaterial3d<StandardMaterial>),
        (
            Without<HeroRageVein>,
            Without<HeroRageShell>,
            Without<HeroRageRing>,
        ),
    >,
    mut mat_q: Query<
        (Entity, &mut MeshMaterial3d<StandardMaterial>, Option<&HeroRageMaterial>),
        (
            Without<HeroRageAuraRoot>,
            Without<HeroRageShell>,
            Without<HeroRageRing>,
            Without<HeroRageSpark>,
            Without<HeroRageVein>,
        ),
    >,
) {
    let Ok((player_ent, root_children)) = player_q.get_single() else {
        return;
    };

    let active = (stats.enraged && matches!(*state.get(), GameState::Playing | GameState::BossFight))
        || (*state.get() == GameState::HeadphoneCutscene && stats.cutscene_beat >= 3);

    if active && aura_root_q.is_empty() {
        let shell_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.08, 0.04, 0.28),
            emissive: LinearRgba::new(3.2, 0.22, 0.05, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let ring_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.14, 0.04, 0.85),
            emissive: LinearRgba::new(4.8, 0.45, 0.08, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let spark_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.28, 0.05, 0.92),
            emissive: LinearRgba::new(5.5, 0.90, 0.10, 1.0),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let vein_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.06, 0.06),
            emissive: LinearRgba::new(5.2, 0.20, 0.15, 1.0),
            unlit: true,
            ..default()
        });

        commands.entity(player_ent).with_children(|p| {
            p.spawn((
                LevelEntity,
                HeroRageAuraRoot,
                Visibility::Inherited,
                Transform::IDENTITY,
            ))
            .with_children(|aura| {
                aura.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.10, 0.06),
                        intensity: 120000.0,
                        range: 18.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 1.35, 0.0),
                ));
                aura.spawn((
                    HeroRageShell,
                    Mesh3d(meshes.add(Sphere::new(0.58))),
                    MeshMaterial3d(shell_mat),
                    Transform::from_xyz(0.0, 0.95, 0.0).with_scale(Vec3::new(1.05, 1.35, 1.05)),
                ));
                for phase in [0.0_f32, 0.5_f32] {
                    aura.spawn((
                        HeroRageRing(phase),
                        Mesh3d(meshes.add(Torus::new(0.65, 0.78))),
                        MeshMaterial3d(ring_mat.clone()),
                        Transform::from_xyz(0.0, 0.25, 0.0).with_scale(Vec3::new(1.0, 0.35, 1.0)),
                    ));
                }
                for i in 0..8 {
                    let angle = (i as f32) * (std::f32::consts::TAU / 8.0);
                    let height = 0.25 + ((i % 3) as f32) * 0.45;
                    let speed = 2.6 + (i as f32) * 0.35;
                    aura.spawn((
                        HeroRageSpark {
                            angle,
                            height,
                            speed,
                        },
                        Mesh3d(meshes.add(Cone::new(0.08, 0.32))),
                        MeshMaterial3d(spark_mat.clone()),
                        Transform::from_xyz(angle.cos() * 0.65, height, angle.sin() * 0.65),
                    ));
                }
            });
        });

        // Attach crimson anime cross-vein popup on HeroHead forehead so fury is unmistakable!
        let mut head_ent = None;
        let mut stack: Vec<Entity> = root_children.iter().copied().collect();
        while let Some(e) = stack.pop() {
            if let Ok(name) = name_q.get(e) {
                if name.as_str() == "HeroHead" {
                    head_ent = Some(e);
                    break;
                }
            }
            if let Ok(kids) = children_q.get(e) {
                stack.extend(kids.iter().copied());
            }
        }
        if let Some(h_ent) = head_ent {
            commands.entity(h_ent).with_children(|head| {
                head.spawn((
                    LevelEntity,
                    HeroRageVein,
                    Visibility::Inherited,
                    Transform::from_xyz(0.14, 0.43, -0.34),
                ))
                .with_children(|v| {
                    for (vx, vy, rot_z) in [
                        (-0.045_f32, 0.025_f32, 0.55_f32),
                        (0.045, 0.025, -0.55),
                        (-0.045, -0.025, -0.55),
                        (0.045, -0.025, 0.55),
                    ] {
                        v.spawn((
                            Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.065, 0.018, 0.018)))),
                            MeshMaterial3d(vein_mat.clone()),
                            Transform::from_xyz(vx, vy, 0.0)
                                .with_rotation(Quat::from_rotation_z(rot_z)),
                        ));
                    }
                });
            });
        }
    }

    for mut vis in &mut aura_root_q {
        *vis = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    let t = time.elapsed_secs();
    let flash = stats.rage_pulse_flash.clamp(0.0, 1.0);
    for (mut vis, mut v_tf) in &mut vein_q {
        *vis = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let s = 1.0 + flash * 0.45 + (t * 16.0).sin().abs() * 0.18;
        v_tf.scale = Vec3::splat(s);
    }

    if active {
        for (mut s_tf, s_mat) in &mut shell_q {
            let pulse = 1.0 + flash * 0.28 + (t * 10.0).sin().abs() * 0.08;
            s_tf.scale = Vec3::new(1.05 * pulse, 1.35 * pulse, 1.05 * pulse);
            if let Some(m) = materials.get_mut(&s_mat.0) {
                m.base_color = Color::srgba(1.0, 0.06, 0.03, 0.22 + flash * 0.26);
                m.emissive = LinearRgba::new(2.8 + flash * 4.5, 0.18, 0.04, 1.0);
            }
        }
        for (ring, mut r_tf, r_mat) in &mut ring_q {
            let u = (t * 1.45 + ring.0).fract();
            let rad = 0.65 + u * 1.85;
            r_tf.translation.y = 0.12 + u * 0.85;
            r_tf.scale = Vec3::new(rad, 0.35, rad);
            if let Some(m) = materials.get_mut(&r_mat.0) {
                let alpha = (1.0 - u).clamp(0.0, 1.0) * (0.65 + flash * 0.35);
                m.base_color = Color::srgba(1.0, 0.12, 0.04, alpha);
                m.emissive = LinearRgba::new(4.2 + flash * 4.0, 0.35, 0.05, 1.0);
            }
        }
        for (sp, mut sp_tf, sp_mat) in &mut spark_q {
            let cycle = (t * sp.speed * 0.42 + sp.height).fract();
            let ang = sp.angle + t * sp.speed;
            let rad = 0.56 + (1.0 - cycle) * 0.34 + flash * 0.18;
            sp_tf.translation = Vec3::new(ang.cos() * rad, 0.18 + cycle * 1.85, ang.sin() * rad);
            sp_tf.rotation = Quat::from_rotation_y(-ang) * Quat::from_rotation_z((t * 12.0).sin() * 0.2);
            sp_tf.scale = Vec3::splat((0.75 + (1.0 - cycle) * 0.65) * (1.0 + flash * 0.4));
            if let Some(m) = materials.get_mut(&sp_mat.0) {
                m.emissive = LinearRgba::new(4.8 + flash * 4.0, 0.75, 0.08, 1.0);
            }
        }
    }

    // Clone player mesh materials on first activation so cached glTF handles never stay tinted on restart
    let mut clone_jobs: Vec<(Entity, StandardMaterial)> = Vec::new();
    let mut stack: Vec<Entity> = root_children.iter().copied().collect();
    while let Some(ent) = stack.pop() {
        if saber_q.contains(ent) {
            continue;
        }
        if let Ok(kids) = children_q.get(ent) {
            stack.extend(kids.iter().copied());
        }
        if let Ok((m_ent, mesh_mat, maybe_orig)) = mat_q.get_mut(ent) {
            if let Some(orig) = maybe_orig {
                if let Some(mat) = materials.get_mut(&mesh_mat.0) {
                    if active {
                        let beat_wave = ((t * 12.566).sin() * 0.5 + 0.5).max(flash);
                        mat.base_color = Color::srgb(
                            1.0,
                            0.06 + (1.0 - beat_wave) * 0.12,
                            0.04 + (1.0 - beat_wave) * 0.08,
                        );
                        mat.emissive = LinearRgba::new(
                            1.8 + beat_wave * 4.2,
                            0.05 + beat_wave * 0.22,
                            0.02,
                            1.0,
                        );
                    } else {
                        mat.base_color = orig.orig_color;
                        mat.emissive = orig.orig_emissive;
                    }
                }
            } else if active {
                if let Some(src) = materials.get(&mesh_mat.0) {
                    clone_jobs.push((m_ent, src.clone()));
                }
            }
        }
    }

    for (m_ent, cloned_src) in clone_jobs {
        let orig_color = cloned_src.base_color;
        let orig_emissive = cloned_src.emissive;
        let mut tinted = cloned_src;
        tinted.base_color = Color::srgb(1.0, 0.08, 0.05);
        tinted.emissive = LinearRgba::new(3.2, 0.10, 0.03, 1.0);
        let new_handle = materials.add(tinted);
        if let Ok((_, mut mesh_mat, _)) = mat_q.get_mut(m_ent) {
            mesh_mat.0 = new_handle;
        }
        commands.entity(m_ent).insert(HeroRageMaterial {
            orig_color,
            orig_emissive,
        });
    }
}

pub(crate) fn spawn_mom_mesh_hierarchy(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    scale: f32,
) -> Entity {
    let skin = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.74, 0.60),
        perceptual_roughness: 0.48,
        ..default()
    });
    let hair = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.17, 0.10),
        perceptual_roughness: 0.75,
        ..default()
    });
    let sweater = materials.add(StandardMaterial {
        base_color: Color::srgb(0.56, 0.12, 0.28),
        perceptual_roughness: 0.76,
        ..default()
    });
    let apron = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.94, 0.90),
        perceptual_roughness: 0.80,
        ..default()
    });
    let slacks = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.18, 0.24),
        perceptual_roughness: 0.7,
        ..default()
    });
    let eye_w = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        ..default()
    });
    let eye_b = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.06),
        ..default()
    });
    let saber_core = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.85),
        emissive: LinearRgba::new(5.5, 0.3, 0.25, 1.0),
        unlit: true,
        ..default()
    });
    let saber_aura = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.12, 0.14, 0.48),
        emissive: LinearRgba::new(3.0, 0.12, 0.12, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let hilt_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.18, 0.20),
        metallic: 0.85,
        ..default()
    });

    commands
        .spawn((
            LevelEntity,
            Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
        ))
        .with_children(|mom| {
            // Torso & Head
            mom.spawn((
                MomPart("Torso"),
                Transform::from_xyz(0.0, 0.85, 0.0),
            ))
            .with_children(|t| {
                t.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.35, 0.30))),
                    MeshMaterial3d(slacks.clone()),
                    Transform::from_xyz(0.0, -0.03, 0.0).with_scale(Vec3::new(1.02, 1.0, 0.78)),
                ));
                t.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.36, 0.78))),
                    MeshMaterial3d(sweater.clone()),
                    Transform::from_xyz(0.0, 0.32, 0.0).with_scale(Vec3::new(1.05, 1.0, 0.78)),
                ));
                // White Kitchen Apron front
                t.spawn((
                    Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.52, 0.72, 0.06)))),
                    MeshMaterial3d(apron),
                    Transform::from_xyz(0.0, 0.22, -0.28),
                ));
                // Head + stern face + chestnut bun
                t.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.28))),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, 0.96, 0.0),
                ));
                t.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.30))),
                    MeshMaterial3d(hair.clone()),
                    Transform::from_xyz(0.0, 1.02, 0.07),
                ));
                t.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.16))),
                    MeshMaterial3d(hair.clone()),
                    Transform::from_xyz(0.0, 1.24, 0.16),
                ));
                // Stern yelling eyes & open mouth facing -Z
                for sx in [-1.0_f32, 1.0_f32] {
                    t.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.048))),
                        MeshMaterial3d(eye_w.clone()),
                        Transform::from_xyz(sx * 0.10, 1.00, -0.24),
                    ));
                    t.spawn((
                        Mesh3d(meshes.add(Sphere::new(0.025))),
                        MeshMaterial3d(eye_b.clone()),
                        Transform::from_xyz(sx * 0.10, 1.00, -0.275),
                    ));
                    t.spawn((
                        Mesh3d(meshes.add(Cuboid::from_size(Vec3::new(0.10, 0.03, 0.03)))),
                        MeshMaterial3d(hair.clone()),
                        Transform::from_xyz(sx * 0.10, 1.07, -0.26)
                            .with_rotation(Quat::from_rotation_z(sx * -0.28)),
                    ));
                }
                t.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.07, 0.04))),
                    MeshMaterial3d(eye_b.clone()),
                    Transform::from_xyz(0.0, 0.85, -0.26)
                        .with_rotation(Quat::from_rotation_x(PI * 0.5)),
                ));
            });

            // Legs
            for (sx, part_name) in [(-1.0_f32, "LeftLeg"), (1.0_f32, "RightLeg")] {
                mom.spawn((
                    MomPart(part_name),
                    Transform::from_xyz(sx * 0.16, 0.72, 0.0),
                ))
                .with_children(|leg| {
                    leg.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.115, 0.72))),
                        MeshMaterial3d(slacks.clone()),
                        Transform::from_xyz(0.0, -0.36, 0.0),
                    ));
                });
            }

            // Left arm
            mom.spawn((
                MomPart("LeftArm"),
                Transform::from_xyz(-0.42, 1.42, 0.0),
            ))
            .with_children(|arm| {
                arm.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.095, 0.56))),
                    MeshMaterial3d(sweater.clone()),
                    Transform::from_xyz(0.0, -0.26, 0.0),
                ));
                arm.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.09))),
                    MeshMaterial3d(skin.clone()),
                    Transform::from_xyz(0.0, -0.55, 0.0),
                ));
            });

            // Right arm holding Crimson Lightsaber in ready duel guard
            mom.spawn((
                MomPart("RightArm"),
                Transform::from_xyz(0.42, 1.42, 0.0)
                    .with_rotation(Quat::from_rotation_x(0.95) * Quat::from_rotation_z(0.25)),
            ))
            .with_children(|arm| {
                arm.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.095, 0.56))),
                    MeshMaterial3d(sweater),
                    Transform::from_xyz(0.0, -0.26, 0.0),
                ));
                arm.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.09))),
                    MeshMaterial3d(skin),
                    Transform::from_xyz(0.0, -0.55, 0.0),
                ));
                // Crimson Lightsaber angled upward and forward away from torso
                arm.spawn(
                    Transform::from_xyz(0.02, -0.56, -0.05)
                        .with_rotation(Quat::from_rotation_x(-1.55)),
                )
                .with_children(|saber| {
                    saber.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.036, 0.28))),
                        MeshMaterial3d(hilt_mat),
                        Transform::from_xyz(0.0, 0.05, 0.0),
                    ));
                    saber.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.036, 1.68))),
                        MeshMaterial3d(saber_core),
                        Transform::from_xyz(0.0, 1.02, 0.0),
                    ));
                    saber.spawn((
                        Mesh3d(meshes.add(Cylinder::new(0.082, 1.75))),
                        MeshMaterial3d(saber_aura),
                        Transform::from_xyz(0.0, 1.02, 0.0),
                    ));
                    saber.spawn(PointLight {
                        color: Color::srgb(1.0, 0.14, 0.14),
                        intensity: 65000.0,
                        range: 18.0,
                        shadows_enabled: false,
                        ..default()
                    });
                });
            });
        })
        .id()
}

pub fn enter_boss_fight(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    enemy_q: Query<Entity, With<Enemy>>,
    mom_q: Query<Entity, With<MomBoss>>,
    hud_q: Query<Entity, With<BossHudNode>>,
    healing_tendy_q: Query<Entity, With<BossHealingTendy>>,
) {
    for e in &enemy_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &mom_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &healing_tendy_q {
        commands.entity(e).despawn_recursive();
    }

    // Dramatic illuminated showdown kitchen atmosphere
    commands.insert_resource(ClearColor(Color::srgb(0.22, 0.14, 0.28)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.90, 0.96),
        brightness: 540.0,
    });

    crate::audio::set_touch_playing(true);
    crate::audio::set_touch_boss_mode(true);
    crate::audio::play_boss_music();
    crate::audio::play_sfx("saber_ignite");
    crate::audio::play_sfx("mom_yell");
    crate::audio::speak_voice("Stan! You must eat your veggies! Final warning!", "mom");

    let resuming_from_headphone = stats.boss_mode && stats.headphone_return_boss;
    stats.headphone_return_boss = false;
    let init_hp = if resuming_from_headphone {
        stats.boss_hp.max(12.5)
    } else {
        100.0
    };

    stats.boss_mode = true;
    stats.hearts = stats.max_hearts;
    stats.tendies = stats.total_tendies;
    stats.player_scale = if stats.enraged {
        stats.target_scale()
    } else {
        3.2
    };
    stats.boss_max_hp = 100.0;
    stats.boss_hp = init_hp;
    if !resuming_from_headphone {
        stats.estus_flasks = stats.max_estus_flasks;
        crate::ui::save_game_checkpoint("boss", &stats);
    }
    stats.mom_quote = MOM_QUOTES[0].to_string();
    stats.hint_text =
        "DODGE & ROLL (SHIFT / ROLL) to counter Mom! Grab glowing Tendies to refuel Hearts!"
            .to_string();

    orbit.yaw = 0.0;
    orbit.pitch = 0.42;
    orbit.distance = 12.0;
    orbit.current_dist = 12.0;

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = if resuming_from_headphone {
            stats.saved_apartment_pos
        } else {
            Vec3::new(0.0, -6.0, 6.0)
        };
        p_tf.rotation = Quat::IDENTITY;
        p_tf.scale = Vec3::splat(stats.player_scale);
        player.velocity = Vec3::ZERO;
        player.grounded = true;
        player.invuln_timer = 1.0;
        player.roll_timer = 0.0;
        player.dodge_counter_timer = 0.0;
        player.attack_timer = 0.0;
        player.attack_cooldown = 0.0;
    }

    // Spawn Stan's Mom on the wide 1,200 Sq Ft Apartment Floor wielding her Crimson Lightsaber
    let mom_ent = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, -6.0, -14.0),
        2.65,
    );
    commands.entity(mom_ent).insert(MomBoss {
        hp: init_hp,
        max_hp: 100.0,
        state: MomState::Pursue,
        timer: 0.0,
        voice_timer: 5.5,
        summon_timer: 8.5,
        tendy_respawn_timer: 7.5,
        tendy_spawn_seq: 6,
        quote_idx: 0,
        walk_phase: 0.0,
        lunge_dir: Vec3::Z,
        hit_cooldown: 0.0,
        dealt_slash_hit: false,
    });

    // Scatter varied healing tendies across the apartment floor so Stan can refuel hearts mid-duel
    let varieties = [
        BossTendyVariety::CrispyGolden,
        BossTendyVariety::HoneyGlazed,
        BossTendyVariety::SpicyBuffalo,
    ];
    for (idx, &pos) in BOSS_TENDY_SPOTS.iter().take(6).enumerate() {
        spawn_boss_healing_tendy(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            varieties[idx % varieties.len()],
            pos,
            idx as f32 * 0.9,
        );
    }

    // Spawn hardened veggie minions across the open 1,200 Sq Ft Apartment Floor
    for (kind, pos) in [
        (EnemyKind::BroccoliBrute, Vec3::new(-12.0, -6.0, -6.0)),
        (EnemyKind::CarrotLancer, Vec3::new(12.0, -6.0, -8.0)),
        (EnemyKind::BroccoliBrute, Vec3::new(-18.0, -6.0, -20.0)),
        (EnemyKind::CarrotLancer, Vec3::new(18.0, -6.0, -22.0)),
        (EnemyKind::KaleSlime, Vec3::new(0.0, -6.0, -24.0)),
    ] {
        spawn_enemy_at(&mut commands, &asset_server, kind, pos);
    }

    // Top-center Boss Health Bar + Mom Quote Banner
    commands
        .spawn((
            BossHudNode,
            LevelEntity,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(84.0),
                left: Val::Percent(50.0),
                margin: UiRect::left(Val::Px(-185.0)),
                width: Val::Px(370.0),
                max_width: Val::Percent(92.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.14, 0.03, 0.06, 0.85)),
            BorderRadius::all(Val::Px(10.0)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("FINAL BOSS: MOM (EAT YOUR VEGGIES!)"),
                TextFont {
                    font_size: 13.5,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.42, 0.45)),
            ));
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.22, 0.08, 0.12)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        BossHpBarFill,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.95, 0.16, 0.22)),
                        BorderRadius::all(Val::Px(6.0)),
                    ));
                });
            panel.spawn((
                BossQuoteText,
                Text::new(format!("\"{}\"", MOM_QUOTES[0])),
                TextFont {
                    font_size: 12.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.88, 0.55)),
            ));
        });
}

pub fn update_boss_healing_tendies(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    player_q: Query<&Transform, With<Player>>,
    mut mom_q: Query<&mut MomBoss, Without<Player>>,
    mut healing_tendy_q: Query<
        (Entity, &mut Transform, &mut BossHealingTendy),
        (Without<Player>, Without<MomBoss>),
    >,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();
    let Ok(p_tf) = player_q.get_single() else {
        return;
    };

    let mut active_tendies = 0usize;
    for (t_ent, mut t_tf, mut tendy) in &mut healing_tendy_q {
        tendy.phase += dt * 2.8;
        t_tf.translation.y = tendy.base_y + tendy.phase.sin() * 0.28;
        t_tf.rotate_y(dt * 1.9);

        let d_xz = Vec2::new(
            p_tf.translation.x - t_tf.translation.x,
            p_tf.translation.z - t_tf.translation.z,
        )
        .length();
        let d_y = (p_tf.translation.y - t_tf.translation.y).abs();
        let can_use = stats.hearts < stats.max_hearts
            || stats.dash_energy < 85.0
            || tendy.kind == BossTendyVariety::SpicyBuffalo;
        if d_xz < 3.2 && d_y < 3.8 && can_use {
            let (heal, energy, msg) = match tendy.kind {
                BossTendyVariety::CrispyGolden => (
                    1,
                    30.0,
                    "ATE CRISPY GOLDEN TENDY! +1 HEART & +30 ROLL ENERGY!",
                ),
                BossTendyVariety::HoneyGlazed => (
                    2,
                    50.0,
                    "ATE HONEY-GLAZED TENDY! +2 HEARTS & +50 ROLL ENERGY!",
                ),
                BossTendyVariety::SpicyBuffalo => {
                    stats.speed_boost_timer = stats.speed_boost_timer.max(4.0);
                    (
                        1,
                        100.0,
                        "ATE SPICY BUFFALO TENDY! +1 HEART, FULL ROLL & SPEED BOOST!",
                    )
                }
            };
            stats.hearts = (stats.hearts + heal).min(stats.max_hearts);
            stats.dash_energy = (stats.dash_energy + energy).min(100.0);
            stats.score += 250;
            stats.hint_text = msg.to_string();
            crate::audio::play_sfx("tendy");
            commands.entity(t_ent).remove::<BossHealingTendy>();
            crate::player::start_pickup_pop(&mut commands, t_ent, t_tf.scale);
        } else {
            active_tendies += 1;
        }
    }

    if let Ok(mut mom) = mom_q.get_single_mut() {
        mom.tendy_respawn_timer -= dt;
        if mom.tendy_respawn_timer <= 0.0 {
            mom.tendy_respawn_timer = 7.5;
            if active_tendies < 6 {
                let varieties = [
                    BossTendyVariety::CrispyGolden,
                    BossTendyVariety::HoneyGlazed,
                    BossTendyVariety::SpicyBuffalo,
                ];
                let spot = BOSS_TENDY_SPOTS[mom.tendy_spawn_seq % BOSS_TENDY_SPOTS.len()];
                let variety = varieties[mom.tendy_spawn_seq % varieties.len()];
                mom.tendy_spawn_seq = mom.tendy_spawn_seq.wrapping_add(1);
                spawn_boss_healing_tendy(
                    &mut commands,
                    &asset_server,
                    &mut meshes,
                    &mut materials,
                    variety,
                    spot,
                    t,
                );
            }
        }
    }
}

pub fn update_boss_fight(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    mut mom_q: Query<(Entity, &mut Transform, &mut MomBoss, &Children), Without<Player>>,
    mut enemy_q: Query<(Entity, &mut Transform, &mut Enemy), (Without<Player>, Without<MomBoss>)>,
    children_q: Query<&Children>,
    mut mom_part_q: Query<(&MomPart, &mut Transform), (Without<Player>, Without<MomBoss>, Without<Enemy>)>,
    mut hp_fill_q: Query<&mut Node, With<BossHpBarFill>>,
    mut quote_q: Query<&mut Text, With<BossQuoteText>>,
    mut hud_pos_q: Query<&mut Node, (With<BossHudNode>, Without<BossHpBarFill>)>,
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    let dt = time.delta_secs().min(0.05);
    let t = time.elapsed_secs();

    // Responsive placement for Boss HUD so mobile HELP/SOUND pills never overlap
    if let (Ok(win), Ok(mut hud_node)) = (window_q.get_single(), hud_pos_q.get_single_mut()) {
        let narrow = win.width() < 680.0;
        let box_w = if narrow { (win.width() * 0.92).min(360.0) } else { 400.0 };
        hud_node.width = Val::Px(box_w);
        hud_node.margin = UiRect::left(Val::Px(-box_w * 0.5));
        hud_node.top = Val::Px(if narrow { 150.0 } else { 12.0 });
    }

    let Ok((p_tf, mut player)) = player_q.get_single_mut() else {
        return;
    };
    let Ok((mom_ent, mut mom_tf, mut mom, mom_children)) = mom_q.get_single_mut() else {
        return;
    };

    if mom.hit_cooldown > 0.0 {
        mom.hit_cooldown = (mom.hit_cooldown - dt).max(0.0);
    }

    // Periodic shouting voice lines from Mom
    mom.voice_timer -= dt;
    if mom.voice_timer <= 0.0 {
        mom.voice_timer = 6.5;
        mom.quote_idx = (mom.quote_idx + 1) % MOM_QUOTES.len();
        let line = MOM_QUOTES[mom.quote_idx];
        stats.mom_quote = line.to_string();
        crate::audio::play_sfx("mom_yell");
        crate::audio::speak_voice(line, "mom");
    }

    // Periodically reinforce angry veggie minions while Mom still fights
    mom.summon_timer -= dt;
    if mom.summon_timer <= 0.0 && mom.hp < 85.0 && enemy_q.iter().count() < 5 {
        mom.summon_timer = 8.5;
        let angle = t * 1.3;
        let spawn_pos = Vec3::new(
            (mom_tf.translation.x + angle.cos() * 5.5).clamp(-42.0, 42.0),
            mom_tf.translation.y,
            (mom_tf.translation.z + angle.sin() * 5.5).clamp(-34.0, 20.0),
        );
        spawn_enemy_at(&mut commands, &asset_server, EnemyKind::BroccoliBrute, spawn_pos);
    }

    // Stan's Lightsaber attack hits Mom (requires Dodge & Roll opening!) AND Veggie Minions
    if player.attack_timer > 0.0 {
        let p_forward = Vec3::new(-player.facing_yaw.sin(), 0.0, -player.facing_yaw.cos());
        let slash_center = p_tf.translation + p_forward * 2.2;

        // Check Mom hit once per swing
        let dist_mom = Vec2::new(
            slash_center.x - mom_tf.translation.x,
            slash_center.z - mom_tf.translation.z,
        )
        .length();
        let dy_mom = (p_tf.translation.y - mom_tf.translation.y).abs();
        if dist_mom < 5.6 && dy_mom < 4.5 && mom.hit_cooldown <= 0.0 {
            let dodge_opening = player.roll_timer > 0.0
                || player.dodge_counter_timer > 0.0
                || matches!(mom.state, MomState::Staggered);
            mom.hit_cooldown = 0.34;
            if !dodge_opening {
                let push = Vec3::new(
                    p_tf.translation.x - mom_tf.translation.x,
                    0.0,
                    p_tf.translation.z - mom_tf.translation.z,
                )
                .normalize_or_zero();
                // Parry shove (same 1.4 units as before) as a decaying knockback impulse.
                crate::player::hit_reaction(&mut player, &mut stats, push, 1.4, false);
                crate::audio::play_sfx("shield");
                stats.hint_text =
                    "MOM PARRIED! Use DODGE & ROLL (SHIFT / ROLL) to evade & break her guard!"
                        .to_string();
            } else {
                let base_dmg = if stats.enraged { 22.0 } else { 17.0 };
                let dmg = base_dmg * if stats.heavy_attack { 1.85 } else { 1.0 };
                mom.hp = (mom.hp - dmg).max(0.0);
                stats.boss_hp = mom.hp;
                stats.score += 1000;
                player.dodge_counter_timer = 0.0;
                mom.state = MomState::Staggered;
                mom.timer = 0.55;
                let kb = mom_tf.translation - p_tf.translation;
                let flat_kb = Vec3::new(kb.x, 0.0, kb.z).normalize_or_zero();
                mom_tf.translation += flat_kb * 1.6;
                crate::audio::play_sfx("saber_hit");
                stats.hint_text =
                    format!("DODGE COUNTER HIT! Mom HP: {:.0}/{:.0}", mom.hp, mom.max_hp);
            }
        }

        // Check Minion hits
        for (e_ent, mut e_tf, mut enemy) in &mut enemy_q {
            let d = Vec2::new(
                slash_center.x - e_tf.translation.x,
                slash_center.z - e_tf.translation.z,
            )
            .length();
            let dy = (p_tf.translation.y - e_tf.translation.y).abs();
            if d < 5.2 && dy < 3.6 && enemy.hit_cooldown <= 0.0 {
                enemy.hp -= 1;
                enemy.hit_cooldown = 0.35;
                let push = e_tf.translation - p_tf.translation;
                enemy.knockback = Vec3::new(push.x, 0.0, push.z).normalize_or_zero() * 18.0;
                crate::audio::play_sfx("saber_hit");
                if enemy.hp <= 0 {
                    stats.score += 450;
                    commands.entity(e_ent).despawn_recursive();
                } else {
                    stats.score += 150;
                    e_tf.translation += enemy.knockback * 0.08;
                }
            }
        }
    }

    // Check Boss Victory -> transition into Japanese Anime thank-you cutscene before NYC
    if mom.hp <= 0.0 {
        stats.score += 10000;
        stats.hint_text = "MOM DEFEATED — ENTERING HONORABLE THANK-YOU SCENE...".to_string();
        crate::audio::play_sfx("win");
        commands.entity(mom_ent).despawn_recursive();
        next_state.set(GameState::MomVictoryCutscene);
        return;
    }

    // Mom AI State Machine (Harder Phase 1 & 2 requiring Dodge & Roll counterplay)
    let to_player = p_tf.translation - mom_tf.translation;
    let flat = Vec3::new(to_player.x, 0.0, to_player.z);
    let dist = flat.length();
    let chase_speed = if mom.hp < 50.0 { 8.0 } else { 7.0 };
    let move_speed = match mom.state {
        MomState::Pursue => {
            if dist > 0.2 {
                let yaw = (-flat.x).atan2(-flat.z);
                mom_tf.rotation = Quat::from_rotation_y(yaw);
            }
            // Match player elevation smoothly so Mom follows Stan between counter & floor
            let dy = p_tf.translation.y - mom_tf.translation.y;
            mom_tf.translation.y += dy * (dt * 3.4).min(1.0);
            if dist < 6.4 {
                mom.state = MomState::SaberWindup;
                mom.timer = if mom.hp < 50.0 { 0.48 } else { 0.56 };
                mom.dealt_slash_hit = false;
                mom.lunge_dir = flat.normalize_or_zero();
                stats.hint_text = "MOM WINDING UP — DODGE ROLL NOW (SHIFT / ROLL)!".to_string();
                0.0
            } else {
                let dir = flat.normalize_or_zero();
                mom_tf.translation += dir * chase_speed * dt;
                chase_speed
            }
        }
        MomState::SaberWindup => {
            mom.timer -= dt;
            if dist > 0.2 {
                mom.lunge_dir = flat.normalize_or_zero();
                mom_tf.rotation = Quat::from_rotation_y((-flat.x).atan2(-flat.z));
            }
            if mom.timer <= 0.0 {
                mom.state = MomState::SaberSlash;
                mom.timer = 0.42;
                crate::audio::play_sfx("saber_swing");
            }
            0.0
        }
        MomState::SaberSlash => {
            mom.timer -= dt;
            if dist > 3.6 {
                mom_tf.translation += mom.lunge_dir * 15.5 * dt;
            }
            if !mom.dealt_slash_hit && dist < 5.8 && (p_tf.translation.y - mom_tf.translation.y).abs() < 3.6 {
                mom.dealt_slash_hit = true;
                if player.roll_timer > 0.0 {
                    player.dodge_counter_timer = 3.2;
                    stats.score += 350;
                    stats.hint_text =
                        "PERFECT DODGE ROLL! Counter-attack Mom NOW (E / Click / SABER)!"
                            .to_string();
                    crate::audio::play_sfx("shield");
                    mom.state = MomState::Staggered;
                    mom.timer = 1.65;
                } else if player.invuln_timer <= 0.0 {
                    stats.hearts -= 1;
                    player.invuln_timer = 0.9;
                    // Same 3.6-unit shove as before, delivered as a decaying knockback impulse.
                    crate::player::hit_reaction(&mut player, &mut stats, mom.lunge_dir, 3.6, true);
                    player.velocity.y = 6.2;
                    stats.hint_text =
                        "SLASHED BY MOM! Press SHIFT / ROLL right as she swings!".to_string();
                    crate::audio::play_sfx("hit");
                    if stats.hearts <= 0 {
                        crate::audio::play_sfx("lose");
                        next_state.set(GameState::GameOver);
                        return;
                    }
                }
            }
            if mom.timer <= 0.0 {
                mom.state = MomState::Staggered;
                mom.timer = 1.15;
            }
            15.5
        }
        MomState::Staggered => {
            mom.timer -= dt;
            if dist > 0.2 {
                mom_tf.rotation = Quat::from_rotation_y((-flat.x).atan2(-flat.z));
            }
            if mom.timer <= 0.0 {
                mom.state = MomState::Pursue;
            }
            0.0
        }
    };

    // Enforce minimum duel spacing so Mom never overlaps inside Stan's mesh
    let min_sep = 3.8_f32;
    let after_flat = Vec3::new(
        p_tf.translation.x - mom_tf.translation.x,
        0.0,
        p_tf.translation.z - mom_tf.translation.z,
    );
    let after_dist = after_flat.length();
    if after_dist < min_sep && after_dist > 0.01 {
        let push_back = after_flat.normalize() * (min_sep - after_dist);
        mom_tf.translation.x -= push_back.x;
        mom_tf.translation.z -= push_back.z;
    }

    mom_tf.translation.x = mom_tf.translation.x.clamp(-46.0, 46.0);
    mom_tf.translation.z = mom_tf.translation.z.clamp(-36.0, 36.0);
    mom.walk_phase += move_speed * dt * 1.4;

    // Animate Mom limbs & Crimson Lightsaber
    let blend = (dt * 15.0).min(1.0);
    let s = mom.walk_phase.sin();
    let (ll, rl, la, ra, body_rot) = match mom.state {
        MomState::Pursue => (
            Quat::from_rotation_x(s * 0.72),
            Quat::from_rotation_x(-s * 0.72),
            Quat::from_rotation_x(-s * 0.55) * Quat::from_rotation_z(-0.18),
            Quat::from_rotation_x(0.85 + s * 0.25) * Quat::from_rotation_z(0.22),
            Quat::from_rotation_x(-0.08),
        ),
        MomState::SaberWindup => (
            Quat::from_rotation_x(0.35),
            Quat::from_rotation_x(-0.35),
            Quat::from_rotation_z(-0.5),
            Quat::from_rotation_x(2.05) * Quat::from_rotation_z(0.30),
            Quat::from_rotation_x(0.22),
        ),
        MomState::SaberSlash => {
            let sweep = (1.0 - (mom.timer / 0.42).clamp(0.0, 1.0)) * 2.5 - 1.25;
            (
                Quat::from_rotation_x(-0.45),
                Quat::from_rotation_x(0.45),
                Quat::from_rotation_x(-0.5),
                Quat::from_rotation_x(0.45) * Quat::from_rotation_y(sweep),
                Quat::from_rotation_x(-0.28),
            )
        }
        MomState::Staggered => (
            Quat::IDENTITY,
            Quat::IDENTITY,
            Quat::from_rotation_z(-0.35),
            Quat::from_rotation_x(0.75) * Quat::from_rotation_z(0.35),
            Quat::from_rotation_x(0.15),
        ),
    };
    let mut stack: Vec<Entity> = mom_children.iter().copied().collect();
    while let Some(e) = stack.pop() {
        if let Ok(kids) = children_q.get(e) {
            stack.extend(kids.iter().copied());
        }
        if let Ok((part, mut tf)) = mom_part_q.get_mut(e) {
            let target = match part.0 {
                "LeftLeg" => ll,
                "RightLeg" => rl,
                "LeftArm" => la,
                "RightArm" => ra,
                "Torso" => body_rot,
                _ => Quat::IDENTITY,
            };
            tf.rotation = tf.rotation.slerp(target, blend);
        }
    }

    // Update Boss HUD Health Bar & Quote
    if let Ok(mut fill) = hp_fill_q.get_single_mut() {
        fill.width = Val::Percent((mom.hp / mom.max_hp * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut q_txt) = quote_q.get_single_mut() {
        q_txt.0 = format!("\"{}\"", stats.mom_quote);
    }
}

pub fn exit_boss_fight(
    mut commands: Commands,
    hud_q: Query<Entity, With<BossHudNode>>,
    healing_tendy_q: Query<Entity, With<BossHealingTendy>>,
) {
    crate::audio::set_touch_boss_mode(false);
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &healing_tendy_q {
        commands.entity(e).despawn_recursive();
    }
}
