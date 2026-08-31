use std::collections::HashMap;
use std::time::Instant;

use egui::{
    Align2, Color32, FontId, FullOutput, Pos2, RawInput, RichText, ScrollArea, Sense, Stroke, Ui,
    Vec2,
};

use crate::game::role::{color_name, color_rgb};
use crate::game::state::OverlayStatus;
use crate::overlay::theme::ThemeConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayAction {
    None,
    Close,
    DragWindow,
    ResizeWindow,
    ToggleStreamProof,
    ToggleLogKills,
    ToggleLogGameState,
    ToggleLogPlayerList,
    ExportMatchLog,
    ClearLogs,
    ChangeToggleKey(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineOrigin {
    LocalPlayer,
    BottomCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerFilter {
    All,
    ImpostorsOnly,
    CrewmatesOnly,
    DeadOnly,
    ImpostorsAndDead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayTab {
    Players,
    Tracers,
    Logs,
    CheatSheet,
    Themes,
}

#[derive(Debug, Clone)]
pub struct RadarState {
    pub scale: f32,
    pub show_tracers: bool,
    pub filter: PlayerFilter,
    pub origin: LineOrigin,
    pub selected_tab: OverlayTab,
    pub theme: ThemeConfig,
    pub smoothed_positions: HashMap<u8, (f32, f32)>,
    pub last_frame: Option<Instant>,
}

impl Default for RadarState {
    fn default() -> Self {
        Self {
            scale: 90.0,
            show_tracers: true,
            filter: PlayerFilter::All,
            origin: LineOrigin::LocalPlayer,
            selected_tab: OverlayTab::Players,
            theme: ThemeConfig::load(),
            smoothed_positions: HashMap::new(),
            last_frame: None,
        }
    }
}

pub fn draw_overlay(
    ctx: &egui::Context,
    raw_input: RawInput,
    state: &OverlayStatus,
    toggle_key: &str,
    is_editing_key: &mut bool,
    key_buffer: &mut String,
    radar: &mut RadarState,
) -> (FullOutput, OverlayAction) {
    ctx.request_repaint();
    let mut action = OverlayAction::None;
    let bg_color = radar.theme.bg_color32();

    let output = ctx.run(raw_input, |ctx| {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(bg_color)
                    .inner_margin(8.0)
                    .rounding(5.0),
            )
            .show(ctx, |ui| {
                draw_header(
                    ui,
                    state,
                    toggle_key,
                    is_editing_key,
                    key_buffer,
                    radar,
                    &mut action,
                );
                ui.separator();

                draw_tab_bar(ui, radar);
                ui.add_space(3.0);

                match radar.selected_tab {
                    OverlayTab::Players => {
                        if !state.players.is_empty() {
                            draw_player_list(ui, state, &mut action, radar);
                        } else {
                            draw_idle(ui, state);
                        }
                    }
                    OverlayTab::Tracers => {
                        if !state.players.is_empty() {
                            draw_tracer_controls(ui, radar);
                            ui.add_space(3.0);
                            draw_tracers_canvas(ui, state, radar);
                        } else {
                            draw_idle(ui, state);
                        }
                    }
                    OverlayTab::Logs => {
                        draw_event_logs(ui, state, &mut action, radar);
                    }
                    OverlayTab::CheatSheet => {
                        draw_cheat_sheet(ui);
                    }
                    OverlayTab::Themes => {
                        draw_theme_settings(ui, radar);
                    }
                }

                draw_resize_handle(ui, &mut action);
            });
    });
    (output, action)
}

fn draw_header(
    ui: &mut Ui,
    state: &OverlayStatus,
    toggle_key: &str,
    is_editing_key: &mut bool,
    key_buffer: &mut String,
    radar: &RadarState,
    action: &mut OverlayAction,
) {
    let top_rect = ui.available_rect_before_wrap();
    let header_rect = egui::Rect::from_min_size(top_rect.min, Vec2::new(top_rect.width(), 32.0));
    let drag_resp = ui.interact(header_rect, ui.id().with("header_drag_area"), Sense::drag());
    if drag_resp.drag_started() || (drag_resp.dragged() && ui.input(|i| i.pointer.primary_down())) {
        *action = OverlayAction::DragWindow;
    }

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Among Us External (by szuwer)")
                .strong()
                .size(13.0)
                .color(radar.theme.header_text_color32()),
        );

        ui.add_space(4.0);

        let (sp_color, sp_text) = if state.stream_proof {
            (Color32::from_rgb(80, 220, 120), "Stream-Proof: ON")
        } else {
            (Color32::from_rgb(230, 110, 90), "Stream-Proof: OFF")
        };

        if ui
            .button(RichText::new(sp_text).small().color(sp_color))
            .clicked()
        {
            *action = OverlayAction::ToggleStreamProof;
        }

        if *is_editing_key {
            let response = ui.add(
                egui::TextEdit::singleline(key_buffer)
                    .desired_width(55.0)
                    .hint_text("Key..."),
            );
            if response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let trimmed = key_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    *action = OverlayAction::ChangeToggleKey(trimmed);
                }
                *is_editing_key = false;
            }
            if ui.button("OK").clicked() {
                let trimmed = key_buffer.trim().to_string();
                if !trimmed.is_empty() {
                    *action = OverlayAction::ChangeToggleKey(trimmed);
                }
                *is_editing_key = false;
            }
        } else {
            let btn_text = format!("[{toggle_key}: Hide/Show]");
            let btn = ui.button(
                RichText::new(btn_text)
                    .small()
                    .color(radar.theme.accent_color32()),
            );
            if btn.clicked() {
                *is_editing_key = true;
                *key_buffer = toggle_key.to_string();
            }
            btn.on_hover_text("Click to customize toggle key");
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close_btn = ui.add(
                egui::Button::new(
                    RichText::new(" X ")
                        .strong()
                        .size(11.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(180, 45, 45)),
            );
            if close_btn.clicked() {
                *action = OverlayAction::Close;
            }
            close_btn.on_hover_text("Close Overlay");

            ui.add_space(4.0);

            let (dot_color, label) = match state.game_state {
                1 => (Color32::from_rgb(80, 180, 240), "LOBBY"),
                2 => (Color32::from_rgb(80, 220, 120), "IN MATCH"),
                3 => (Color32::from_rgb(240, 190, 60), "DISCUSSION"),
                _ => {
                    if state.connected {
                        (Color32::from_rgb(240, 190, 60), "CONNECTED")
                    } else {
                        (Color32::from_rgb(200, 80, 80), "OFFLINE")
                    }
                }
            };
            ui.label(RichText::new(label).small().color(dot_color));

            // Kill cooldown indicator
            if state.game_state == 2 {
                if let Some(last_k) = state.last_kill_time {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    let elapsed = (now_ms.saturating_sub(last_k) as f32) / 1000.0;
                    if elapsed < 35.0 {
                        let remaining = (35.0 - elapsed).max(0.0);
                        ui.label(
                            RichText::new(format!("CD: {:.0}s", remaining))
                                .small()
                                .color(Color32::from_rgb(255, 180, 70)),
                        );
                    }
                }
            }
        });
    });
}

