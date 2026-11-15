use std::f32::consts::PI;
use bevy::prelude::*;
use crate::enemies::{spawn_enemy_at, Enemy, EnemyKind};
use crate::player::Player;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlayerStats};

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
pub struct HeroCosmetic;

#[derive(Component)]
pub struct HeroSaberBlade;

#[derive(Component)]
pub struct CutsceneOverlay;

#[derive(Component)]
pub struct CutsceneBeatTitle;

#[derive(Component)]
pub struct CutsceneSubtitleText;

#[derive(Component)]
pub struct SkipCutsceneButton;

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

#[derive(Component)]
pub struct MomBoss {
    pub hp: f32,
    pub max_hp: f32,
    pub state: MomState,
    pub timer: f32,
    pub voice_timer: f32,
    pub summon_timer: f32,
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
    let should_wear =
        *state.get() == GameState::BossFight || (*state.get() == GameState::Cutscene && stats.cutscene_beat >= 3);
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

pub fn enter_cutscene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut player_q: Query<(&mut Transform, &mut Player)>,
    cosmetic_q: Query<Entity, With<HeroCosmetic>>,
) {
    for e in &cosmetic_q {
        commands.entity(e).despawn_recursive();
    }

    stats.cutscene_timer = 0.0;
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
        color: Color::srgb(0.24, 0.18, 0.44),
        brightness: 42.0,
    });

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(-0.3, 40.0, -1.3);
        p_tf.rotation = Quat::from_rotation_y(0.15);
        p_tf.scale = Vec3::splat(1.35);
        player.velocity = Vec3::ZERO;
        player.grounded = true;
        player.roll_timer = 0.0;
        player.attack_timer = 0.0;
    }

    let base_y = 40.0_f32;
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

    // Work-from-home Desk + Dual VMware ESXi DCUI Monitors
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, base_y + 1.0, -3.2),
        Vec3::new(4.8, 0.16, 1.8),
        Color::srgb(0.24, 0.16, 0.11),
        None,
    );
    for mx in [-0.95_f32, 0.95_f32] {
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(mx, base_y + 1.85, -3.55),
            Vec3::new(1.65, 1.08, 0.10),
            Color::srgb(0.06, 0.06, 0.08),
            None,
        );
        // ESXi Host DCUI Yellow top bar + Dark slate/cyan hypervisor console screen
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(mx, base_y + 2.18, -3.48),
            Vec3::new(1.50, 0.24, 0.03),
            Color::srgb(0.98, 0.78, 0.08),
            Some(LinearRgba::new(1.1, 0.85, 0.08, 1.0)),
        );
        spawn_box(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(mx, base_y + 1.72, -3.48),
            Vec3::new(1.50, 0.64, 0.03),
            Color::srgb(0.08, 0.14, 0.24),
            Some(LinearRgba::new(0.12, 0.38, 0.65, 1.0)),
        );
    }

    // Bare-metal ESXi Host Server Rack beside desk with blinking LEDs
    spawn_box(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(2.75, base_y + 1.3, -3.1),
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
            Vec3::new(2.75, ry, -2.43),
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
            Transform::from_xyz(3.05, ry, -2.38),
        ));
    }

    // Eerie monitor cyan glow + spooky purple doorway backlight
    commands.spawn((
        CutsceneEntity,
        LevelEntity,
        PointLight {
            color: Color::srgb(0.25, 0.65, 1.0),
            intensity: 26000.0,
            range: 9.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, base_y + 2.3, -2.4),
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

    // Spooky Mailman carrying the Christmas Vikings Onesie Parcel (enters left during Beat 2)
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
            Transform::from_xyz(-8.0, base_y, 0.2).with_rotation(Quat::from_rotation_y(-PI * 0.5)),
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
            // Festive parcel box with golden glow & Vikings purple ribbon
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

    // Mom enters in Beat 4 from right doorway wielding Crimson Lightsaber
    let mom_root = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(8.5, base_y, 0.4),
        1.55,
    );
    commands.entity(mom_root).insert((CutsceneEntity, CutsceneMom));

    // Cutscene Letterbox UI + Subtitles + Skip Button
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
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
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
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.78, 0.62, 1.0)),
                ));
            });

            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.01, 0.05, 0.88)),
            ))
            .with_children(|bot| {
                bot.spawn((
                    CutsceneSubtitleText,
                    Text::new("Stan worked late from home in the dark, managing virtual machines on his ESXi host..."),
                    TextFont {
                        font_size: 16.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.96, 0.94, 0.88)),
                ));
                bot.spawn((
                    SkipCutsceneButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.45, 0.16, 0.68)),
                    BorderRadius::all(Val::Px(8.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("SKIP TO BOSS BATTLE (SPACE / TAP)"),
                        TextFont {
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });
}

pub fn update_cutscene(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    skip_q: Query<&Interaction, (Changed<Interaction>, With<SkipCutsceneButton>)>,
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
    mut led_q: Query<(&CutsceneEsxiLed, &mut Transform), (Without<Camera3d>, Without<Player>, Without<CutsceneMailman>, Without<CutsceneMom>)>,
    mut title_q: Query<&mut Text, With<CutsceneBeatTitle>>,
    mut sub_q: Query<&mut Text, (With<CutsceneSubtitleText>, Without<CutsceneBeatTitle>)>,
) {
    let dt = time.delta_secs().min(0.05);
    stats.cutscene_timer += dt;
    let t = stats.cutscene_timer;

    // Skip immediately on Space/Enter/Tap/Button
    let skip_clicked = skip_q.iter().any(|i| *i == Interaction::Pressed);
    if (t > 0.35
        && (skip_clicked
            || keys.just_pressed(KeyCode::Space)
            || keys.just_pressed(KeyCode::Enter)
            || touches.any_just_pressed()
            || crate::audio::take_touch_action_tap()
            || crate::audio::take_touch_jump()))
        || t >= 18.0
    {
        next_state.set(GameState::BossFight);
        return;
    }

    // Pulse ESXi rack activity LEDs
    for (led, mut tf) in &mut led_q {
        let pulse = 0.6 + 0.5 * ((t * 9.0 + led.0).sin().abs());
        tf.scale = Vec3::splat(pulse);
    }

    // Determine beat 1..4 and speak voice narration once per transition
    let new_beat: u8 = if t < 4.5 {
        1
    } else if t < 9.0 {
        2
    } else if t < 13.5 {
        3
    } else {
        4
    };

    if new_beat != stats.cutscene_beat {
        stats.cutscene_beat = new_beat;
        let (title, sub, voice, role) = match new_beat {
            1 => (
                "CHAPTER I: 3:00 AM ON THE ESXi HOST",
                "In a dimly lit bedroom, Stan worked from home on his bare-metal VMware ESXi host...",
                "In a dimly lit bedroom, Stan worked from home through the dead of night, managing virtual machines on his ESXi host.",
                "narrator",
            ),
            2 => (
                "CHAPTER II: THE MAILMAN'S MYSTERIOUS PARCEL",
                "Suddenly a spooky mailman arrived, delivering Stan's new Christmas Vikings Onesie!",
                "Suddenly, a mysterious mailman arrived at the door, delivering Stan's newly purchased Christmas onesie with the Vikings logo!",
                "narrator",
            ),
            3 => (
                "CHAPTER III: AWAKENING OF THE VIKINGS ONESIE",
                "Putting it on unlocked Stan's Dodge & Roll powers and a blazing Lightsaber!",
                "Putting on the Christmas Vikings onesie unlocked ancient reflexes: Dodge and Roll mechanics, and a blazing Lightsaber!",
                "narrator",
            ),
            _ => (
                "FINAL BOSS: MOM HAS ENTERED WITH A LIGHTSABER!",
                "MOM: \"STAN! YOU MUST EAT YOUR VEGGIES!\"",
                "Stan! Put down those chicken tendies! You must eat your veggies right now!",
                "mom",
            ),
        };
        if let Ok(mut txt) = title_q.get_single_mut() {
            txt.0 = title.to_string();
        }
        if let Ok(mut txt) = sub_q.get_single_mut() {
            txt.0 = sub.to_string();
        }
        if new_beat == 3 {
            crate::audio::play_sfx("saber_ignite");
        } else if new_beat == 4 {
            crate::audio::play_sfx("mom_yell");
        }
        crate::audio::speak_voice(voice, role);
    }

    // Animate cutscene actors & camera across the 4 beats
    let base_y = 40.0_f32;
    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        match stats.cutscene_beat {
            1 => {
                // Working at ESXi desk facing north (-Z)
                p_tf.translation = Vec3::new(-0.3, base_y, -1.4);
                p_tf.rotation = Quat::from_rotation_y(0.0);
                player.velocity = Vec3::ZERO;
                player.roll_timer = 0.0;
            }
            2 => {
                // Turns left toward incoming mailman
                p_tf.translation = Vec3::new(-0.3, base_y, -1.0);
                p_tf.rotation = Quat::from_rotation_y(PI * 0.45);
                player.roll_timer = 0.0;
            }
            3 => {
                // Showcases new Christmas Vikings Onesie + Dodge & Roll + Lightsaber!
                let p = (t - 9.0).clamp(0.0, 4.5);
                p_tf.translation = Vec3::new((p * 1.6).sin() * 1.2, base_y + (p * 3.0).sin().abs() * 0.4, 0.2);
                p_tf.rotation = Quat::from_rotation_y(PI + (p * 1.4).sin() * 0.35);
                player.roll_timer = if (p % 1.4) < 0.42 { 0.42 - (p % 1.4) } else { 0.0 };
                player.attack_timer = if (p % 1.4) > 0.65 && (p % 1.4) < 0.95 { 0.25 } else { 0.0 };
            }
            _ => {
                // Faces off against Mom on right
                p_tf.translation = Vec3::new(-1.3, base_y, 0.4);
                p_tf.rotation = Quat::from_rotation_y(-PI * 0.5);
                player.roll_timer = 0.0;
                player.attack_timer = 0.2;
            }
        }
    }

    if let Ok(mut m_tf) = mailman_q.get_single_mut() {
        let walk_t = ((t - 4.2) / 2.2).clamp(0.0, 1.0);
        let x = -7.5 + walk_t * 5.6;
        m_tf.translation = Vec3::new(x, base_y, -0.3);
        m_tf.rotation = Quat::from_rotation_y(-PI * 0.5);
    }

    if let Ok(mut mom_tf) = mom_q.get_single_mut() {
        let enter_t = ((t - 13.3) / 1.4).clamp(0.0, 1.0);
        let x = 8.5 - enter_t * 6.5;
        mom_tf.translation = Vec3::new(x, base_y, 0.4);
        mom_tf.rotation = Quat::from_rotation_y(PI * 0.5);
    }

    if let Ok(mut c_tf) = cam_q.get_single_mut() {
        let (cam_pos, look_at) = match stats.cutscene_beat {
            1 => (
                Vec3::new(-2.4 + (t * 0.25).sin() * 0.5, base_y + 3.2, 2.8),
                Vec3::new(0.6, base_y + 1.6, -2.8),
            ),
            2 => (
                Vec3::new(-0.8, base_y + 2.8, 4.4),
                Vec3::new(-1.3, base_y + 1.4, -0.4),
            ),
            3 => (
                Vec3::new((t * 0.8).sin() * 2.4, base_y + 2.7, 4.6),
                Vec3::new(0.0, base_y + 1.5, 0.0),
            ),
            _ => (
                Vec3::new(0.2, base_y + 2.6, 5.2),
                Vec3::new(0.4, base_y + 1.6, 0.3),
            ),
        };
        c_tf.translation = cam_pos;
        c_tf.look_at(look_at, Vec3::Y);
    }
}

