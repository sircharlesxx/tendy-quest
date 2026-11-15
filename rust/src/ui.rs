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

#[derive(Component)]
pub struct TutorialCard;

#[derive(Component)]
pub struct TutorialTitleText;

#[derive(Component)]
pub struct TutorialBodyText;

#[derive(Component)]
pub struct TutorialControlsText;

#[derive(Component)]
pub struct TutorialBtnText;

pub fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .with_children(|parent| {
            // Top-left: Hearts, Dash Bar, Tendy & Size Counter
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        max_width: Val::Percent(56.0),
                        row_gap: Val::Px(4.0),
                        padding: UiRect::all(Val::Px(9.0)),
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
                            font_size: 15.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.32, 0.36)),
                    ));
                    // Dash bar
                    col.spawn((
                        Node {
                            width: Val::Px(132.0),
                            height: Val::Px(12.0),
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
                        Text::new("TENDIES: 0 / 12\nSIZE: 15cm (Tiny Toy)"),
                        TextFont {
                            font_size: 14.5,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 0.82, 0.22)),
                    ));
                    col.spawn((
                        StatusText,
                        Text::new(""),
                        TextFont {
                            font_size: 12.0,
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
                        row_gap: Val::Px(3.0),
                        padding: UiRect::all(Val::Px(9.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.05, 0.03, 0.72)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|col| {
                    col.spawn((
                        ScoreText,
                        Text::new("SCORE: 000000\nWAVE 1/3"),
                        TextFont {
                            font_size: 15.5,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                    col.spawn((
                        Text::new("[H] Help  |  [M] Audio"),
                        TextFont {
                            font_size: 12.0,
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
        t.0 = format!(
            "TENDIES: {} / {}\nSIZE: {}cm ({})",
            stats.tendies,
            stats.total_tendies,
            stats.height_cm(),
            stats.size_tier()
        );
    }
    if let Ok(mut t) = score_q.get_single_mut() {
        if stats.in_city {
            t.0 = format!("SCORE: {:06}\nNYC | {}/50", stats.score, stats.tendies);
        } else {
            t.0 = format!("SCORE: {:06}\nWAVE {}/3", stats.score, stats.wave);
        }
    }
    if let Ok(mut t) = status_q.get_single_mut() {
        let mut tags = Vec::new();
        if stats.enraged {
            tags.push(format!(
                "ENRAGED x{} ({:.0}s)",
                stats.rage_level.max(1),
                stats.rage_cooldown_timer.ceil().max(1.0)
            ));
        }
        if stats.speed_boost_timer > 0.0 {
            tags.push(format!("MUSTARD {:.0}s", stats.speed_boost_timer));
        }
        if stats.has_ranch_shield {
            tags.push("RANCH SHIELD".to_string());
        }
        if stats.audio_muted {
            tags.push("MUTED".to_string());
        }
        let buff_line = tags.join(" | ");
        if buff_line.is_empty() {
            t.0 = stats.hint_text.clone();
        } else if stats.hint_text.is_empty() {
            t.0 = buff_line;
        } else {
            t.0 = format!("{}\n{}", buff_line, stats.hint_text);
        }
    }
    if let Ok(mut node) = dash_q.get_single_mut() {
        node.width = Val::Percent(stats.dash_energy.clamp(0.0, 100.0));
    }
}

pub fn spawn_tutorial_overlay(mut commands: Commands) {
    crate::audio::set_tutorial_visible(true);
    commands
        .spawn((
            TutorialOverlay,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    top: Val::Px(58.0),
                    bottom: Val::Px(14.0),
                    left: Val::Px(10.0),
                    right: Val::Px(10.0),
                },
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.28)),
        ))
        .with_children(|root| {
            root.spawn((
                TutorialCard,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(14.0)),
                    width: Val::Percent(92.0),
                    max_width: Val::Px(540.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.05, 0.03, 0.86)),
                BorderRadius::all(Val::Px(14.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    TutorialTitleText,
                    Text::new("TENDY QUEST: 1,200 SQ FT APARTMENT"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.22)),
                ));
                card.spawn((
                    TutorialBodyText,
                    Text::new(
                        "Eat all 12 Golden Tendies to grow from 17cm Toy to 180cm Adult!\nExplore the 1,200 sq ft apartment, dodge Cheese Cups & Controllers, DON'T step on Headphones, and open the Giant Fridge!",
                    ),
                    TextFont {
                        font_size: 13.5,
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.92, 0.86)),
                ));
                card.spawn((
                    TutorialControlsText,
                    Text::new(
                        "- Move: WASD / Arrows or Left Touch Stick\n\
                         - Jump / Double-Jump: SPACE or JUMP (Dash: SHIFT / DASH)\n\
                         - Look / Zoom: Mouse + Scroll Wheel (Mobile: Drag + Pinch Zoom)\n\
                         - Help / Audio: H / M",
                    ),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.95, 1.0)),
                ));
                card.spawn((
                    StartButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(22.0), Val::Px(9.0)),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.82, 0.14, 0.16)),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        TutorialBtnText,
                        Text::new("START QUEST (SPACE / TAP)"),
                        TextFont {
                            font_size: 16.5,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
            });
        });
}