fn draw_tab_bar(ui: &mut Ui, radar: &mut RadarState) {
    ui.horizontal(|ui| {
        let btn_players =
            ui.selectable_label(radar.selected_tab == OverlayTab::Players, "Players List");
        if btn_players.clicked() {
            radar.selected_tab = OverlayTab::Players;
        }

        let btn_tracers =
            ui.selectable_label(radar.selected_tab == OverlayTab::Tracers, "Radar/ESP");
        if btn_tracers.clicked() {
            radar.selected_tab = OverlayTab::Tracers;
        }

        let btn_logs = ui.selectable_label(radar.selected_tab == OverlayTab::Logs, "Console Logs");
        if btn_logs.clicked() {
            radar.selected_tab = OverlayTab::Logs;
        }

        let btn_cheat =
            ui.selectable_label(radar.selected_tab == OverlayTab::CheatSheet, "Cheat Sheet");
        if btn_cheat.clicked() {
            radar.selected_tab = OverlayTab::CheatSheet;
        }

        let btn_themes =
            ui.selectable_label(radar.selected_tab == OverlayTab::Themes, "Themes & Colors");
        if btn_themes.clicked() {
            radar.selected_tab = OverlayTab::Themes;
        }
    });
}

fn draw_tracer_controls(ui: &mut Ui, radar: &mut RadarState) {
    ui.horizontal(|ui| {
        let tracer_color = if radar.show_tracers {
            Color32::from_rgb(80, 220, 120)
        } else {
            Color32::from_rgb(180, 180, 180)
        };
        if ui
            .button(
                RichText::new(if radar.show_tracers {
                    "Tracers: ON"
                } else {
                    "Tracers: OFF"
                })
                .small()
                .color(tracer_color),
            )
            .clicked()
        {
            radar.show_tracers = !radar.show_tracers;
        }

        ui.add_space(3.0);

        let (filter_text, filter_color) = match radar.filter {
            PlayerFilter::All => ("Filter: All", radar.theme.accent_color32()),
            PlayerFilter::ImpostorsOnly => (
                "Filter: Impostors Only",
                radar.theme.impostor_line_color32(),
            ),
            PlayerFilter::CrewmatesOnly => {
                ("Filter: Crewmates Only", Color32::from_rgb(90, 220, 150))
            }
            PlayerFilter::DeadOnly => ("Filter: Dead Bodies Only", Color32::from_rgb(255, 90, 90)),
            PlayerFilter::ImpostorsAndDead => (
                "Filter: Impostors + Bodies",
                Color32::from_rgb(255, 140, 60),
            ),
        };
        if ui
            .button(RichText::new(filter_text).small().color(filter_color))
            .clicked()
        {
            radar.filter = match radar.filter {
                PlayerFilter::All => PlayerFilter::ImpostorsOnly,
                PlayerFilter::ImpostorsOnly => PlayerFilter::CrewmatesOnly,
                PlayerFilter::CrewmatesOnly => PlayerFilter::DeadOnly,
                PlayerFilter::DeadOnly => PlayerFilter::ImpostorsAndDead,
                PlayerFilter::ImpostorsAndDead => PlayerFilter::All,
            };
        }

        ui.add_space(3.0);

        let origin_text = match radar.origin {
            LineOrigin::LocalPlayer => "Origin: Local Player",
            LineOrigin::BottomCenter => "Origin: Bottom Center",
        };
        if ui
            .button(
                RichText::new(origin_text)
                    .small()
                    .color(radar.theme.accent_color32()),
            )
            .clicked()
        {
            radar.origin = match radar.origin {
                LineOrigin::LocalPlayer => LineOrigin::BottomCenter,
                LineOrigin::BottomCenter => LineOrigin::LocalPlayer,
            };
        }

        ui.add_space(3.0);

        if ui
            .button("-")
            .on_hover_text("Decrease Scale / FOV")
            .clicked()
        {
            radar.scale = (radar.scale - 10.0).max(20.0);
        }
        ui.label(RichText::new(format!("{:.0}px", radar.scale)).small());
        if ui
            .button("+")
            .on_hover_text("Increase Scale / FOV")
            .clicked()
        {
            radar.scale = (radar.scale + 10.0).min(300.0);
        }
    });
}

