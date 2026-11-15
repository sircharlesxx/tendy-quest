use bevy::prelude::*;
use crate::enemies::spawn_initial_enemies_inner;
use crate::level::spawn_kitchen_level_inner;
use crate::player::spawn_player_inner;
use crate::state::{CameraOrbit, GameState, LevelEntity, PlayerStats};

#[derive(Component)]
pub struct TutorialOverlay;

#[derive(Component)]
pub struct HudOverlay;

#[derive(Component)]
pub struct EndOverlay;

#[derive(Component)]
pub struct StartButton;

#[derive(Component)]
pub struct RestartButton;

#[derive(Component)]
pub struct HeartsText;

#[derive(Component)]
pub struct TendyText;

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct StatusText;

#[derive(Component)]
pub struct DashBarFill;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(18.0)),
                ..default()
            },
        ))
        .with_children(|parent| {
            // Top-left: Hearts, Dash Bar, Tendy Counter
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.05, 0.03, 0.72)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|col| {
                    col.spawn((
                        HeartsText,
                        Text::new("HEARTS: 5 / 5"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.32, 0.36)),
                    ));
                    // Dash bar
                    col.spawn((
                        Node {
                            width: Val::Px(190.0),
                            height: Val::Px(14.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.18, 0.18, 0.22)),
                        BorderRadius::all(Val::Px(6.0)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            DashBarFill,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.2, 0.78, 1.0)),
                            BorderRadius::all(Val::Px(6.0)),
                        ));
                    });
                    col.spawn((
                        TendyText,
                        Text::new("TENDIES: 0 / 10"),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.82, 0.22)),
                    ));
                    col.spawn((
                        StatusText,
                        Text::new(""),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.75, 1.0, 0.78)),
                    ));
                });

            // Top-right: Score + Controls hint
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        row_gap: Val::Px(4.0),
                        padding: UiRect::all(Val::Px(12.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.05, 0.03, 0.72)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|col| {
                    col.spawn((
                        ScoreText,
                        Text::new("SCORE: 000000"),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    col.spawn((
                        Text::new("[H] Show Controls"),
                        TextFont {
                            font_size: 15.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.85, 0.7)),
                    ));
                });
        });
}

pub fn update_hud(
    stats: Res<PlayerStats>,
    mut hearts_q: Query<&mut Text, With<HeartsText>>,
    mut tendy_q: Query<&mut Text, (With<TendyText>, Without<HeartsText>)>,
    mut score_q: Query<&mut Text, (With<ScoreText>, Without<HeartsText>, Without<TendyText>)>,
    mut status_q: Query<
        &mut Text,
        (
            With<StatusText>,
            Without<HeartsText>,
            Without<TendyText>,
            Without<ScoreText>,
        ),
    >,
    mut dash_q: Query<&mut Node, With<DashBarFill>>,
) {
    if let Ok(mut t) = hearts_q.get_single_mut() {
        t.0 = format!("HEARTS: {} / {}", stats.hearts.max(0), stats.max_hearts);
    }
    if let Ok(mut t) = tendy_q.get_single_mut() {
        t.0 = format!("TENDIES: {} / {}", stats.tendies, stats.total_tendies);
    }
    if let Ok(mut t) = score_q.get_single_mut() {
        t.0 = format!("SCORE: {:06}  |  WAVE {}/3", stats.score, stats.wave);
    }
    if let Ok(mut t) = status_q.get_single_mut() {
        let mut tags = Vec::new();
        if stats.speed_boost_timer > 0.0 {
            tags.push(format!("HONEY MUSTARD {:.0}s", stats.speed_boost_timer));
        }
        if stats.has_ranch_shield {
            tags.push("RANCH SHIELD".to_string());
        }
        t.0 = tags.join("  |  ");
    }
    if let Ok(mut node) = dash_q.get_single_mut() {
        node.width = Val::Percent(stats.dash_energy.clamp(0.0, 100.0));
    }
}

