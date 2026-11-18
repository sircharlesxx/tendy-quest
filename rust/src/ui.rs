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
pub struct ContinueButton;

/// Serializes progress as a compact pipe-delimited string for `window.tendySave` (`localStorage`)
/// rather than pulling `serde`/`serde_json` into the WASM binary.
pub fn save_game_checkpoint(stage: &str, stats: &PlayerStats) {
    let data = format!(
        "{}|{}|{}|{}|{}|{}",
        stage,
        stats.score,
        stats.tendies,
        stats.tendy_credits,
        stats.tj_recruited as u8,
        stats.scooter_app_unlocked as u8,
    );
    crate::audio::save_checkpoint(&data);
}

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
            // Top-left: Compact, translucent Hearts, Dash Bar, Tendy & Size Counter
            parent
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        max_width: Val::Px(290.0),
                        row_gap: Val::Px(2.0),
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.03, 0.05, 0.26)),
                    BorderRadius::all(Val::Px(6.0)),
                ))
                .with_children(|col| {
                    col.spawn((
                        HeartsText,
                        Text::new("HEARTS 5/5"),
                        TextFont {
                            font_size: 12.5,
                            ..default()
                        },
                        TextColor(Color::srgba(1.0, 0.42, 0.46, 0.88)),
                    ));
                    // Dash bar
                    col.spawn((
                        Node {
                            width: Val::Px(92.0),
                            height: Val::Px(5.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.16, 0.18, 0.22, 0.35)),
                        BorderRadius::all(Val::Px(3.0)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            DashBarFill,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.18, 0.78, 0.34, 0.88)),
                            BorderRadius::all(Val::Px(3.0)),
                        ));
                    });
                    col.spawn((
                        TendyText,
                        Text::new("TENDIES 0/12 | 15cm (Tiny Toy)"),
                        TextFont {
                            font_size: 11.5,
                            ..default()
                        },
                        TextColor(Color::srgba(1.0, 0.88, 0.38, 0.92)),
                    ));
                    col.spawn((
                        StatusText,
                        Text::new(""),
                        TextFont {
                            font_size: 10.5,
                            ..default()
                        },
                        TextColor(Color::srgba(0.88, 1.0, 0.9, 0.9)),
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

#[derive(Component)]
pub struct HurtFlashRoot;

/// One ring of the red damage vignette; `weight` scales the shared flash alpha for this ring.
#[derive(Component)]
pub struct HurtFlashRing {
    pub weight: f32,
}

/// Red screen-edge flash on taking damage: three nested border rings (strongest at the very
/// edge) since Bevy UI has no gradients. Sits above the HUD but below the help card and the
/// scene-fade overlay, and never takes pointer focus.
pub fn spawn_hurt_flash_overlay(mut commands: Commands) {
    let rings = [(2.2_f32, 0.85_f32), (2.6, 0.45), (3.2, 0.2)];
    let ring_node = |w: f32| Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        border: UiRect::all(Val::VMin(w)),
        ..default()
    };
    commands
        .spawn((
            HurtFlashRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            GlobalZIndex(900),
            bevy::ui::FocusPolicy::Pass,
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                ring_node(rings[0].0),
                BorderColor(Color::NONE),
                HurtFlashRing { weight: rings[0].1 },
                bevy::ui::FocusPolicy::Pass,
            ))
            .with_children(|p| {
                p.spawn((
                    ring_node(rings[1].0),
                    BorderColor(Color::NONE),
                    HurtFlashRing { weight: rings[1].1 },
                    bevy::ui::FocusPolicy::Pass,
                ))
                .with_children(|p| {
                    p.spawn((
                        ring_node(rings[2].0),
                        BorderColor(Color::NONE),
                        HurtFlashRing { weight: rings[2].1 },
                        bevy::ui::FocusPolicy::Pass,
                    ));
                });
            });
        });
}

/// Decays `PlayerStats::hurt_flash` (real time, so a help pause can't freeze it on screen) and
/// drives the vignette rings from it.
pub fn update_hurt_flash(
    real: Res<Time<Real>>,
    mut stats: ResMut<PlayerStats>,
    mut root_q: Query<&mut Visibility, (With<HurtFlashRoot>, Without<HurtFlashRing>)>,
    mut ring_q: Query<(&HurtFlashRing, &mut BorderColor)>,
) {
    let dt = real.delta_secs().min(0.05);
    if stats.hurt_flash > 0.0 {
        stats.hurt_flash = (stats.hurt_flash - dt / 0.35).max(0.0);
    }
    let a = stats.hurt_flash * stats.hurt_flash;
    let want = if a > 0.001 { Visibility::Inherited } else { Visibility::Hidden };
    for mut vis in &mut root_q {
        if *vis != want {
            *vis = want;
        }
    }
    if want == Visibility::Hidden {
        return;
    }
    for (ring, mut border) in &mut ring_q {
        border.0 = Color::srgba(0.85, 0.02, 0.04, a * ring.weight);
    }
}