fn draw_tracers_canvas(ui: &mut Ui, state: &OverlayStatus, radar: &mut RadarState) {
    let now = Instant::now();
    let dt = if let Some(last) = radar.last_frame {
        now.duration_since(last).as_secs_f32().min(0.05)
    } else {
        0.016
    };
    radar.last_frame = Some(now);

    let lerp_factor = 1.0 - (-24.0 * dt).exp();

    for player in &state.players {
        let (tx, ty) = player.position;
        if tx == 0.0 && ty == 0.0 && radar.smoothed_positions.contains_key(&player.player_id) {
            continue;
        }

        let entry = radar
            .smoothed_positions
            .entry(player.player_id)
            .or_insert((tx, ty));
        let jump_dist = ((tx - entry.0).powi(2) + (ty - entry.1).powi(2)).sqrt();
        if jump_dist > 25.0 {
            *entry = (tx, ty);
        } else {
            entry.0 += (tx - entry.0) * lerp_factor;
            entry.1 += (ty - entry.1) * lerp_factor;
        }
    }

    let avail_w = ui.available_width();
    let avail_h = ui.available_height().max(240.0);
    let size = Vec2::new(avail_w, avail_h);
    let (rect, _response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 4.0, radar.theme.canvas_color32());
    painter.rect_stroke(
        rect,
        4.0,
        Stroke::new(1.0_f32, radar.theme.border_color32()),
    );

    let center = rect.center();
    let origin = match radar.origin {
        LineOrigin::LocalPlayer => center,
        LineOrigin::BottomCenter => Pos2::new(center.x, rect.bottom() - 10.0),
    };

    let local_color = radar.theme.local_player_color32();
    painter.circle_filled(origin, 5.0, local_color);
    painter.circle_stroke(
        origin,
        7.0,
        Stroke::new(
            1.2_f32,
            Color32::from_rgba_unmultiplied(local_color.r(), local_color.g(), local_color.b(), 140),
        ),
    );
    painter.text(
        Pos2::new(origin.x, origin.y + 8.0),
        Align2::CENTER_TOP,
        "[YOU]",
        FontId::proportional(10.0),
        local_color,
    );

    let local_player = state.players.iter().find(|p| p.is_local);
    let local_pos = local_player.map(|p| p.position).unwrap_or((0.0, 0.0));

    // 1. Draw alive players (Skip ghosts)
    for player in &state.players {
        if player.is_local || player.is_dead {
            continue;
        }

        let is_imp = player.role.is_impostor_team();

        match radar.filter {
            PlayerFilter::All => {}
            PlayerFilter::ImpostorsOnly => {
                if !is_imp {
                    continue;
                }
            }
            PlayerFilter::CrewmatesOnly => {
                if is_imp {
                    continue;
                }
            }
            PlayerFilter::DeadOnly => {
                // Only dead bodies are drawn
                continue;
            }
            PlayerFilter::ImpostorsAndDead => {
                if !is_imp {
                    continue;
                }
            }
        }

        let smoothed_pos = radar
            .smoothed_positions
            .get(&player.player_id)
            .copied()
            .unwrap_or(player.position);
        let dx = smoothed_pos.0 - local_pos.0;
        let dy = smoothed_pos.1 - local_pos.1;
        let distance = (dx * dx + dy * dy).sqrt();

        let target_x = center.x + (dx * (radar.scale * 0.5));
        let target_y = match radar.origin {
            LineOrigin::LocalPlayer => center.y - (dy * (radar.scale * 0.5)),
            LineOrigin::BottomCenter => {
                origin.y
                    - 20.0
                    - (dy.max(0.0) * (radar.scale * 0.5) + (dx.abs() * 0.2 * (radar.scale * 0.5)))
            }
        };

        let pad = 24.0;
        let clamped_x = target_x.clamp(rect.left() + pad, rect.right() - pad);
        let clamped_y = target_y.clamp(rect.top() + pad, rect.bottom() - pad);
        let target_pt = Pos2::new(clamped_x, clamped_y);

        let (r, g, b) = color_rgb(player.color_id);
        let player_col = Color32::from_rgb(r, g, b);
        let role_col = radar.theme.role_color32(&player.role);

        if radar.show_tracers {
            let line_color = if player.in_vent {
                Color32::from_rgb(255, 140, 40)
            } else if is_imp {
                radar.theme.impostor_line_color32()
            } else {
                Color32::from_rgba_unmultiplied(role_col.r(), role_col.g(), role_col.b(), 180)
            };
            painter.line_segment([origin, target_pt], Stroke::new(1.6_f32, line_color));
        }

        painter.circle_filled(target_pt, 5.0, player_col);
        let outline_col = if player.in_vent {
            Color32::from_rgb(255, 140, 40)
        } else if is_imp {
            radar.theme.impostor_line_color32()
        } else {
            role_col
        };
        painter.circle_stroke(target_pt, 5.0, Stroke::new(1.4_f32, outline_col));

        let mut label_text = format!("{} [{}] - {:.1}m", player.name, player.role, distance);
        if player.in_vent {
            label_text = format!("[VENT] {label_text}");
        } else if player.shapeshifting {
            label_text = format!("[SHAPESHIFTED] {label_text}");
        }

        let text_color = if player.in_vent {
            Color32::from_rgb(255, 160, 50)
        } else if is_imp {
            Color32::from_rgb(255, 110, 110)
        } else {
            role_col
        };

        painter.text(
            Pos2::new(target_pt.x, target_pt.y - 12.0),
            Align2::CENTER_BOTTOM,
            label_text,
            FontId::proportional(11.0),
            text_color,
        );
    }

    // 2. Draw Physical Dead Bodies on the floor (from recorded match kill events)
    let should_draw_bodies = match radar.filter {
        PlayerFilter::All | PlayerFilter::DeadOnly | PlayerFilter::ImpostorsAndDead => true,
        PlayerFilter::ImpostorsOnly | PlayerFilter::CrewmatesOnly => false,
    };

    if should_draw_bodies && (state.game_state == 2 || state.game_state == 0 || state.game_state == 1) {
        for body in &state.dead_bodies {
            if body.location.0 == 0.0 && body.location.1 == 0.0 {
                continue;
            }

            let dx = body.location.0 - local_pos.0;
            let dy = body.location.1 - local_pos.1;
            let distance = (dx * dx + dy * dy).sqrt();

            let target_x = center.x + (dx * (radar.scale * 0.5));
            let target_y = match radar.origin {
                LineOrigin::LocalPlayer => center.y - (dy * (radar.scale * 0.5)),
                LineOrigin::BottomCenter => {
                    origin.y
                        - 20.0
                        - (dy.max(0.0) * (radar.scale * 0.5)
                            + (dx.abs() * 0.2 * (radar.scale * 0.5)))
                }
            };

            let pad = 24.0;
            let clamped_x = target_x.clamp(rect.left() + pad, rect.right() - pad);
            let clamped_y = target_y.clamp(rect.top() + pad, rect.bottom() - pad);
            let target_pt = Pos2::new(clamped_x, clamped_y);

            if radar.show_tracers {
                painter.line_segment(
                    [origin, target_pt],
                    Stroke::new(2.0_f32, Color32::from_rgba_unmultiplied(255, 60, 60, 220)),
                );
            }

            painter.circle_filled(target_pt, 6.0, Color32::from_rgb(180, 20, 20));
            painter.circle_stroke(
                target_pt,
                8.0,
                Stroke::new(1.6_f32, Color32::from_rgb(255, 60, 60)),
            );
            painter.text(
                target_pt,
                Align2::CENTER_CENTER,
                "X",
                FontId::proportional(10.0),
                Color32::WHITE,
            );

            let label_text = format!("[BODY] {} - {:.1}m", body.victim_name, distance);
            painter.text(
                Pos2::new(target_pt.x, target_pt.y - 12.0),
                Align2::CENTER_BOTTOM,
                label_text,
                FontId::proportional(11.0),
                Color32::from_rgb(255, 90, 90),
            );
        }
    }
}