pub fn spawn_tutorial_overlay(mut commands: Commands) {
    commands
        .spawn((
            TutorialOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::bottom(Val::Px(22.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.16)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(12.0)),
                    max_width: Val::Px(740.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.05, 0.03, 0.74)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new("TENDY QUEST: GIANT KITCHEN"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.22)),
                ));
                card.spawn((
                    Text::new("Collect all 10 Golden Tendies along the countertop and reach the Pancake Stack!\nDodge Broccoli Brutes, lunging Carrot Lancers, and Kale Slime trails."),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.86)),
                ));
                card.spawn((
                    Text::new(
                        "CONTROLS:\n\
                         - [Mouse Move] : Look around in 3D space ([Scroll] Zoom, [Click] Lock Cursor, [ESC] Unlock)\n\
                         - [W A S D] or [Arrow Keys] : Run (relative to camera look)\n\
                         - [SPACE] : Jump & Double-Jump (hop onto Sponges & Pancakes!)\n\
                         - [SHIFT] : Belly Dash (brief invulnerability)\n\
                         - [H] : Toggle this Controls Guide anytime",
                    ),
                    TextFont {
                        font_size: 14.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.95, 1.0)),
                ));
                card.spawn((
                    StartButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(22.0), Val::Px(8.0)),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.82, 0.14, 0.16)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new("START QUEST (SPACE / CLICK)"),
                        TextFont {
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });
}

pub fn handle_tutorial_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
) {
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) {
        next_state.set(GameState::Playing);
    }
}

pub fn toggle_tutorial_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::KeyH) {
        next_state.set(GameState::Tutorial);
    }
}

pub fn despawn_tutorial_overlay(
    mut commands: Commands,
    q: Query<Entity, With<TutorialOverlay>>,
) {
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

pub fn spawn_stage_clear_overlay(commands: Commands, stats: Res<PlayerStats>) {
    spawn_end_card(
        commands,
        "STAGE CLEAR!",
        &format!(
            "LEVEL 1 COMPLETE!\nTENDIES COLLECTED: {} / {}\nFINAL SCORE: {:06} (+5000 BONUS)",
            stats.tendies, stats.total_tendies, stats.score
        ),
        "PLAY AGAIN (R / CLICK)",
        Color::srgb(1.0, 0.84, 0.2),
    );
}

pub fn spawn_game_over_overlay(commands: Commands, stats: Res<PlayerStats>) {
    spawn_end_card(
        commands,
        "OH NO! TOO MANY VEGGIES!",
        &format!(
            "TENDIES COLLECTED: {} / {}\nSCORE: {:06}",
            stats.tendies, stats.total_tendies, stats.score
        ),
        "TRY AGAIN (R / CLICK)",
        Color::srgb(1.0, 0.35, 0.35),
    );
}

fn spawn_end_card(
    mut commands: Commands,
    title: &str,
    body: &str,
    btn_label: &str,
    title_color: Color,
) {
    commands
        .spawn((
            EndOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.65)),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(14.0),
                    padding: UiRect::all(Val::Px(28.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.09, 0.05, 0.95)),
                BorderRadius::all(Val::Px(16.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 36.0,
                        ..default()
                    },
                    TextColor(title_color),
                ));
                card.spawn((
                    Text::new(body),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
                card.spawn((
                    RestartButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                        margin: UiRect::top(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.82, 0.14, 0.16)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Text::new(btn_label),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });
}

pub fn handle_restart_input(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    mut next_state: ResMut<NextState<GameState>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<RestartButton>)>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    if !(clicked || keys.just_pressed(KeyCode::KeyR)) {
        return;
    }

    for e in &end_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &level_q {
        commands.entity(e).despawn_recursive();
    }
    *stats = PlayerStats::default();
    *orbit = CameraOrbit::default();
    spawn_kitchen_level_inner(&mut commands, &asset_server, &mut meshes, &mut materials);
    spawn_player_inner(&mut commands, &asset_server);
    spawn_initial_enemies_inner(&mut commands, &asset_server);
    next_state.set(GameState::Playing);
}