pub fn exit_cutscene(
    mut commands: Commands,
    ent_q: Query<Entity, With<CutsceneEntity>>,
    ui_q: Query<Entity, With<CutsceneOverlay>>,
) {
    for e in &ent_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &ui_q {
        commands.entity(e).despawn_recursive();
    }
}

fn spawn_mom_mesh_hierarchy(
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
                    .with_rotation(Quat::from_rotation_x(-0.85) * Quat::from_rotation_z(-0.18)),
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
                // Crimson Lightsaber
                arm.spawn(
                    Transform::from_xyz(0.02, -0.56, -0.05)
                        .with_rotation(Quat::from_rotation_x(-0.45)),
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
) {
    for e in &mom_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &hud_q {
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

    stats.boss_mode = true;
    stats.hearts = stats.max_hearts;
    stats.tendies = stats.total_tendies;
    stats.player_scale = 3.2;
    stats.boss_max_hp = 100.0;
    stats.boss_hp = 100.0;
    stats.mom_quote = MOM_QUOTES[0].to_string();
    stats.hint_text = "ROLL: Dodge & Roll | SABER: Lightsaber Slash!".to_string();

    orbit.yaw = 0.0;
    orbit.pitch = 0.45;
    orbit.distance = 11.2;
    orbit.current_dist = 11.2;

    if let Ok((mut p_tf, mut player)) = player_q.get_single_mut() {
        p_tf.translation = Vec3::new(0.0, 0.0, 12.5);
        p_tf.rotation = Quat::IDENTITY;
        p_tf.scale = Vec3::splat(3.2);
        player.velocity = Vec3::ZERO;
        player.grounded = true;
        player.roll_timer = 0.0;
        player.attack_timer = 0.0;
        player.attack_cooldown = 0.0;
    }

    // Spawn Stan's Mom on the main countertop wielding her Crimson Lightsaber
    let mom_ent = spawn_mom_mesh_hierarchy(
        &mut commands,
        &mut meshes,
        &mut materials,
        Vec3::new(0.0, 0.0, -3.5),
        2.65,
    );
    commands.entity(mom_ent).insert(MomBoss {
        hp: 100.0,
        max_hp: 100.0,
        state: MomState::Staggered,
        timer: 1.5,
        voice_timer: 5.5,
        summon_timer: 10.0,
        quote_idx: 0,
        walk_phase: 0.0,
        lunge_dir: Vec3::Z,
        hit_cooldown: 0.0,
        dealt_slash_hit: false,
    });

    // Ensure a fierce wave of hardened veggie minions is in the arena
    if enemy_q.iter().count() < 5 {
        for (kind, pos) in [
            (EnemyKind::BroccoliBrute, Vec3::new(-4.2, 0.0, 2.0)),
            (EnemyKind::CarrotLancer, Vec3::new(4.2, 0.0, 1.0)),
            (EnemyKind::BroccoliBrute, Vec3::new(-2.8, 0.0, -7.5)),
            (EnemyKind::CarrotLancer, Vec3::new(2.8, 0.0, -9.5)),
            (EnemyKind::KaleSlime, Vec3::new(0.0, 0.0, -1.5)),
        ] {
            spawn_enemy_at(&mut commands, &asset_server, kind, pos);
        }
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

pub fn update_boss_fight(
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
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

    let Ok((mut p_tf, mut player)) = player_q.get_single_mut() else {
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
    if mom.summon_timer <= 0.0 && mom.hp < 75.0 && enemy_q.iter().count() < 5 {
        mom.summon_timer = 11.0;
        let angle = t * 1.3;
        let spawn_pos = Vec3::new(
            (mom_tf.translation.x + angle.cos() * 3.5).clamp(-5.5, 5.5),
            mom_tf.translation.y,
            (mom_tf.translation.z + angle.sin() * 3.5).clamp(-18.0, 18.0),
        );
        spawn_enemy_at(&mut commands, &asset_server, EnemyKind::BroccoliBrute, spawn_pos);
    }

    // Stan's Lightsaber attack hits Mom AND Veggie Minions
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
            mom.hp = (mom.hp - 12.5).max(0.0);
            stats.boss_hp = mom.hp;
            stats.score += 1000;
            mom.hit_cooldown = 0.36;
            mom.state = MomState::Staggered;
            mom.timer = 0.45;
            let kb = mom_tf.translation - p_tf.translation;
            let flat_kb = Vec3::new(kb.x, 0.0, kb.z).normalize_or_zero();
            mom_tf.translation += flat_kb * 1.6;
            crate::audio::play_sfx("saber_hit");
            stats.hint_text = format!("SABER HIT! Mom HP: {:.0}/{:.0}", mom.hp, mom.max_hp);
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

    // Check Boss Victory
    if mom.hp <= 0.0 {
        stats.score += 10000;
        stats.hint_text = "MOM DEFEATED! CHICKEN TENDIES SECURED!".to_string();
        crate::audio::play_sfx("win");
        crate::audio::speak_voice("Okay Stan! You win! Eat all the chicken tendies you want!", "mom");
        commands.entity(mom_ent).despawn_recursive();
        next_state.set(GameState::StageClear);
        return;
    }

    // Mom AI State Machine
    let to_player = p_tf.translation - mom_tf.translation;
    let flat = Vec3::new(to_player.x, 0.0, to_player.z);
    let dist = flat.length();
    let move_speed = match mom.state {
        MomState::Pursue => {
            if dist > 0.2 {
                let yaw = (-flat.x).atan2(-flat.z);
                mom_tf.rotation = Quat::from_rotation_y(yaw);
            }
            // Match player elevation smoothly so Mom follows Stan between counter & floor
            let dy = p_tf.translation.y - mom_tf.translation.y;
            mom_tf.translation.y += dy * (dt * 2.8).min(1.0);
            if dist < 5.8 {
                mom.state = MomState::SaberWindup;
                mom.timer = 0.65;
                mom.dealt_slash_hit = false;
                mom.lunge_dir = flat.normalize_or_zero();
                0.0
            } else {
                let dir = flat.normalize_or_zero();
                mom_tf.translation += dir * 5.4 * dt;
                5.4
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
            if dist > 4.0 {
                mom_tf.translation += mom.lunge_dir * 10.5 * dt;
            }
            if !mom.dealt_slash_hit && dist < 5.0 && (p_tf.translation.y - mom_tf.translation.y).abs() < 3.4 {
                mom.dealt_slash_hit = true;
                if player.roll_timer > 0.0 || player.invuln_timer > 0.0 {
                    stats.score += 300;
                    stats.hint_text = "EVADED MOM'S SABER! (+300)".to_string();
                    crate::audio::play_sfx("shield");
                } else {
                    stats.hearts -= 1;
                    player.invuln_timer = 1.1;
                    p_tf.translation += mom.lunge_dir * 2.5;
                    player.velocity.y = 5.5;
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
                mom.timer = 1.35;
            }
            10.5
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

    mom_tf.translation.x = mom_tf.translation.x.clamp(-18.5, 20.5);
    mom_tf.translation.z = mom_tf.translation.z.clamp(-20.0, 20.0);
    mom.walk_phase += move_speed * dt * 1.4;

    // Animate Mom limbs & Crimson Lightsaber
    let blend = (dt * 15.0).min(1.0);
    let s = mom.walk_phase.sin();
    let (ll, rl, la, ra, body_rot) = match mom.state {
        MomState::Pursue => (
            Quat::from_rotation_x(s * 0.72),
            Quat::from_rotation_x(-s * 0.72),
            Quat::from_rotation_x(-s * 0.55) * Quat::from_rotation_z(-0.18),
            Quat::from_rotation_x(-0.85 + s * 0.25) * Quat::from_rotation_z(-0.15),
            Quat::from_rotation_x(-0.08),
        ),
        MomState::SaberWindup => (
            Quat::from_rotation_x(0.35),
            Quat::from_rotation_x(-0.35),
            Quat::from_rotation_z(-0.5),
            Quat::from_rotation_x(2.2) * Quat::from_rotation_z(0.3),
            Quat::from_rotation_x(0.22),
        ),
        MomState::SaberSlash => {
            let sweep = (1.0 - (mom.timer / 0.42).clamp(0.0, 1.0)) * 2.5 - 1.25;
            (
                Quat::from_rotation_x(-0.45),
                Quat::from_rotation_x(0.45),
                Quat::from_rotation_x(-0.5),
                Quat::from_rotation_x(-0.65) * Quat::from_rotation_y(sweep),
                Quat::from_rotation_x(-0.28),
            )
        }
        MomState::Staggered => (
            Quat::IDENTITY,
            Quat::IDENTITY,
            Quat::from_rotation_z(-0.65),
            Quat::from_rotation_z(0.65),
            Quat::from_rotation_x(0.25),
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
) {
    crate::audio::set_touch_boss_mode(false);
    for e in &hud_q {
        commands.entity(e).despawn_recursive();
    }
}