fn draw_idle(ui: &mut Ui, state: &OverlayStatus) {
    let message = if !state.connected {
        if state.status_message.is_empty() {
            "Waiting for Among Us".to_string()
        } else {
            state.status_message.clone()
        }
    } else if !state.status_message.is_empty() {
        format!("Connected to Among Us — {}", state.status_message)
    } else {
        "Connected to Among Us — Waiting for players".to_string()
    };
    ui.label(
        RichText::new(message)
            .italics()
            .color(Color32::from_rgb(220, 200, 100)),
    );
}

fn draw_player_card(
    ui: &mut Ui,
    player: &crate::game::player::PlayerSnapshot,
    state: &OverlayStatus,
    radar: &RadarState,
    card_width: f32,
) {
    ui.group(|ui| {
        ui.set_width(card_width);
        let (r, g, b) = color_rgb(player.color_id);
        let player_color = Color32::from_rgb(r, g, b);
        let role_col = radar.theme.role_color32(&player.role);

        ui.horizontal(|ui| {
            let (sq_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
            ui.painter().rect_filled(sq_rect, 3.0, player_color);
            ui.painter().rect_stroke(
                sq_rect,
                3.0,
                Stroke::new(
                    1.0_f32,
                    Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                ),
            );

            ui.vertical(|ui| {
                let name_text = if player.is_dead {
                    RichText::new(format!("{} (DEAD)", player.name))
                        .strong()
                        .strikethrough()
                        .color(Color32::from_rgb(240, 80, 80))
                } else if player.is_local {
                    RichText::new(format!("{} [YOU]", player.name))
                        .strong()
                        .color(Color32::from_rgb(60, 220, 255))
                } else {
                    RichText::new(&player.name).strong()
                };
                ui.label(name_text);

                // Role & Distance line (No Pos)
                ui.horizontal(|ui| {
                    ui.label(color_name(player.color_id));
                    ui.label("|");
                    ui.label(RichText::new(player.role.to_string()).color(role_col));
                    if player.is_local {
                        ui.label(
                            RichText::new("[YOU]")
                                .small()
                                .color(Color32::from_rgb(130, 210, 240)),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("({:.1}m)", player.distance))
                                .small()
                                .color(Color32::from_rgb(180, 190, 210)),
                        );
                    }
                });

                if player.in_vent {
                    ui.label(
                        RichText::new("[IN VENT]")
                            .strong()
                            .small()
                            .color(Color32::from_rgb(255, 140, 40)),
                    );
                }

                if player.shapeshifting {
                    let morph_name = if let Some(tid) = player.shapeshift_target {
                        state
                            .players
                            .iter()
                            .find(|p| p.player_id == tid)
                            .map(|p| p.name.as_str())
                            .unwrap_or("Unknown")
                    } else {
                        "Target"
                    };
                    ui.label(
                        RichText::new(format!("[SHAPESHIFTED AS: {morph_name}]"))
                            .strong()
                            .small()
                            .color(Color32::from_rgb(255, 90, 120)),
                    );
                }

                if !player.friend_code.is_empty() {
                    ui.label(
                        RichText::new(format!("ID: {}", player.friend_code))
                            .small()
                            .color(Color32::from_rgb(150, 170, 200)),
                    );
                }
            });
        });
    });
}