pub fn update_hud(
    stats: Res<PlayerStats>,
    state: Res<State<GameState>>,
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
        t.0 = format!("HEARTS {}/{}", stats.hearts.max(0), stats.max_hearts);
    }
    if let Ok(mut t) = tendy_q.get_single_mut() {
        t.0 = format!(
            "TENDIES {}/{} | {}cm ({})",
            stats.tendies,
            stats.total_tendies,
            stats.height_cm(),
            stats.size_tier()
        );
    }
    if let Ok(mut t) = score_q.get_single_mut() {
        if stats.in_city {
            t.0 = format!("SCORE: {:06}\nCEDAR RAPIDS | {}/50", stats.score, stats.tendies);
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
        if stats.estus_flasks > 0 || stats.boss_mode {
            tags.push(format!("ESTUS [Q]: {}/{}", stats.estus_flasks, stats.max_estus_flasks));
        }
        if stats.lock_on_active {
            tags.push("LOCK-ON [TAB]".to_string());
        }
        if stats.in_city {
            tags.push(format!("CREDITS: {}", stats.tendy_credits));
            if stats.wanted_stars > 0 {
                let stars = "*".repeat(stats.wanted_stars as usize);
                tags.push(format!("WANTED: {}", stars));
            }
            if stats.high_timer > 0.0 {
                tags.push(format!("HIGH+DRUNK {:.0}s", stats.high_timer));
            } else {
                tags.push("DRUNK".to_string());
            }
            if stats.riding_scooter.is_some() {
                tags.push("SCOOTER".to_string());
            } else if stats.scooter_app_unlocked {
                tags.push("APP: UNLOCKED".to_string());
            } else {
                tags.push("PHONE [P]".to_string());
            }
        }
        if stats.tj_recruited {
            tags.push("ALLY: TJ".to_string());
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
    crate::audio::update_telemetry(
        &format!("{:?}", state.get()),
        stats.hearts,
        stats.tendies,
        stats.dash_energy,
        stats.boss_hp,
        stats.tendy_credits,
        stats.in_city,
    );
}

pub fn spawn_tutorial_overlay(mut commands: Commands) {
    spawn_tutorial_card(&mut commands, false);
}

/// Shared by the start-of-game Tutorial state and the in-game help pause overlay
/// (`as_help` swaps the start/continue buttons for a single RESUME button).
fn spawn_tutorial_card(commands: &mut Commands, as_help: bool) {
    crate::audio::set_tutorial_visible(true);
    let mut root = commands.spawn((
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
        BackgroundColor(Color::srgba(0.05, 0.03, 0.02, if as_help { 0.55 } else { 0.28 })),
    ));
    if as_help {
        // Draw over the city boss bar / minimap, which are spawned later than the HUD.
        root.insert((HelpOverlay, GlobalZIndex(1000)));
    }
    root
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
                    Text::new("TENDY QUEST"),
                    TextFont {
                        font_size: 24.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.82, 0.22)),
                ));
                card.spawn((
                    TutorialBodyText,
                    Text::new(
                        "Eat all 12 Golden Tendies to grow from 17cm Toy to 180cm Adult!\nExplore the apartment, dodge Cheese Cups & Controllers, DON'T step on Headphones, and open the Giant Fridge!",
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
                        Text::new(if as_help {
                            "RESUME (SPACE / H / TAP)"
                        } else {
                            "START QUEST (SPACE / TAP)"
                        }),
                        TextFont {
                            font_size: 16.5,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                    ));
                });
                let saved = crate::audio::load_checkpoint();
                let mom_defeated = crate::audio::has_defeated_mom()
                    || saved.split('|').next() == Some("city");
                let saved_stage = saved.split('|').next().unwrap_or("");
                if !as_help && (mom_defeated || saved_stage == "boss") {
                    let label = if mom_defeated {
                        "SKIP TO CEDAR RAPIDS [C] (MOM DEFEATED!)"
                    } else {
                        "CONTINUE CHECKPOINT: MOM BOSS [C]"
                    };
                    card.spawn((
                        ContinueButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                            margin: UiRect::top(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.58, 0.28)),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 14.5,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                }
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
    mut next_state: crate::transition::SceneTransition,
) {
    match crate::audio::init_test_mode() {
        1 => next_state.set_from_black(GameState::Cutscene),
        2 => next_state.set_from_black(GameState::BossFight),
        3 => next_state.set_from_black(GameState::MomVictoryCutscene),
        4 => next_state.set_from_black(GameState::CityIntroCutscene),
        5 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.player_scale = 1.8;
            stats.wave = 4;
            next_state.set_from_black(GameState::CityPlay);
        }
        6 => {
            stats.in_city = true;
            stats.tendies = 50;
            stats.total_tendies = 50;
            stats.player_scale = 18.0;
            stats.wave = 4;
            next_state.set_from_black(GameState::CityPlay);
        }
        7 => next_state.set_from_black(GameState::HeadphoneCutscene),
        8 => {
            stats.rage_level = 1;
            next_state.set_from_black(GameState::HeadphoneCutscene);
        }
        9 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.wave = 4;
            next_state.set_from_black(GameState::TjCutscene);
        }
        _ => {}
    }
}

