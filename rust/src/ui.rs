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

/// Start-menu button launching multiplayer Cedar Rapids Free Roam directly (with Character Select).
/// Always available; preserves unfinished story checkpoints.
#[derive(Component)]
pub struct FreeModeButton;

/// StageClear "Continue" (post-game free roam) button; shown only after the Lexie win.
#[derive(Component)]
pub struct RoamButton;

/// True when a checkpoint string records that Lexie has been beaten (field 7).
pub fn saved_game_beaten(saved: &str) -> bool {
    saved.split('|').nth(7) == Some("1")
}

/// Serializes progress as a compact pipe-delimited string for `window.tendySave` (`localStorage`)
/// rather than pulling `serde`/`serde_json` into the WASM binary.
pub fn save_game_checkpoint(stage: &str, stats: &PlayerStats) {
    // Pre-victory Free Mode entries (from the Start Menu button or `?free_roam=1`) must not
    // overwrite an unfinished story checkpoint (`"boss"`, mid-crew `"city"`, etc.).
    if stats.free_roam && !stats.game_beaten {
        return;
    }
    // Field 6 (gate mask: 1 TJ, 2 Derek, 4 Nate, 8 Glenn, 16 Mike), field 7 (Lexie beaten) and
    // field 8 (driver's license found) were appended later; older saves still load (see
    // `handle_tutorial_input`). The beaten flag is sticky: once a save has it, later saves from
    // a fresh run keep it.
    let beaten = stats.game_beaten || saved_game_beaten(&crate::audio::load_checkpoint());
    let data = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        stage,
        stats.score,
        stats.tendies,
        stats.tendy_credits,
        stats.tj_recruited as u8,
        stats.scooter_app_unlocked as u8,
        stats.gate_mask(),
        beaten as u8,
        stats.has_license as u8,
    );
    crate::audio::save_checkpoint(&data);
}

#[derive(Component)]
pub struct RestartButton;

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

/// Which HUD text a `Text` node shows; `update_hud` fills them all through one query.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudField {
    Hearts,
    Tendies,
    Tier,
    Flask,
    Credits,
    App,
    Objective,
    Status,
    Score,
    Hint,
}

/// HUD nodes that are shown only when they apply (`Display::None` otherwise).
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HudSlot {
    /// City-only chips (credits, scooter app).
    CityChip,
    /// The active-status line (enraged, mustard, shield, high...).
    Status,
}

/// One heart pip of the heart meter (index 0..HUD_MAX_PIPS).
#[derive(Component)]
pub struct HeartPip(pub u8);

/// Fill of the tendy progress bar under the tendy counter.
#[derive(Component)]
pub struct TendyBarFill;

#[derive(Component)]
pub struct DashBarFill;

/// The top-left status panel (resized by `adapt_ui_to_viewport`).
#[derive(Component)]
pub struct HudStatusPanel;

/// Transient hint toast (separate from persistent status), fades out a few seconds after the
/// hint changes.
#[derive(Component)]
pub struct HintToast;

#[derive(Component)]
pub struct HintToastBox;

const HUD_MAX_PIPS: u8 = 20;
const HUD_PANEL_BG: Color = Color::srgba(0.04, 0.05, 0.09, 0.80);
const HUD_CHIP_BG: Color = Color::srgba(1.0, 1.0, 1.0, 0.07);
const HEART_FULL: Color = Color::srgb(1.0, 0.30, 0.38);
const HEART_EMPTY: Color = Color::srgba(0.45, 0.20, 0.24, 0.45);
const HINT_SECS: f32 = 5.0;

fn hud_text(field: HudField, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        field,
        Text::new(text),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

fn hud_label(text: &str, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        TextColor(color),
        Node {
            width: Val::Px(30.0),
            ..default()
        },
    )
}