fn draw_player_list(
    ui: &mut Ui,
    state: &OverlayStatus,
    action: &mut OverlayAction,
    radar: &RadarState,
) {
    let num_players = state.players.len();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Players ({num_players})")).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    RichText::new("Clear")
                        .small()
                        .color(Color32::from_rgb(255, 110, 110)),
                )
                .on_hover_text("Clear kills, bodies & shapeshifts")
                .clicked()
            {
                *action = OverlayAction::ClearLogs;
            }
            if ui
                .button(
                    RichText::new("Export (.txt)")
                        .small()
                        .color(radar.theme.accent_color32()),
                )
                .on_hover_text("Save player data & match logs to match_logs/")
                .clicked()
            {
                *action = OverlayAction::ExportMatchLog;
            }
        });
    });
    ui.add_space(3.0);

    let avail_w = ui.available_width();
    let num_cols = if avail_w >= 640.0 {
        3
    } else if avail_w >= 360.0 {
        2
    } else {
        1
    };

    let max_h = if state.kill_events.is_empty() {
        ui.available_height().max(200.0)
    } else {
        (ui.available_height() - 90.0).max(140.0)
    };

    let gap = 6.0;
    let col_w = ((avail_w - (num_cols as f32 - 1.0) * gap) / num_cols as f32).max(150.0);

    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        ui.horizontal_top(|ui| {
            for col_idx in 0..num_cols {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    for (idx, player) in state.players.iter().enumerate() {
                        if idx % num_cols == col_idx {
                            draw_player_card(ui, player, state, radar, col_w - 8.0);
                            ui.add_space(4.0);
                        }
                    }
                });
                if col_idx + 1 < num_cols {
                    ui.add_space(gap);
                }
            }
        });
    });

    if !state.kill_events.is_empty() {
        ui.add_space(4.0);
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Recent Kills")
                        .strong()
                        .color(Color32::from_rgb(255, 90, 90)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            RichText::new("Clear")
                                .small()
                                .color(Color32::from_rgb(180, 180, 180)),
                        )
                        .clicked()
                    {
                        *action = OverlayAction::ClearLogs;
                    }
                });
            });
            ui.add_space(2.0);

            for event in state.kill_events.iter().take(4) {
                ui.label(
                    RichText::new(&event.message)
                        .small()
                        .color(Color32::from_rgb(255, 130, 130)),
                );
            }
        });
    }
}