pub fn handle_tutorial_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    cont_q: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
) {
    // Debug scene skips, only with `?debug=1` in the URL.
    if keys.just_pressed(KeyCode::KeyK) && crate::audio::debug_keys_enabled() {
        next_state.set(GameState::Cutscene);
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) && crate::audio::debug_keys_enabled() {
        next_state.set(GameState::BossFight);
        return;
    }
    let cont_clicked = cont_q.iter().any(|i| *i == Interaction::Pressed);
    if cont_clicked || keys.just_pressed(KeyCode::KeyC) {
        let saved = crate::audio::load_checkpoint();
        let parts: Vec<&str> = saved.split('|').collect();
        let stage = parts.first().copied().unwrap_or("");
        let mom_defeated = crate::audio::has_defeated_mom() || stage == "city";
        if mom_defeated || stage == "boss" {
            if let Some(s) = parts.get(1).and_then(|v| v.parse::<u32>().ok()) {
                stats.score = s;
            }
            if let Some(c) = parts.get(3).and_then(|v| v.parse::<u32>().ok()) {
                stats.tendy_credits = c;
            }
            if parts.get(5).copied() == Some("1") {
                stats.scooter_app_unlocked = true;
            }
            if mom_defeated {
                stats.in_city = true;
                stats.tendies = 0;
                stats.total_tendies = 50;
                stats.player_scale = 1.8;
                stats.wave = 4;
                next_state.set(GameState::CityPlay);
                return;
            } else if stage == "boss" {
                next_state.set(GameState::BossFight);
                return;
            }
        }
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

/// Marks the in-game help card (a pause overlay, not a `GameState`).
#[derive(Component)]
pub struct HelpOverlay;

/// In-game help pause. Opening help no longer leaves the gameplay state (which used to re-run
/// `OnEnter` on dismiss: the Mom fight reset her HP, and CityPlay jumped to `BossFight`); instead
/// gameplay systems are gated off by [`gameplay_running`] and `Time<Virtual>` is paused, so
/// everything resumes exactly where it was.
#[derive(Resource, Default)]
pub struct HelpPause {
    pub open: bool,
    /// Set on the frame help closes so the dismiss key (Space/Enter/tap) isn't also read as a
    /// jump by gameplay systems later in that frame. Cleared in `PostUpdate`.
    resume_grace: bool,
}

/// Run condition for gameplay-state Update sets: no transition pending and help not open.
pub fn gameplay_running(fade: Res<crate::transition::SceneFade>, help: Res<HelpPause>) -> bool {
    !fade.is_pending() && !help.open && !help.resume_grace
}

pub fn clear_help_resume_grace(mut help: ResMut<HelpPause>) {
    if help.resume_grace {
        help.resume_grace = false;
    }
}

/// H / touch HELP opens the help card in Playing, BossFight and CityPlay; H, Space, Enter, Esc,
/// a tap or the RESUME button closes it.
pub fn toggle_help_overlay(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut help: ResMut<HelpPause>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    overlay_q: Query<Entity, With<HelpOverlay>>,
) {
    if !help.open {
        if keys.just_pressed(KeyCode::KeyH) || crate::audio::take_touch_help() {
            help.open = true;
            virtual_time.pause();
            spawn_tutorial_card(&mut commands, true);
            // Free the mouse so the RESUME button can be clicked.
            if let Ok(mut window) = window_q.get_single_mut() {
                window.cursor_options.grab_mode = bevy::window::CursorGrabMode::None;
                window.cursor_options.visible = true;
            }
            crate::audio::set_touch_playing(false);
        }
        return;
    }
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    if clicked
        || keys.just_pressed(KeyCode::KeyH)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Escape)
        || touches.any_just_pressed()
        || crate::audio::take_touch_help()
        || crate::audio::take_touch_action_tap()
        || crate::audio::take_touch_jump()
    {
        close_help(&mut commands, &mut help, &mut virtual_time, &overlay_q);
        help.resume_grace = true;
        crate::audio::set_touch_playing(true);
    }
}

/// OnExit of the gameplay states: never leave the game paused or the card up.
pub fn close_help_overlay(
    mut commands: Commands,
    mut help: ResMut<HelpPause>,
    mut virtual_time: ResMut<Time<Virtual>>,
    overlay_q: Query<Entity, With<HelpOverlay>>,
) {
    close_help(&mut commands, &mut help, &mut virtual_time, &overlay_q);
}

fn close_help(
    commands: &mut Commands,
    help: &mut HelpPause,
    virtual_time: &mut Time<Virtual>,
    overlay_q: &Query<Entity, With<HelpOverlay>>,
) {
    if help.open {
        crate::audio::set_tutorial_visible(false);
    }
    help.open = false;
    if virtual_time.is_paused() {
        virtual_time.unpause();
    }
    for e in overlay_q {
        commands.entity(e).despawn_recursive();
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
    let title = "YOU DIED";
    let btn = if stats.in_city {
        "RETRY NYC (R / CLICK)"
    } else {
        "TRY AGAIN (R / CLICK)"
    };
    spawn_end_card(
        commands,
        title,
        &format!(
            "TENDIES COLLECTED: {} / {}\nSCORE: {:06}",
            stats.tendies, stats.total_tendies, stats.score
        ),
        btn,
        Color::srgb(0.86, 0.10, 0.10),
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

/// GameOver / StageClear input: after a short lockout (so a jump/attack held at the moment of
/// death can't skip the card) request a fade back into play. The actual world reset happens in
/// `OnExit` (see [`restart_world_after_game_over`]) while the screen is fully black.
pub fn handle_restart_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    stats: Res<PlayerStats>,
    state: Res<State<GameState>>,
    mut next_state: crate::transition::SceneTransition,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<RestartButton>)>,
) {
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    // Always drain queued touch taps so a tap during the lockout doesn't fire afterwards.
    let tapped = crate::audio::take_touch_action_tap();
    let jumped = crate::audio::take_touch_jump();
    if !next_state.input_unlocked(crate::transition::RESTART_LOCKOUT_SECS) {
        return;
    }
    if !(clicked
        || keys.just_pressed(KeyCode::KeyR)
        || keys.just_pressed(KeyCode::Space)
        || touches.any_just_pressed()
        || tapped
        || jumped)
    {
        return;
    }
    let retry_city = *state.get() == GameState::GameOver && stats.in_city;
    next_state.set(if retry_city {
        GameState::CityPlay
    } else {
        GameState::Playing
    });
}

/// OnExit(GameOver): reset while black. Retrying in the city respawns only the hero.
pub fn restart_world_after_game_over(
    commands: Commands,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    stats: ResMut<PlayerStats>,
    orbit: ResMut<CameraOrbit>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
    let retry_city = stats.in_city;
    reset_world_for_restart(
        retry_city, commands, asset_server, meshes, materials, stats, orbit, level_q, end_q,
    );
}

/// OnExit(StageClear): "play again" always restarts Level 1.
pub fn restart_world_after_stage_clear(
    commands: Commands,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    stats: ResMut<PlayerStats>,
    orbit: ResMut<CameraOrbit>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
    reset_world_for_restart(
        false, commands, asset_server, meshes, materials, stats, orbit, level_q, end_q,
    );
}

#[allow(clippy::too_many_arguments)]
fn reset_world_for_restart(
    retry_city: bool,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    mut orbit: ResMut<CameraOrbit>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
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

    if retry_city {
        stats.in_city = true;
        stats.tendies = 0;
        stats.total_tendies = 50;
        stats.player_scale = 1.8;
        stats.wave = 4;
        spawn_player_inner(&mut commands, &asset_server);
        return;
    }

    commands.insert_resource(ClearColor(Color::srgb(0.86, 0.84, 0.76)));
    commands.insert_resource(AmbientLight {
        color: Color::srgb(1.0, 0.96, 0.90),
        brightness: 620.0,
    });
    spawn_kitchen_level_inner(&mut commands, &asset_server, &mut meshes, &mut materials);
    spawn_player_inner(&mut commands, &asset_server);
    spawn_initial_enemies_inner(&mut commands, &asset_server);
    crate::audio::play_bgm();
}