pub fn spawn_hud(mut commands: Commands, mut stats: ResMut<PlayerStats>) {
    stats.sync_tendy_wendies_from_save();
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
            // Top-left status panel: heart meter, stamina bar, tendies + size tier, flask and
            // city chips, one objective line, and active statuses (only while they apply).
            parent
                .spawn((
                    HudStatusPanel,
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(266.0),
                        max_width: Val::Percent(48.0),
                        row_gap: Val::Px(5.0),
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(HUD_PANEL_BG),
                    BorderRadius::all(Val::Px(10.0)),
                ))
                .with_children(|col| {
                    // Hearts: pip meter + count
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(hud_label("HP", HEART_FULL));
                        row.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            column_gap: Val::Px(1.5),
                            row_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|pips| {
                            for i in 0..HUD_MAX_PIPS {
                                pips.spawn((
                                    HeartPip(i),
                                    Node {
                                        width: Val::Px(6.0),
                                        height: Val::Px(6.0),
                                        ..default()
                                    },
                                    BackgroundColor(HEART_FULL),
                                    BorderRadius::all(Val::Px(3.0)),
                                ));
                            }
                        });
                        row.spawn(hud_text(HudField::Hearts, "5/5", 12.0, Color::WHITE));
                    });
                    // Stamina bar
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn(hud_label("STA", Color::srgb(0.45, 0.92, 0.55)));
                        row.spawn((
                            Node {
                                flex_grow: 1.0,
                                height: Val::Px(6.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.10)),
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
                                BackgroundColor(Color::srgb(0.24, 0.84, 0.40)),
                                BorderRadius::all(Val::Px(3.0)),
                            ));
                        });
                    });
                    // Tendies counter + progress bar + size tier
                    col.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|blk| {
                        blk.spawn(hud_text(
                            HudField::Tendies,
                            "TENDIES 0/12",
                            14.0,
                            Color::srgb(1.0, 0.86, 0.32),
                        ));
                        blk.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(4.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.10)),
                            BorderRadius::all(Val::Px(2.0)),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                TendyBarFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(1.0, 0.78, 0.18)),
                                BorderRadius::all(Val::Px(2.0)),
                            ));
                        });
                        blk.spawn(hud_text(
                            HudField::Tier,
                            "Tiny Toy | 15cm",
                            11.0,
                            Color::srgb(0.86, 0.88, 0.94),
                        ));
                    });
                    // Chips: flask, credits, scooter app
                    col.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(4.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|chips| {
                        for (field, slot, color) in [
                            (HudField::Flask, None, Color::srgb(1.0, 0.80, 0.42)),
                            (HudField::Credits, Some(HudSlot::CityChip), Color::srgb(0.55, 0.90, 1.0)),
                            (HudField::App, Some(HudSlot::CityChip), Color::srgb(0.70, 1.0, 0.70)),
                        ] {
                            let mut chip = chips.spawn((
                                Node {
                                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(HUD_CHIP_BG),
                                BorderRadius::all(Val::Px(6.0)),
                            ));
                            if let Some(slot) = slot {
                                chip.insert(slot);
                            }
                            chip.with_children(|c| {
                                c.spawn(hud_text(field, "", 10.5, color));
                            });
                        }
                    });
                    // Current objective (one line, wraps on narrow screens)
                    col.spawn(hud_text(HudField::Objective, "", 11.5, Color::WHITE));
                    // Active statuses only (hidden when nothing applies)
                    col.spawn((
                        HudSlot::Status,
                        Node {
                            display: Display::None,
                            ..default()
                        },
                    ))
                    .with_children(|c| {
                        c.spawn(hud_text(HudField::Status, "", 10.5, Color::srgb(0.62, 1.0, 0.72)));
                    });
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
                    col.spawn(hud_text(HudField::Score, "SCORE: 000000\nWAVE 1/3", 15.5, Color::WHITE));
                    col.spawn((
                        Text::new("[ESC] Pause  |  [M] Sound"),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.85, 0.7)),
                    ));
                });

            // Transient hint toast, lower center (kept clear of the top boss bars).
            parent
                .spawn((
                    HintToast,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        right: Val::Px(0.0),
                        bottom: Val::Px(90.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Visibility::Hidden,
                ))
                .with_children(|root| {
                    root.spawn((
                        HintToastBox,
                        Node {
                            max_width: Val::Px(560.0),
                            margin: UiRect::horizontal(Val::Px(12.0)),
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(HUD_PANEL_BG),
                        BorderColor(Color::srgba(1.0, 0.84, 0.32, 0.55)),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|b| {
                        b.spawn(hud_text(HudField::Hint, "", 13.5, Color::srgb(1.0, 0.95, 0.80)));
                    });
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

/// The single "what do I do now" line for the HUD.
fn objective_line(stats: &PlayerStats) -> String {
    if stats.in_club_level {
        return "> VELVET DISCO CLUB: VIP Dances (TW) | Left: Men's Room (Wash Hands = Wish!) & Women's Room | South: Casino"
            .to_string();
    }
    if stats.in_casino_level {
        return "> TENDY PALACE CASINO: Blackjack, Buffalo & Lucky Ladies Slots (TW) | South Portal -> Pub 217"
            .to_string();
    }
    if stats.in_city {
        if stats.free_roam {
            return "> FREE ROAM: scoot, hit the stunt gates, visit the crew in Pub 217".to_string();
        }
        if stats.lexie_defeated {
            return "LEXIE DEFEATED!".to_string();
        }
        if !stats.city_tendies_unlocked() {
            // TJ, Derek, Nate and Glenn are scattered around the city; Mike is in Pub 217.
            let mut steps = Vec::new();
            let crew = stats.missing_crew();
            if !crew.is_empty() {
                steps.push(format!("find {} around the city", crew.join(", ")));
            }
            if !stats.mike_smoked {
                steps.push("smoke with Mike in Pub 217".to_string());
            }
            return format!(
                "> UNLOCK TENDIES {}/5: {}",
                stats.gate_steps_done(),
                steps.join(", ")
            );
        }
        if stats.tendies < stats.total_tendies {
            return "> Eat tendies to grow, then slash Lexie".to_string();
        }
        return "> SLASH LEXIE! [CLICK] attack, [R-CLICK] block, [SHIFT] roll".to_string();
    }
    if stats.boss_mode {
        return "> Beat Mom: [CLICK] slash, [R-CLICK] block, [SHIFT] roll".to_string();
    }
    if stats.tendies < stats.total_tendies {
        "> Eat tendies to grow, then open the fridge".to_string()
    } else {
        "> Open the fridge!".to_string()
    }
}

/// Status words shown only while they apply.
fn status_tags(stats: &PlayerStats) -> Vec<String> {
    let mut tags = Vec::new();
    if stats.enraged {
        tags.push(format!(
            "ENRAGED x{} {:.0}s",
            stats.rage_level.max(1),
            stats.rage_cooldown_timer.ceil().max(1.0)
        ));
    }
    if stats.speed_boost_timer > 0.0 {
        tags.push(format!("MUSTARD {:.0}s", stats.speed_boost_timer.ceil()));
    }
    if stats.has_ranch_shield {
        tags.push("RANCH SHIELD".to_string());
    }
    if stats.lock_on_active {
        tags.push("LOCK-ON [TAB]".to_string());
    }
    if stats.in_city {
        if stats.high_timer > 0.0 {
            tags.push(format!("HIGH {:.0}s", stats.high_timer.ceil()));
        }
        if stats.drunk_timer > 0.0 {
            tags.push(format!("DRUNK {:.0}s", stats.drunk_timer.ceil()));
        }
        if stats.wanted_stars > 0 {
            tags.push(format!("WANTED {}", "*".repeat(stats.wanted_stars as usize)));
        }
        if stats.riding_scooter.is_some() {
            let g_pct = (stats.scooter_gravity_scale * 100.0).round() as u32;
            if g_pct < 100 {
                tags.push(format!("ON SCOOTER  GRAV {}%", g_pct));
            } else {
                tags.push("ON SCOOTER".to_string());
            }
        }
        if stats.driving_car.is_some() {
            let g_pct = (stats.car_gravity_scale * 100.0).round() as u32;
            if g_pct < 100 {
                tags.push(format!("IN CAR  GRAV {}%", g_pct));
            }
        }
    }
    if stats.audio_muted {
        tags.push("MUTED".to_string());
    }
    tags
}

#[allow(clippy::too_many_arguments)]
pub fn update_hud(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    state: Res<State<GameState>>,
    mut text_q: Query<(&mut Text, &mut TextColor, &HudField)>,
    mut dash_q: Query<&mut Node, With<DashBarFill>>,
    mut tendy_bar_q: Query<(&mut Node, &mut BackgroundColor), (With<TendyBarFill>, Without<DashBarFill>)>,
    mut pip_q: Query<
        (&HeartPip, &mut Node, &mut BackgroundColor),
        (Without<DashBarFill>, Without<TendyBarFill>),
    >,
    mut slot_q: Query<(&HudSlot, &mut Node), (Without<DashBarFill>, Without<TendyBarFill>, Without<HeartPip>)>,
    mut toast_q: Query<&mut Visibility, With<HintToast>>,
    mut toast_box_q: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<HintToastBox>, Without<TendyBarFill>, Without<HeartPip>),
    >,
    mut hint_track: Local<(String, u32, f32)>,
) {
    // Transient hint: shown for HINT_SECS after `hint_text` (or `hint_bump`) changes.
    if hint_track.0 != stats.hint_text || hint_track.1 != stats.hint_bump {
        hint_track.0 = stats.hint_text.clone();
        hint_track.1 = stats.hint_bump;
        hint_track.2 = 0.0;
    } else {
        hint_track.2 += time.delta_secs();
    }
    let hint_age = hint_track.2;
    let hint_alpha = if stats.hint_text.is_empty() {
        0.0
    } else {
        (1.0 - (hint_age - (HINT_SECS - 0.6)) / 0.6).clamp(0.0, 1.0)
    };
    let want_toast = if hint_alpha > 0.0 { Visibility::Inherited } else { Visibility::Hidden };
    for mut vis in &mut toast_q {
        if *vis != want_toast {
            *vis = want_toast;
        }
    }
    if let Ok((mut bg, mut border)) = toast_box_q.get_single_mut() {
        bg.0 = HUD_PANEL_BG.with_alpha(0.80 * hint_alpha);
        border.0 = Color::srgba(1.0, 0.84, 0.32, 0.55 * hint_alpha);
    }

    let unlocked = !stats.in_city || stats.in_casino_level || stats.in_club_level || stats.city_tendies_unlocked();
    let tags = status_tags(&stats);
    for (mut text, mut color, field) in &mut text_q {
        let new_text = match field {
            HudField::Hearts => format!("{}/{}", stats.hearts.max(0), stats.max_hearts),
            HudField::Tendies => {
                if stats.in_club_level {
                    "CLUB LEVEL".to_string()
                } else if stats.in_casino_level {
                    "CASINO LEVEL".to_string()
                } else if stats.in_city && stats.free_roam {
                    "CITY CLEARED".to_string()
                } else if unlocked {
                    format!("TENDIES {}/{}", stats.tendies, stats.total_tendies)
                } else {
                    format!("TENDIES {}/{}  LOCKED", stats.tendies, stats.total_tendies)
                }
            }
            HudField::Tier => format!("{} | {}cm", stats.size_tier(), stats.height_cm()),
            HudField::Flask => format!("FLASK [Q] {}/{}", stats.estus_flasks, stats.max_estus_flasks),
            // The license is saved with the checkpoint; Tendy Wendies persist across modes.
            HudField::Credits => {
                if (stats.in_city || stats.in_casino_level || stats.in_club_level) && stats.has_license {
                    format!("TW {} | CR {} | LICENSE", stats.tendy_wendies, stats.tendy_credits)
                } else {
                    format!("TW {} | CR {}", stats.tendy_wendies, stats.tendy_credits)
                }
            }
            HudField::App => {
                if stats.in_club_level {
                    "CASINO: SOUTH".to_string()
                } else if stats.in_casino_level {
                    "PORTAL: SOUTH".to_string()
                } else if stats.scooter_app_unlocked {
                    "SCOOTER APP OK".to_string()
                } else {
                    "PHONE [P]".to_string()
                }
            }
            HudField::Objective => objective_line(&stats),
            HudField::Status => tags.join("  "),
            HudField::Score => {
                if stats.in_club_level {
                    format!("SCORE: {:06}\nVELVET DISCO CLUB", stats.score)
                } else if stats.in_casino_level {
                    format!("SCORE: {:06}\nTENDY PALACE CASINO", stats.score)
                } else if stats.in_city && stats.free_roam {
                    format!("SCORE: {:06}\nCEDAR RAPIDS | FREE ROAM", stats.score)
                } else if stats.in_city {
                    format!("SCORE: {:06}\nCEDAR RAPIDS | {}/50", stats.score, stats.tendies)
                } else {
                    format!("SCORE: {:06}\nWAVE {}/3", stats.score, stats.wave)
                }
            }
            HudField::Hint => stats.hint_text.clone(),
        };
        if text.0 != new_text {
            text.0 = new_text;
        }
        match field {
            HudField::Hint => {
                color.0 = color.0.with_alpha(hint_alpha);
            }
            HudField::Tendies => {
                color.0 = if unlocked {
                    Color::srgb(1.0, 0.86, 0.32)
                } else {
                    Color::srgb(0.70, 0.70, 0.74)
                };
            }
            _ => {}
        }
    }

    for (slot, mut node) in &mut slot_q {
        let show = match slot {
            HudSlot::CityChip => stats.in_city || stats.in_casino_level || stats.in_club_level,
            HudSlot::Status => !tags.is_empty(),
        };
        let want = if show { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }

    let max_pips = stats.max_hearts.clamp(1, HUD_MAX_PIPS as i32);
    for (pip, mut node, mut bg) in &mut pip_q {
        let i = pip.0 as i32;
        let want = if i < max_pips { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        bg.0 = if i < stats.hearts { HEART_FULL } else { HEART_EMPTY };
    }

    if let Ok(mut node) = dash_q.get_single_mut() {
        node.width = Val::Percent(stats.dash_energy.clamp(0.0, 100.0));
    }
    if let Ok((mut node, mut bg)) = tendy_bar_q.get_single_mut() {
        // Free roam & Casino have no tendies; the bar reads as complete.
        let frac = if stats.in_casino_level || stats.in_club_level || (stats.in_city && stats.free_roam) {
            1.0
        } else {
            stats.tendies as f32 / stats.total_tendies.max(1) as f32
        };
        node.width = Val::Percent((frac * 100.0).clamp(0.0, 100.0));
        bg.0 = if unlocked {
            Color::srgb(1.0, 0.78, 0.18)
        } else {
            Color::srgb(0.55, 0.55, 0.60)
        };
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

/// Start-of-game Tutorial card (the in-game pause menu lives in controls.rs).
pub fn spawn_tutorial_overlay(mut commands: Commands) {
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
        BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.28)),
    ));
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
                         - Saber fights: tap CLICK / J to slash, hold it to charge a heavy, SHIFT / ROLL dodge roll\n\
                         - Block: hold RIGHT-CLICK / R / BLOCK / LT (raise it just in time to parry)\n\
                         - Strafe: hold V / L3 (or while blocking / locked on), Lock-on: TAB / R3, Heal: Q\n\
                         - Pause menu: ESC / H / START / HELP, Sound: M\n\
                         - Skip cutscene: hold ENTER / START / SKIP",
                    ),
                    TextFont {
                        font_size: 13.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.95, 1.0)),
                ));
                card.spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        row_gap: Val::Px(8.0),
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    row.spawn((
                        StartButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(22.0), Val::Px(9.0)),
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
                    row.spawn((
                        FreeModeButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.11, 0.46, 0.72)),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("FREE ROAM [F]"),
                            TextFont {
                                font_size: 15.5,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
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
        (&mut TextFont, &HudField),
        (
            Without<TutorialTitleText>,
            Without<TutorialBodyText>,
            Without<TutorialControlsText>,
            Without<TutorialBtnText>,
        ),
    >,
    mut hud_node_q: Query<
        (&mut Node, Has<HintToast>),
        (
            Or<(With<HudStatusPanel>, With<HintToast>)>,
            Without<TutorialOverlay>,
            Without<TutorialCard>,
            Without<crate::city::LexieBossHudRoot>,
        ),
    >,
    mut strip_q: Query<
        &mut Node,
        (
            With<crate::controls::ControlStrip>,
            Without<TutorialOverlay>,
            Without<TutorialCard>,
            Without<crate::city::LexieBossHudRoot>,
            Without<HudStatusPanel>,
            Without<HintToast>,
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
    // HUD text: one size per field, a touch smaller on narrow / short viewports.
    let k = if is_short || is_narrow { 0.88 } else { 1.0 };
    for (mut tf, field) in &mut hud_text_q {
        let base = match field {
            HudField::Hearts => 12.0,
            HudField::Tendies => 14.0,
            HudField::Tier => 11.0,
            HudField::Flask | HudField::Credits | HudField::App | HudField::Status => 10.5,
            HudField::Objective => 11.5,
            HudField::Score => 15.5,
            HudField::Hint => 13.5,
        };
        let size = base * k;
        if tf.font_size != size {
            tf.font_size = size;
        }
    }
    for (mut node, is_toast) in &mut hud_node_q {
        if is_toast {
            // Lower center; on narrow screens sit above the bottom-docked Lexie bar.
            let bottom = Val::Px(if w < 720.0 { 190.0 } else { 90.0 });
            if node.bottom != bottom {
                node.bottom = bottom;
            }
        } else {
            let width = Val::Px(if is_narrow { 214.0 } else { 266.0 });
            if node.width != width {
                node.width = width;
            }
        }
    }
    // Controls strip: bottom-left, never reaching the centred SCOOT button / hint toast.
    if let Ok(mut node) = strip_q.get_single_mut() {
        // A definite width (not max_width) so Taffy wraps whole chips into rows.
        let width = Val::Px((w * 0.5 - 200.0).clamp(200.0, 520.0));
        if node.width != width {
            node.width = width;
        }
        let bottom = Val::Px(if is_short { 6.0 } else { 10.0 });
        if node.bottom != bottom {
            node.bottom = bottom;
        }
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
        // Test links for the remaining city cutscenes (same city setup as the TJ link).
        m @ 10..=14 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.player_scale = 1.8;
            stats.wave = 4;
            // The Lexie finale link ends on the real post-Lexie StageClear card.
            stats.lexie_defeated = m == 14;
            next_state.set_from_black(match m {
                10 => GameState::DerekCutscene,
                11 => GameState::NateCutscene,
                12 => GameState::GlennCutscene,
                13 => GameState::MikeCutscene,
                _ => GameState::LexieVictoryCutscene,
            });
        }
        // `city_crew=1` (gate done, 0 tendies) and `city_crew_kaiju=1` (gate done, 30 tendies).
        m @ 15..=16 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.player_scale = 1.8;
            stats.wave = 4;
            stats.test_crew_preset = Some(if m == 15 { 0 } else { 30 });
            next_state.set_from_black(GameState::CityPlay);
        }
        // `free_roam=1`: direct Cedar Rapids free roam (preserves any unfinished story save).
        17 => {
            hydrate_beaten_free_roam_stats(&mut stats);
            stats.prepare_free_roam();
            stats.needs_city_char_select = true;
            next_state.set_from_black(GameState::CityPlay);
        }
        // `?chillary=1`: QA/dev direct-preview link for the optional boardwalk NPC cutscene so it can
        // be verified without walking to the River Star pier; has zero effect on normal gameplay.
        18 => {
            stats.in_city = true;
            stats.tendies = 0;
            stats.total_tendies = 50;
            stats.player_scale = 1.8;
            stats.wave = 4;
            stats.chillary_return_city = true;
            stats.saved_city_pos = crate::chillary::CHILLARY_POS + Vec3::new(3.0, 0.0, -2.5);
            next_state.set_from_black(GameState::ChillaryCutscene);
        }
        // `?casino=1`: QA/dev direct-preview link for Level 3 Tendy Palace Casino.
        19 => {
            stats.in_city = false;
            stats.in_casino_level = true;
            stats.player_scale = 1.8;
            stats.free_roam = true;
            stats.lexie_defeated = true;
            stats.scooter_app_unlocked = true;
            stats.saved_city_pos =
                crate::city::PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * crate::city::PUB217_SCALE;
            stats.saved_casino_city_scale = 1.8;
            next_state.set_from_black(GameState::CasinoPlay);
        }
        // `?pub_portal=1`: QA/dev direct-preview link for the Pub 217 10-beers & portal cutscene.
        20 => {
            stats.in_city = true;
            stats.free_roam = true;
            stats.lexie_defeated = true;
            stats.scooter_app_unlocked = true;
            stats.player_scale = 1.8;
            stats.saved_city_pos =
                crate::city::PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * crate::city::PUB217_SCALE;
            stats.saved_casino_city_scale = 1.8;
            next_state.set_from_black(GameState::PubPortalCutscene);
        }
        // `?lexie_battle=1`: QA/dev direct-preview link for the Turn-Based RPG Lexie Showdown.
        21 => {
            stats.in_city = true;
            stats.tendies = 50;
            stats.total_tendies = 50;
            stats.player_scale = 18.0;
            stats.tj_recruited = true;
            stats.derek_recruited = true;
            stats.nate_recruited = true;
            stats.glenn_recruited = true;
            stats.mike_smoked = true;
            stats.boss_hp = 950.0;
            stats.boss_max_hp = 950.0;
            next_state.set_from_black(GameState::LexieBattle);
        }
        // `?coaster_cutscene=1`: QA/dev direct-preview link for the Wonka/2001 Stargate Coaster cutscene.
        22 => {
            stats.in_city = false;
            stats.in_casino_level = true;
            stats.in_club_level = false;
            stats.player_scale = 1.8;
            stats.free_roam = true;
            stats.lexie_defeated = true;
            stats.scooter_app_unlocked = true;
            stats.saved_city_pos =
                crate::city::PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * crate::city::PUB217_SCALE;
            stats.saved_casino_city_scale = 1.8;
            next_state.set_from_black(GameState::CoasterCutscene);
        }
        // `?club=1`: QA/dev direct-preview link for Level 4 Velvet Disco Club.
        23 => {
            stats.in_city = false;
            stats.in_casino_level = false;
            stats.in_club_level = true;
            stats.player_scale = 1.8;
            stats.free_roam = true;
            stats.lexie_defeated = true;
            stats.scooter_app_unlocked = true;
            stats.sync_tendy_wendies_from_save();
            stats.tendy_wendies = stats.tendy_wendies.max(100);
            stats.club_skip_tw_sync = true;
            stats.saved_city_pos =
                crate::city::PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * crate::city::PUB217_SCALE;
            stats.saved_casino_city_scale = 1.8;
            next_state.set_from_black(GameState::ClubPlay);
        }
        // `?club_broke=1`: QA/dev link for out-of-TW Club -> Casino teleport check.
        24 => {
            stats.in_city = false;
            stats.in_casino_level = false;
            stats.in_club_level = true;
            stats.player_scale = 1.8;
            stats.free_roam = true;
            stats.lexie_defeated = true;
            stats.scooter_app_unlocked = true;
            stats.tendy_wendies = 0;
            stats.club_skip_tw_sync = true;
            stats.saved_city_pos =
                crate::city::PUB217_ANCHOR + Vec3::new(-5.8, 0.0, -0.8) * crate::city::PUB217_SCALE;
            stats.saved_casino_city_scale = 1.8;
            next_state.set_from_black(GameState::ClubPlay);
        }
        _ => {}
    }
}

/// Hydrates beaten-campaign score, tendy credits, and driver's license before entering Free Mode,
/// leaving unfinished campaign saves untouched (`game_beaten = false`).
fn hydrate_beaten_free_roam_stats(stats: &mut PlayerStats) {
    let saved = crate::audio::load_checkpoint();
    let parts: Vec<&str> = saved.split('|').collect();
    if saved_game_beaten(&saved) {
        if let Some(s) = parts.get(1).and_then(|v| v.parse::<u32>().ok()) {
            stats.score = s;
        }
        if let Some(c) = parts.get(3).and_then(|v| v.parse::<u32>().ok()) {
            stats.tendy_credits = c;
        }
        if parts.get(8).copied() == Some("1") {
            stats.has_license = true;
        }
        stats.game_beaten = true;
    } else {
        // Keep unfinished campaign progress untouched while entering Free Mode.
        stats.game_beaten = false;
    }
}

/// Enters Cedar Rapids Free Roam (Online P2P by default, with Character Select) from the Start Menu
/// while preserving unfinished campaign progress.
fn enter_free_mode_from_menu(
    stats: &mut PlayerStats,
    next_state: &mut crate::transition::SceneTransition,
) {
    hydrate_beaten_free_roam_stats(stats);
    crate::audio::net_set_online(true);
    stats.prepare_free_roam();
    stats.needs_city_char_select = true;
    next_state.set(GameState::CityPlay);
}

pub fn handle_tutorial_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut stats: ResMut<PlayerStats>,
    mut next_state: crate::transition::SceneTransition,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<StartButton>)>,
    free_q: Query<&Interaction, (Changed<Interaction>, With<FreeModeButton>)>,
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
    let free_clicked = free_q.iter().any(|i| *i == Interaction::Pressed);
    if free_clicked || keys.just_pressed(KeyCode::KeyF) {
        enter_free_mode_from_menu(&mut stats, &mut next_state);
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
            crate::audio::net_set_online(true);
            stats.free_roam = false;
            stats.needs_city_char_select = true;
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

/// Marks the in-game pause menu root (a pause overlay, not a `GameState`; see controls.rs).
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
    pub(crate) resume_grace: bool,
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

/// OnExit of the gameplay states: never leave the game paused or the card up.
pub fn close_help_overlay(
    mut commands: Commands,
    mut help: ResMut<HelpPause>,
    mut virtual_time: ResMut<Time<Virtual>>,
    overlay_q: Query<Entity, With<HelpOverlay>>,
) {
    close_help(&mut commands, &mut help, &mut virtual_time, &overlay_q);
    // A pointer-lock loss reported around the state change must not pause the next scene.
    let _ = crate::audio::take_pointer_lock_lost();
}

pub(crate) fn close_help(
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

pub fn spawn_stage_clear_overlay(commands: Commands, mut stats: ResMut<PlayerStats>) {
    if stats.lexie_defeated {
        // Record the win so a reload can go straight to free roam (checkpoint field 7).
        stats.game_beaten = true;
        save_game_checkpoint("city", &stats);
        spawn_end_card(
            commands,
            "LEXIE DEFEATED - NEW YORK CITY SAVED!",
            &format!(
                "SKYSCRAPER KAIJU STAN MASTERED THE TENDY-VERSE!\n50 / 50 MANHATTAN TENDIES | LEXIE & PICKLED GARLIC BANISHED!\nFINAL SCORE: {:06}\n\nCONTINUE: back to Cedar Rapids at normal size, crew at their spots, scoot around.",
                stats.score
            ),
            "PLAY AGAIN (R)",
            Color::srgb(1.0, 0.84, 0.2),
            true,
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
            false,
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
            false,
        );
    }
}

pub fn spawn_game_over_overlay(commands: Commands, stats: Res<PlayerStats>) {
    if stats.club_fentanyl_death {
        spawn_end_card(
            commands,
            "YOU DIED -- FENTANYL OVERDOSE",
            "NOBODY FOUND THE NARCAN IN TIME!\nSTAN GOD CLAIMED YOUR SOUL IN THE ETHEREAL REALM.\n\"TENDY WENDY FENDY BENDY...\"",
            "RETURN TO VELVET DISCO CLUB (R / CLICK)",
            Color::srgb(0.95, 0.14, 0.22),
            false,
        );
        return;
    }
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
        false,
    );
}

fn spawn_end_card(
    mut commands: Commands,
    title: &str,
    body: &str,
    btn_label: &str,
    title_color: Color,
    with_roam: bool,
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
                if with_roam {
                    // Post-game choice: free roam first (selected by default), then play again.
                    // `handle_restart_input` drives the selection highlight.
                    card.spawn((
                        RoamButton,
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                            margin: UiRect::top(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.14, 0.58, 0.28)),
                        BorderColor(Color::NONE),
                        BorderRadius::all(Val::Px(10.0)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            Text::new("CONTINUE: FREE ROAM (C)"),
                            TextFont {
                                font_size: 20.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                }
                card.spawn((
                    RestartButton,
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(24.0), Val::Px(12.0)),
                        margin: UiRect::top(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.82, 0.14, 0.16)),
                    BorderColor(Color::NONE),
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
#[allow(clippy::too_many_arguments)]
pub fn handle_restart_input(
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    mut stats: ResMut<PlayerStats>,
    state: Res<State<GameState>>,
    mut next_state: crate::transition::SceneTransition,
    btn_q: Query<&Interaction, (Changed<Interaction>, With<RestartButton>)>,
    roam_q: Query<&Interaction, (Changed<Interaction>, With<RoamButton>)>,
    new_roam_q: Query<(), Added<RoamButton>>,
    mut style_q: Query<
        (&mut BackgroundColor, &mut BorderColor, Has<RoamButton>),
        Or<(With<RestartButton>, With<RoamButton>)>,
    >,
    // Two-choice card selection: 0 = Continue (free roam), 1 = Play Again.
    mut selected: Local<u8>,
) {
    let clicked = btn_q.iter().any(|i| *i == Interaction::Pressed);
    let roam_clicked = roam_q.iter().any(|i| *i == Interaction::Pressed);
    // Always drain queued touch taps so a tap during the lockout doesn't fire afterwards.
    let tapped = crate::audio::take_touch_action_tap();
    let jumped = crate::audio::take_touch_jump();
    let has_roam = style_q.iter().any(|(_, _, roam)| roam);
    if has_roam {
        if !new_roam_q.is_empty() {
            *selected = 0;
        }
        // Up/down (keys, D-pad, stick) or left/right toggle between the two choices.
        let nav = crate::audio::take_menu_nav();
        if nav == -1
            || nav == 1
            || keys.any_just_pressed([
                KeyCode::ArrowUp,
                KeyCode::ArrowDown,
                KeyCode::ArrowLeft,
                KeyCode::ArrowRight,
                KeyCode::KeyW,
                KeyCode::KeyS,
            ])
        {
            *selected = 1 - (*selected).min(1);
        }
        for (mut bg, mut border, roam) in &mut style_q {
            let on = (*selected == 0) == roam;
            let want_bg = match (roam, on) {
                (true, true) => Color::srgb(0.14, 0.58, 0.28),
                (false, true) => Color::srgb(0.82, 0.14, 0.16),
                _ => Color::srgba(1.0, 1.0, 1.0, 0.10),
            };
            let want_border = if on { Color::srgba(1.0, 0.84, 0.32, 0.9) } else { Color::NONE };
            if bg.0 != want_bg {
                bg.0 = want_bg;
            }
            if border.0 != want_border {
                border.0 = want_border;
            }
        }
    }
    if !next_state.input_unlocked(crate::transition::RESTART_LOCKOUT_SECS) {
        return;
    }
    if has_roam {
        // Buttons (mouse / touch), hotkeys C and R, or confirm the highlighted choice with
        // Space / Enter / gamepad A / touch JUMP. A bare screen tap is ambiguous here, so it
        // only counts through the buttons. LT also sends R, so R is keyboard-only.
        let confirm = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter) || jumped;
        let go_roam = roam_clicked || keys.just_pressed(KeyCode::KeyC) || (confirm && *selected == 0);
        let go_again = clicked
            || (keys.just_pressed(KeyCode::KeyR) && !crate::audio::last_input_gamepad())
            || (confirm && *selected == 1);
        if go_roam {
            // Read by `restart_world_after_stage_clear` while the screen is black.
            stats.free_roam = true;
            next_state.set(GameState::CityPlay);
        } else if go_again {
            stats.free_roam = false;
            next_state.set(GameState::Playing);
        }
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
    next_state.set(if *state.get() == GameState::GameOver && stats.club_fentanyl_death {
        GameState::ClubPlay
    } else if retry_city {
        GameState::CityPlay
    } else {
        GameState::Playing
    });
}

/// OnExit(GameOver): reset while black. Retrying in the city respawns only the hero.
pub fn restart_world_after_game_over(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<PlayerStats>,
    orbit: ResMut<CameraOrbit>,
    level_q: Query<Entity, With<LevelEntity>>,
    end_q: Query<Entity, With<EndOverlay>>,
) {
    if stats.club_fentanyl_death {
        for e in &end_q {
            commands.entity(e).despawn_recursive();
        }
        stats.club_fentanyl_death = false;
        stats.hearts = stats.max_hearts;
        stats.club_cutscene_open = false;
        stats.club_modal_open = false;
        stats.club_dance_open = false;
        stats.club_psychedelic_timer = 0.0;
        return;
    }
    let retry_city = stats.in_city;
    reset_world_for_restart(
        retry_city, commands, asset_server, meshes, materials, stats, orbit, level_q, end_q,
    );
}

/// OnExit(StageClear): "play again" restarts Level 1; "continue" (`free_roam` set by
/// `handle_restart_input`) respawns the city for post-game free roam.
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
    let roam = stats.free_roam;
    reset_world_for_restart(
        roam, commands, asset_server, meshes, materials, stats, orbit, level_q, end_q,
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn reset_world_for_restart(
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
    // A city retry / restart inside free roam stays in free roam (keeping the score), and the
    // "Lexie beaten" flag always survives so later checkpoints keep it.
    let keep_roam = retry_city && stats.free_roam;
    let roam_score = stats.score;
    let beaten = stats.game_beaten;
    let license = stats.has_license;
    *stats = PlayerStats::default();
    stats.sync_tendy_wendies_from_save();
    stats.audio_muted = was_muted;
    stats.game_beaten = beaten;
    // A found driver's license survives city retries (and free roam restarts).
    stats.has_license = retry_city && license;
    *orbit = CameraOrbit::default();
    crate::audio::set_touch_boss_mode(false);
    crate::audio::set_uzumaki_mode(0);

    if retry_city {
        stats.in_city = true;
        stats.tendies = 0;
        stats.total_tendies = 50;
        stats.player_scale = 1.8;
        stats.wave = 4;
        if keep_roam {
            stats.prepare_free_roam();
            stats.score = roam_score;
        }
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