pub fn adapt_ui_to_viewport(
    window_q: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut root_q: Query<&mut Node, With<TutorialOverlay>>,
    mut card_q: Query<&mut Node, (With<TutorialCard>, Without<TutorialOverlay>)>,
    mut lexie_hud_q: Query<
        &mut Node,
        (
            With<crate::city::LexieBossHudRoot>,
            Without<TutorialOverlay>,
            Without<TutorialCard>,
        ),
    >,
    mut title_q: Query<&mut TextFont, With<TutorialTitleText>>,
    mut body_q: Query<&mut TextFont, (With<TutorialBodyText>, Without<TutorialTitleText>)>,
    mut ctrl_q: Query<
        &mut TextFont,
        (
            With<TutorialControlsText>,
            Without<TutorialTitleText>,
            Without<TutorialBodyText>,
        ),
    >,
    mut btn_q: Query<
        &mut TextFont,
        (
            With<TutorialBtnText>,
            Without<TutorialTitleText>,
            Without<TutorialBodyText>,
            Without<TutorialControlsText>,
        ),
    >,
    mut hud_text_q: Query<
        &mut TextFont,
        (
            Or<(With<HeartsText>, With<TendyText>, With<ScoreText>)>,
            Without<TutorialTitleText>,
            Without<TutorialBodyText>,
            Without<TutorialControlsText>,
            Without<TutorialBtnText>,
        ),
    >,
) {
    let Ok(win) = window_q.get_single() else {
        return;
    };
    let w = win.width();
    let h = win.height();
    let is_short = h < 500.0;
    let is_narrow = w < 520.0;

    if let Ok(mut root) = root_q.get_single_mut() {
        root.padding = if is_short {
            UiRect {
                top: Val::Px(44.0),
                bottom: Val::Px(8.0),
                left: Val::Px(10.0),
                right: Val::Px(10.0),
            }
        } else {
            UiRect {
                top: Val::Px(58.0),
                bottom: Val::Px(14.0),
                left: Val::Px(10.0),
                right: Val::Px(10.0),
            }
        };
    }
    if let Ok(mut card) = card_q.get_single_mut() {
        card.max_width = if is_short { Val::Px(640.0) } else { Val::Px(540.0) };
        card.width = if is_narrow { Val::Percent(94.0) } else { Val::Percent(90.0) };
        card.padding = if is_short {
            UiRect::axes(Val::Px(16.0), Val::Px(10.0))
        } else if is_narrow {
            UiRect::axes(Val::Px(16.0), Val::Px(14.0))
        } else {
            UiRect::axes(Val::Px(24.0), Val::Px(20.0))
        };
        card.row_gap = Val::Px(if is_short {
            5.0
        } else if is_narrow {
            8.0
        } else {
            11.0
        });
    }
    if let Ok(mut tf) = title_q.get_single_mut() {
        tf.font_size = if is_short { 19.0 } else if is_narrow { 21.0 } else { 27.0 };
    }
    if let Ok(mut tf) = body_q.get_single_mut() {
        tf.font_size = if is_short { 12.0 } else if is_narrow { 13.0 } else { 15.0 };
    }
    if let Ok(mut tf) = ctrl_q.get_single_mut() {
        tf.font_size = if is_short { 11.5 } else if is_narrow { 12.5 } else { 14.0 };
    }
    if let Ok(mut tf) = btn_q.get_single_mut() {
        tf.font_size = if is_short { 14.5 } else if is_narrow { 16.0 } else { 18.5 };
    }
    for mut tf in &mut hud_text_q {
        tf.font_size = if is_short || is_narrow { 12.5 } else { 15.0 };
    }
    for mut node in &mut lexie_hud_q {
        if w < 720.0 {
            node.top = Val::Auto;
            node.bottom = Val::Px(122.0);
        } else {
            node.top = Val::Px(8.0);
            node.bottom = Val::Auto;
        }
    }
}

pub fn check_url_test_mode(
    mut stats: ResMut<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    match crate::audio::init_test_mode() {
        1 => next_state.set(GameState::Cutscene),
        2 => next_state.set(GameState::BossFight),
        3 => next_state.set(GameState::MomVictoryCutscene),
        4 => next_state.set(GameState::CityIntroCutscene),
        5 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.player_scale = 1.8;
            stats.wave = 4;
            next_state.set(GameState::CityPlay);
        }
        6 => {
            stats.in_city = true;
            stats.tendies = 50;
            stats.total_tendies = 50;
            stats.player_scale = 18.0;
            stats.wave = 4;
            next_state.set(GameState::CityPlay);
        }
        7 => next_state.set(GameState::HeadphoneCutscene),
        8 => {
            stats.rage_level = 1;
            next_state.set(GameState::HeadphoneCutscene);
        }
        _ => {}
    }
}