fn draw_event_logs(
    ui: &mut Ui,
    state: &OverlayStatus,
    action: &mut OverlayAction,
    radar: &RadarState,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Console Logs").strong().size(13.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    RichText::new("Clear")
                        .small()
                        .color(Color32::from_rgb(255, 110, 110)),
                )
                .on_hover_text("Clear all recorded kills and shapeshifts")
                .clicked()
            {
                *action = OverlayAction::ClearLogs;
            }
            if ui
                .button(
                    RichText::new("Export (.txt)")
                        .small()
                        .color(radar.theme.accent_color32()),
                )
                .clicked()
            {
                *action = OverlayAction::ExportMatchLog;
            }
        });
    });
    ui.add_space(4.0);

    let max_h = ui.available_height().max(200.0);
    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        // Meeting Voting Matrix
        let has_votes =
            state.players.iter().any(|p| p.voted_for.is_some()) || state.game_state == 3;
        if has_votes {
            ui.group(|ui| {
                ui.label(
                    RichText::new("Meeting Voting Matrix")
                        .strong()
                        .color(Color32::from_rgb(240, 200, 80)),
                );
                ui.label(
                    RichText::new("Real-time live votes before meeting ends:")
                        .small()
                        .italics()
                        .color(Color32::from_rgb(160, 180, 210)),
                );
                ui.add_space(3.0);

                for p in &state.players {
                    if p.is_dead {
                        continue;
                    }
                    let vote_text = match p.voted_for {
                        Some(-1) => RichText::new("SKIPPED VOTE")
                            .italics()
                            .color(Color32::from_rgb(200, 200, 120)),
                        Some(target_id) => {
                            let target_name = state
                                .players
                                .iter()
                                .find(|t| t.player_id == target_id as u8)
                                .map(|t| t.name.as_str())
                                .unwrap_or("Unknown");
                            RichText::new(format!("Voted for ➜ {target_name}"))
                                .strong()
                                .color(Color32::from_rgb(255, 100, 100))
                        }
                        None => RichText::new("Thinking... (Not voted yet)")
                            .small()
                            .color(Color32::from_rgb(140, 150, 160)),
                    };

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&p.name).strong().color(Color32::WHITE));
                        ui.label(":");
                        ui.label(vote_text);
                    });
                }
            });
            ui.add_space(6.0);
        }

        // Console Logging Toggles
        ui.group(|ui| {
            ui.label(
                RichText::new("Console Logging Controls")
                    .strong()
                    .color(radar.theme.accent_color32()),
            );
            ui.label(
                RichText::new("Toggle real-time terminal output on and off:")
                    .small()
                    .italics()
                    .color(Color32::from_rgb(160, 180, 210)),
            );
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                let kill_text = if state.log_config.log_kills {
                    "Kill Logs: ON"
                } else {
                    "Kill Logs: OFF"
                };
                let kill_col = if state.log_config.log_kills {
                    Color32::from_rgb(80, 220, 120)
                } else {
                    Color32::from_rgb(180, 180, 180)
                };
                if ui
                    .button(RichText::new(kill_text).small().color(kill_col))
                    .clicked()
                {
                    *action = OverlayAction::ToggleLogKills;
                }

                let state_text = if state.log_config.log_game_state {
                    "State Logs: ON"
                } else {
                    "State Logs: OFF"
                };
                let state_col = if state.log_config.log_game_state {
                    Color32::from_rgb(80, 220, 120)
                } else {
                    Color32::from_rgb(180, 180, 180)
                };
                if ui
                    .button(RichText::new(state_text).small().color(state_col))
                    .clicked()
                {
                    *action = OverlayAction::ToggleLogGameState;
                }

                let roster_text = if state.log_config.log_player_list {
                    "Player Roster Logs: ON"
                } else {
                    "Player Roster Logs: OFF"
                };
                let roster_col = if state.log_config.log_player_list {
                    Color32::from_rgb(80, 220, 120)
                } else {
                    Color32::from_rgb(180, 180, 180)
                };
                if ui
                    .button(RichText::new(roster_text).small().color(roster_col))
                    .clicked()
                {
                    *action = OverlayAction::ToggleLogPlayerList;
                }
            });
        });

        ui.add_space(6.0);

        // Shapeshifter Disguises Feed
        ui.group(|ui| {
            ui.label(
                RichText::new("Shapeshifts")
                    .strong()
                    .color(Color32::from_rgb(255, 120, 160)),
            );
            ui.add_space(2.0);

            if state.disguise_events.is_empty() {
                ui.label(
                    RichText::new("No shapeshifts detected yet")
                        .italics()
                        .color(Color32::from_rgb(160, 170, 180)),
                );
            } else {
                for event in &state.disguise_events {
                    ui.label(
                        RichText::new(&event.message)
                            .strong()
                            .color(Color32::from_rgb(255, 140, 180)),
                    );
                }
            }
        });

        ui.add_space(6.0);

        // Kill & Death Event Feed
        ui.group(|ui| {
            ui.label(
                RichText::new("Kill Feed")
                    .strong()
                    .color(Color32::from_rgb(255, 100, 100)),
            );
            ui.add_space(2.0);

            if state.kill_events.is_empty() {
                ui.label(
                    RichText::new("No kills detected yet")
                        .italics()
                        .color(Color32::from_rgb(160, 170, 180)),
                );
            } else {
                for event in &state.kill_events {
                    ui.label(
                        RichText::new(&event.message)
                            .strong()
                            .color(Color32::from_rgb(255, 120, 120)),
                    );
                }
            }
        });
    });
}

fn draw_cheat_sheet(ui: &mut Ui) {
    ui.label(RichText::new("Task Timing Cheat Sheet").strong().size(13.0));
    ui.add_space(4.0);

    let max_h = ui.available_height().max(200.0);
    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        ui.group(|ui| {
            ui.label(
                RichText::new("Instant / Very Short Tasks (1 to 3 seconds)")
                    .strong()
                    .color(Color32::from_rgb(100, 220, 150)),
            );
            ui.add_space(2.0);

            draw_task_item(
                ui,
                "Swipe Card / Insert Keys",
                "2 to 3 seconds",
                Some("allow extra time if you want to fake a failed first attempt"),
            );
            draw_task_item(ui, "Clean Vent / Chart Course", "2 to 3 seconds", None);
            draw_task_item(ui, "Divert Power", "1 to 2 seconds per stage", None);
        });

        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(
                RichText::new("Standard / Medium Tasks (4 to 8 seconds)")
                    .strong()
                    .color(Color32::from_rgb(240, 200, 80)),
            );
            ui.add_space(2.0);

            draw_task_item(ui, "Fix Wiring", "4 to 5 seconds per panel", None);
            draw_task_item(ui, "Empty Garbage", "3 to 4 seconds", None);
            draw_task_item(ui, "Fuel Engines", "4 to 6 seconds per stage", None);
            draw_task_item(ui, "Enter ID Code", "5 to 6 seconds", None);
        });

        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(
                RichText::new("Longer Tasks (9 to 20+ seconds)")
                    .strong()
                    .color(Color32::from_rgb(255, 120, 100)),
            );
            ui.add_space(2.0);

            draw_task_item(ui, "Download / Upload Data", "Around 9 to 10 seconds", None);
            draw_task_item(
                ui,
                "Submit MedBay Scan",
                "Exactly 10 seconds",
                Some("visual animation"),
            );
            draw_task_item(
                ui,
                "Clear Asteroids",
                "Around 15 seconds",
                Some("varies based on shots"),
            );
            draw_task_item(
                ui,
                "Start Reactor / Unlock Manifolds",
                "15 to 20 seconds",
                None,
            );
        });
    });
}

fn draw_task_item(ui: &mut Ui, name: &str, timing: &str, note: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("•")
                .strong()
                .color(Color32::from_rgb(140, 170, 220)),
        );
        ui.label(
            RichText::new(name)
                .strong()
                .color(Color32::from_rgb(220, 230, 245)),
        );
        ui.label(RichText::new(format!(": {timing}")).color(Color32::from_rgb(180, 210, 250)));
        if let Some(n) = note {
            ui.label(
                RichText::new(format!("({n})"))
                    .small()
                    .italics()
                    .color(Color32::from_rgb(150, 160, 180)),
            );
        }
    });
}

fn draw_theme_settings(ui: &mut Ui, radar: &mut RadarState) {
    ui.label(
        RichText::new("Theme & Color Customization")
            .strong()
            .size(13.0),
    );
    ui.label(
        RichText::new("Changes are automatically saved to theme.toml")
            .small()
            .italics()
            .color(Color32::from_rgb(150, 170, 200)),
    );
    ui.add_space(4.0);

    let max_h = ui.available_height().max(200.0);
    ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        ui.group(|ui| {
            ui.label(
                RichText::new("Color Presets")
                    .strong()
                    .color(radar.theme.accent_color32()),
            );
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                let presets = [
                    ("dark", ThemeConfig::dark_modern()),
                    ("purple", ThemeConfig::midnight_purple()),
                    ("neon", ThemeConfig::cyberpunk_neon()),
                    ("green", ThemeConfig::matrix_emerald()),
                    ("crimson", ThemeConfig::crimson_blood()),
                ];

                for (name, preset) in presets {
                    let is_active = radar.theme.name == name;
                    if ui.selectable_label(is_active, name).clicked() {
                        radar.theme = preset;
                        radar.theme.save();
                    }
                }
            });
        });

        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(
                RichText::new("All Role Colors")
                    .strong()
                    .color(radar.theme.accent_color32()),
            );
            ui.add_space(4.0);

            let mut changed = false;

            if draw_compact_color_edit(ui, "Crewmate", &mut radar.theme.crewmate) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Impostor", &mut radar.theme.impostor) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Scientist", &mut radar.theme.scientist) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Engineer", &mut radar.theme.engineer) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Guardian Angel", &mut radar.theme.guardian_angel) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Shapeshifter", &mut radar.theme.shapeshifter) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Crewmate Ghost", &mut radar.theme.crewmate_ghost) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Impostor Ghost", &mut radar.theme.impostor_ghost) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Phantom", &mut radar.theme.phantom) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Tracker", &mut radar.theme.tracker) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Noisemaker", &mut radar.theme.noisemaker) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Detective", &mut radar.theme.detective) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Viper", &mut radar.theme.viper) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Judge", &mut radar.theme.judge) {
                changed = true;
            }

            if changed {
                radar.theme.name = "Custom".into();
                radar.theme.save();
            }
        });

        ui.add_space(6.0);

        ui.group(|ui| {
            ui.label(
                RichText::new("Custom UI Palette")
                    .strong()
                    .color(radar.theme.accent_color32()),
            );
            ui.add_space(4.0);

            let mut changed = false;

            if draw_compact_color_edit(ui, "Window Background", &mut radar.theme.background) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Canvas Background", &mut radar.theme.canvas) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Borders & Outlines", &mut radar.theme.border) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Accent Buttons & Highlights", &mut radar.theme.accent) {
                changed = true;
            }
            if draw_compact_color_edit(ui, "Header Title Text", &mut radar.theme.header_text) {
                changed = true;
            }
            if draw_compact_color_edit(
                ui,
                "Local Player Reticle [YOU]",
                &mut radar.theme.local_player,
            ) {
                changed = true;
            }
            if draw_compact_color_edit(
                ui,
                "Impostor Snaplines & Highlights",
                &mut radar.theme.impostor_line,
            ) {
                changed = true;
            }

            if changed {
                radar.theme.name = "Custom".into();
                radar.theme.save();
            }
        });

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Reset to Default Theme")
                        .small()
                        .color(Color32::from_rgb(220, 140, 140)),
                )
                .clicked()
            {
                radar.theme = ThemeConfig::dark_modern();
                radar.theme.save();
            }
        });
    });
}