pub fn handle_tutorial_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    stats: Res<PlayerStats>,
    mut next_state: ResMut<NextState<GameState>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
) {
    if keys.just_pressed(KeyCode::KeyK) {
        next_state.set(GameState::Cutscene);
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        next_state.set(GameState::BossFight);
        return;
    }
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    if clicked
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || touches.any_just_pressed()
        || crate::audio::take_touch_action_tap()
        || crate::audio::take_touch_jump()
        || crate::audio::take_touch_help()
    {
        if stats.boss_mode {
            next_state.set(GameState::BossFight);
        } else {
            crate::audio::play_bgm();
            next_state.set(GameState::Playing);
        }
    }
}

pub fn toggle_audio_hotkey(keys: Res<ButtonInput<KeyCode>>, mut stats: ResMut<PlayerStats>) {
    if keys.just_pressed(KeyCode::KeyM) {
        stats.audio_muted = crate::audio::toggle_mute();
    } else {
        stats.audio_muted = crate::audio::is_muted();
    }
}

pub fn toggle_tutorial_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::KeyH) || crate::audio::take_touch_help() {
        next_state.set(GameState::Tutorial);
    }
}

pub fn despawn_tutorial_overlay(
    mut commands: Commands,
    q: Query<Entity, With<TutorialOverlay>>,
) {
    crate::audio::set_tutorial_visible(false);
    for e in &q {
        commands.entity(e).despawn_recursive();
    }
}

pub fn spawn_stage_clear_overlay(commands: Commands, stats: Res<PlayerStats>) {
    if stats.lexie_defeated {
        spawn_end_card(
            commands,
            "LEXIE DEFEATED - NEW YORK CITY SAVED!",
            &format!(
                "SKYSCRAPER KAIJU STAN MASTERED THE TENDY-VERSE!\n50 / 50 MANHATTAN TENDIES | LEXIE & PICKLED GARLIC BANISHED!\nFINAL SCORE: {:06}",
                stats.score
            ),
            "PLAY AGAIN (R / TAP)",
            Color::srgb(1.0, 0.84, 0.2),
        );
    } else if stats.boss_mode {
        spawn_end_card(
            commands,
            "MOM DEFEATED - TENDY CHAMPION!",
            &format!(
                "STAN MASTERED THE CHRISTMAS VIKINGS ONESIE!\nDODGE & ROLL + LIGHTSABER SECURED THE TENDIES!\nFINAL SCORE: {:06}",
                stats.score
            ),
            "PLAY AGAIN (R / TAP)",
            Color::srgb(1.0, 0.84, 0.2),
        );
    } else {
        spawn_end_card(
            commands,
            "FRIDGE OPENED - STAGE CLEAR!",
            &format!(
                "YOU GREW TO {}cm ({})!\nTENDIES COLLECTED: {} / {}\nFINAL SCORE: {:06} (+5000 FRIDGE BONUS)",
                stats.height_cm(),
                stats.size_tier(),
                stats.tendies,
                stats.total_tendies,
                stats.score
            ),
            "PLAY AGAIN (R / TAP)",
            Color::srgb(1.0, 0.84, 0.2),
        );
    }
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
                    row_gap: Val::Px(12.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    width: Val::Percent(92.0),
                    max_width: Val::Px(540.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.09, 0.05, 0.95)),
                BorderRadius::all(Val::Px(16.0)),
            ))
            .with_children(|card| {
                card.spawn((
                    Text::new(title),
                    TextFont {
                        font_size: 26.0,
                        ..default()
                    },
                    TextColor(title_color),
                ));
                card.spawn((
                    Text::new(body),
                    TextFont {
                        font_size: 16.5,
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
    touches: Res<Touches>,
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
    if !(clicked
        || keys.just_pressed(KeyCode::KeyR)
        || keys.just_pressed(KeyCode::Space)
        || touches.any_just_pressed()
        || crate::audio::take_touch_action_tap()
        || crate::audio::take_touch_jump())
    {
        return;
    }

    for e in &end_q {
        commands.entity(e).despawn_recursive();
    }
    for e in &level_q {
        commands.entity(e).despawn_recursive();
    }
    let was_muted = stats.audio_muted;
    *stats = PlayerStats::default();
    stats.audio_muted = was_muted;
    *orbit = CameraOrbit::default();
    crate::audio::set_touch_boss_mode(false);
    crate::audio::set_uzumaki_mode(0);
    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 620.0,
    });
    spawn_kitchen_level_inner(&mut commands, &asset_server, &mut meshes, &mut materials);
    spawn_player_inner(&mut commands, &asset_server);
    spawn_initial_enemies_inner(&mut commands, &asset_server);
    crate::audio::play_bgm();
    next_state.set(GameState::Playing);
}