fn draw_compact_color_edit(ui: &mut Ui, label: &str, rgba: &mut [u8; 4]) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        let (sq_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), Sense::hover());
        let col = Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3]);
        ui.painter().rect_filled(sq_rect, 3.0, col);
        ui.painter().rect_stroke(
            sq_rect,
            3.0,
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 120)),
        );

        ui.label(RichText::new(label).strong().size(12.0));
    });

    ui.horizontal(|ui| {
        ui.add_space(18.0);

        let mut hex = format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2]);
        let hex_resp = ui.add(
            egui::TextEdit::singleline(&mut hex)
                .desired_width(62.0)
                .hint_text("#RRGGBB"),
        );
        if hex_resp.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let clean = hex.trim().trim_start_matches('#');
            if clean.len() == 6 {
                if let Ok(val) = u32::from_str_radix(clean, 16) {
                    rgba[0] = ((val >> 16) & 0xFF) as u8;
                    rgba[1] = ((val >> 8) & 0xFF) as u8;
                    rgba[2] = (val & 0xFF) as u8;
                    changed = true;
                }
            }
        }

        ui.add_space(2.0);
        ui.label(
            RichText::new("R")
                .small()
                .color(Color32::from_rgb(255, 100, 100)),
        );
        if ui
            .add(egui::DragValue::new(&mut rgba[0]).range(0..=255).speed(1.0))
            .changed()
        {
            changed = true;
        }

        ui.label(
            RichText::new("G")
                .small()
                .color(Color32::from_rgb(100, 255, 100)),
        );
        if ui
            .add(egui::DragValue::new(&mut rgba[1]).range(0..=255).speed(1.0))
            .changed()
        {
            changed = true;
        }

        ui.label(
            RichText::new("B")
                .small()
                .color(Color32::from_rgb(100, 150, 255)),
        );
        if ui
            .add(egui::DragValue::new(&mut rgba[2]).range(0..=255).speed(1.0))
            .changed()
        {
            changed = true;
        }

        ui.label(
            RichText::new("A")
                .small()
                .color(Color32::from_rgb(200, 200, 200)),
        );
        if ui
            .add(egui::DragValue::new(&mut rgba[3]).range(0..=255).speed(1.0))
            .changed()
        {
            changed = true;
        }
    });

    ui.add_space(2.0);
    changed
}

fn draw_resize_handle(ui: &mut Ui, action: &mut OverlayAction) {
    let rect = ui.clip_rect();
    let grip_size = Vec2::new(14.0, 14.0);
    let grip_rect = egui::Rect::from_min_size(
        Pos2::new(
            rect.right() - grip_size.x - 2.0,
            rect.bottom() - grip_size.y - 2.0,
        ),
        grip_size,
    );

    let grip_resp = ui.interact(grip_rect, ui.id().with("window_resize_grip"), Sense::drag());
    if grip_resp.drag_started() || (grip_resp.dragged() && ui.input(|i| i.pointer.primary_down())) {
        *action = OverlayAction::ResizeWindow;
    }

    // Draw subtle 3-line diagonal grip icon
    let p = ui.painter();
    let col = Color32::from_rgba_unmultiplied(200, 210, 230, 90);
    for i in 0..3 {
        let off = (i as f32) * 3.5;
        p.line_segment(
            [
                Pos2::new(grip_rect.right() - off, grip_rect.bottom()),
                Pos2::new(grip_rect.right(), grip_rect.bottom() - off),
            ],
            Stroke::new(1.2_f32, col),
        );
    }
}
