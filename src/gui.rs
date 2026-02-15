use iced::widget::{column, container, mouse_area, row, scrollable, text, svg, tooltip, Column};
use iced::{color, Element, Font, Subscription, Theme};

use crate::model::{self, Environment, State};
use crate::notifications::Notifier;
use crate::watcher::{RegistryWatcher, WatchEvent};

/// Set the macOS dock icon from embedded PNG bytes.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::ClassType;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(include_bytes!("../assets/icon.png"));
    let image = NSImage::initWithData(NSImage::alloc(), &data);
    if let Some(image) = image {
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}

const MONO: Font = Font::MONOSPACE;

// ---------------------------------------------------------------------------
// Lucide SVG icons (24x24 viewBox, stroke-based)
// ---------------------------------------------------------------------------

const ICON_VOLUME_2: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/></svg>"#;

const ICON_VOLUME_X: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5"/><line x1="22" y1="9" x2="16" y2="15"/><line x1="16" y1="9" x2="22" y2="15"/></svg>"#;

const ICON_SUN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>"#;

const ICON_MOON: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>"#;

const ICON_BELL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>"#;

const ICON_BELL_OFF: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M13.73 21a2 2 0 0 1-3.46 0"/><path d="M18.63 13A17.89 17.89 0 0 1 18 8"/><path d="M6.26 6.26A5.86 5.86 0 0 0 6 8c0 7-3 9-3 9h14"/><path d="M18 8a6 6 0 0 0-9.33-5"/><line x1="1" y1="1" x2="23" y2="23"/></svg>"#;

const ICON_SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/></svg>"#;

/// Build an iced Svg handle from an embedded byte slice, rendered at the given size and color.
fn icon_svg(data: &'static [u8], size: f32, color: iced::Color) -> Element<'static, Message> {
    let handle = svg::Handle::from_memory(data);
    svg(handle)
        .width(size)
        .height(size)
        .style(move |_theme, _status| svg::Style {
            color: Some(color),
        })
        .into()
}

/// Style for tooltip bubbles — background, rounded border, subtle shadow.
fn tooltip_style(pal: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(iced::Background::Color(pal.card_bg)),
        border: iced::Border {
            color: pal.card_border,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: iced::Shadow {
            color: pal.card_shadow,
            offset: iced::Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        text_color: Some(pal.fg),
    }
}

/// Theme-aware color palette.
#[derive(Clone, Copy)]
struct Palette {
    fg: iced::Color,
    muted: iced::Color,
    card_bg: iced::Color,
    card_border: iced::Color,
    card_shadow: iced::Color,
    hover_bg: iced::Color,
    green: iced::Color,
    yellow: iced::Color,
    red: iced::Color,
    gray: iced::Color,
    cyan: iced::Color,
}

fn palette(dark_mode: bool) -> Palette {
    if dark_mode {
        Palette {
            fg: color!(0xf8f8f2),
            muted: color!(0x8890a0),
            card_bg: color!(0x282a36),
            card_border: color!(0x383a4a),
            card_shadow: color!(0x000000, 0.4),
            hover_bg: color!(0x343648),
            green: color!(0x50fa7b),
            yellow: color!(0xf1fa8c),
            red: color!(0xff5555),
            gray: color!(0x6272a4),
            cyan: color!(0x8be9fd),
        }
    } else {
        Palette {
            fg: color!(0x1e1e2e),
            muted: color!(0x9399a8),
            card_bg: color!(0xffffff),
            card_border: color!(0xeaeaea),
            card_shadow: color!(0x000000, 0.08),
            hover_bg: color!(0xf0f0f4),
            green: color!(0x2da44e),
            yellow: color!(0x9a6700),
            red: color!(0xd1242f),
            gray: color!(0xa0a8b8),
            cyan: color!(0x0969da),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Tick,
    WatchEvent,
    ToggleGlobalMute,
    ToggleUnitMute {
        env_id: String,
        unit_name: String,
    },
    ToggleGlobalNotifications,
    ToggleUnitNotifications {
        env_id: String,
        unit_name: String,
    },
    ToggleTheme,
    OpenBrowser { port: u16 },
    TerminateEnv { pid: u32 },
    HoverUnit {
        env_id: String,
        unit_name: String,
    },
    UnhoverUnit,
    Quit,
}

struct App {
    envs: Vec<Environment>,
    notifier: Notifier,
    dark_mode: bool,
    hovered_unit: Option<(String, String)>, // (env_id, unit_name)
}

pub fn run() {
    #[cfg(target_os = "macos")]
    set_dock_icon();

    let icon = iced::window::icon::from_file_data(
        include_bytes!("../assets/icon.png"),
        None,
    )
    .ok();

    iced::application("Sutra", update, view)
        .theme(theme)
        .subscription(subscription)
        .window(iced::window::Settings {
            size: iced::Size::new(480.0, 420.0),
            icon,
            ..Default::default()
        })
        .run_with(|| {
            let envs = model::load_all();
            let mut notifier = Notifier::new();
            notifier.process(&envs);
            (
                App {
                    envs,
                    notifier,
                    dark_mode: false,
                    hovered_unit: None,
                },
                iced::Task::none(),
            )
        })
        .expect("failed to launch GUI");
}

fn update(app: &mut App, message: Message) -> iced::Task<Message> {
    match message {
        Message::Tick | Message::WatchEvent => {
            app.envs = model::load_all();
            app.notifier.process(&app.envs);
        }
        Message::ToggleGlobalMute => {
            app.notifier.toggle_global_mute();
        }
        Message::ToggleTheme => {
            app.dark_mode = !app.dark_mode;
        }
        Message::ToggleUnitMute {
            env_id,
            unit_name,
        } => {
            app.notifier.toggle_unit_mute(&env_id, &unit_name);
        }
        Message::ToggleGlobalNotifications => {
            app.notifier.toggle_global_notifications();
        }
        Message::ToggleUnitNotifications {
            env_id,
            unit_name,
        } => {
            app.notifier.toggle_unit_notifications(&env_id, &unit_name);
        }
        Message::OpenBrowser { port } => {
            let _ = std::process::Command::new("open")
                .arg(format!("http://localhost:{port}"))
                .spawn();
        }
        Message::TerminateEnv { pid } => {
            if let Ok(raw_pid) = i32::try_from(pid) {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(raw_pid),
                    nix::sys::signal::Signal::SIGTERM,
                );
            }
        }
        Message::HoverUnit { env_id, unit_name } => {
            app.hovered_unit = Some((env_id, unit_name));
        }
        Message::UnhoverUnit => {
            app.hovered_unit = None;
        }
        Message::Quit => {
            return iced::window::get_latest().and_then(iced::window::close);
        }
    }
    iced::Task::none()
}

fn view(app: &App) -> Element<'_, Message> {
    let pal = palette(app.dark_mode);

    // Toolbar -- minimal, right-aligned controls with SVG icons
    let toolbar = {
        let mute_icon = if app.notifier.global_mute {
            ICON_VOLUME_X
        } else {
            ICON_VOLUME_2
        };

        let notif_icon = if app.notifier.global_notifications_off {
            ICON_BELL_OFF
        } else {
            ICON_BELL
        };

        let theme_icon = if app.dark_mode {
            ICON_SUN
        } else {
            ICON_MOON
        };

        let icon_color = pal.fg;

        let mute_tip = if app.notifier.global_mute { "Unmute all sounds" } else { "Mute all sounds" };
        let notif_tip = if app.notifier.global_notifications_off { "Enable notifications" } else { "Disable notifications" };
        let theme_tip = if app.dark_mode { "Switch to light mode" } else { "Switch to dark mode" };

        let tip_style = tooltip_style(pal);
        let tip_style2 = tooltip_style(pal);
        let tip_style3 = tooltip_style(pal);

        let toolbar_row = row![
            iced::widget::horizontal_space(),
            tooltip(
                mouse_area(icon_svg(mute_icon, 16.0, icon_color))
                    .on_press(Message::ToggleGlobalMute),
                text(mute_tip).size(11),
                tooltip::Position::Bottom,
            )
            .style(tip_style)
            .gap(4),
            text("\u{00b7}").size(8).color(pal.muted), // middle dot separator
            tooltip(
                mouse_area(icon_svg(notif_icon, 16.0, icon_color))
                    .on_press(Message::ToggleGlobalNotifications),
                text(notif_tip).size(11),
                tooltip::Position::Bottom,
            )
            .style(tip_style2)
            .gap(4),
            text("\u{00b7}").size(8).color(pal.muted),
            tooltip(
                mouse_area(icon_svg(theme_icon, 16.0, icon_color))
                    .on_press(Message::ToggleTheme),
                text(theme_tip).size(11),
                tooltip::Position::Bottom,
            )
            .style(tip_style3)
            .gap(4),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        container(toolbar_row)
            .padding(iced::Padding::from([6.0, 16.0]))
            .width(iced::Fill)
    };

    if app.envs.is_empty() {
        let dir_label = model::state_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.dev-runner/".into());
        let empty_msg = column![
            text("No environments found.").size(14).color(pal.muted),
            text(format!("Watching {} for environments", dir_label))
                .size(12)
                .color(pal.muted),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center);
        return column![
            toolbar,
            container(empty_msg)
                .padding(40)
                .center_x(iced::Fill)
                .center_y(iced::Fill),
        ]
        .into();
    }

    let mut items: Vec<Element<Message>> = Vec::new();

    for env in app.envs.iter() {
        items.push(env_card(env, &app.notifier, &pal, &app.hovered_unit));
    }

    let content = Column::with_children(items).spacing(12).width(iced::Fill);

    column![
        toolbar,
        scrollable(
            container(content)
                .padding(iced::Padding {
                    top: 4.0,
                    right: 16.0,
                    bottom: 16.0,
                    left: 16.0,
                })
                .width(iced::Fill)
        ),
    ]
    .into()
}

fn env_card(
    env: &Environment,
    notifier: &Notifier,
    pal: &Palette,
    hovered_unit: &Option<(String, String)>,
) -> Element<'static, Message> {
    let alive_color = if env.alive { pal.green } else { pal.gray };

    // Header: alive dot + name + elapsed + terminate button
    let mut header = row![
        text("\u{25cf}").size(10).color(alive_color),
        text(env.display_name().to_string())
            .size(15)
            .color(pal.fg)
            .font(Font::DEFAULT),
        iced::widget::horizontal_space(),
        text(env.elapsed_string())
            .size(12)
            .color(pal.muted),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center);

    if env.alive {
        let stop_btn: Element<'static, Message> = tooltip(
            mouse_area(
                icon_svg(ICON_SQUARE, 10.0, pal.red),
            )
            .on_press(Message::TerminateEnv { pid: env.pid }),
            text("Terminate environment").size(11),
            tooltip::Position::Top,
        )
        .style(tooltip_style(*pal))
        .gap(4)
        .into();
        header = header.push(stop_btn);
    }

    let mut card_col = column![header].spacing(8);

    if !env.units.is_empty() {
        // Compute fixed pixel widths for table-column alignment.
        // ~7.2px per char at size 12 monospace is a reasonable approximation.
        const CHAR_W: f32 = 7.2;

        let max_name_chars = env.units.iter().map(|u| u.name.len()).max().unwrap_or(0);
        let name_col_w = (max_name_chars as f32 * CHAR_W).ceil() + 4.0;

        let has_any_port = env.units.iter().any(|u| env.port_for(&u.name).is_some());
        let port_col_w: f32 = if has_any_port { 6.0 * CHAR_W + 4.0 } else { 0.0 };

        let max_state_chars = env
            .units
            .iter()
            .map(|u| u.state.to_string().len())
            .max()
            .unwrap_or(0);
        let state_col_w = (max_state_chars as f32 * CHAR_W).ceil() + 4.0;

        let muted_color = pal.muted;
        let cyan = pal.cyan;
        let hover_bg = pal.hover_bg;

        let mut unit_col = Column::new().spacing(3);

        for unit in &env.units {
            let is_muted = notifier.is_unit_muted(&env.id, &unit.name);
            let is_notif_off = notifier.is_unit_notifications_off(&env.id, &unit.name);
            let color = state_color(&unit.state, pal);
            let indicator = unit.state.display_indicator();

            let name_color = if is_muted { pal.muted } else { pal.fg };

            // Per-unit icons (left of indicator for alignment)
            let mute_icon_data = if is_muted { ICON_VOLUME_X } else { ICON_VOLUME_2 };
            let notif_icon_data = if is_notif_off { ICON_BELL_OFF } else { ICON_BELL };

            // Fixed-width cells for true table-column alignment
            let name_cell = container(
                text(unit.name.clone()).size(12).color(name_color).font(MONO),
            )
            .width(name_col_w);

            let port_cell: Element<'static, Message> = if has_any_port {
                let label = match env.port_for(&unit.name) {
                    Some(p) => format!(":{p}"),
                    None => String::new(),
                };
                container(text(label).size(11).color(cyan).font(MONO))
                    .width(port_col_w)
                    .into()
            } else {
                text("").into()
            };

            let state_cell = container(
                text(unit.state.to_string()).size(12).color(color),
            )
            .width(state_col_w);

            let unit_mute_tip = if is_muted {
                format!("Unmute {}", unit.name)
            } else {
                format!("Mute {}", unit.name)
            };
            let unit_notif_tip = if is_notif_off {
                format!("Enable notifications for {}", unit.name)
            } else {
                format!("Disable notifications for {}", unit.name)
            };

            let mut unit_row = row![
                // icon pair: mute + bell (fixed 12px each)
                tooltip(
                    mouse_area(icon_svg(mute_icon_data, 12.0, if is_muted { pal.muted } else { pal.fg }))
                        .on_press(Message::ToggleUnitMute {
                            env_id: env.id.clone(),
                            unit_name: unit.name.clone(),
                        }),
                    text(unit_mute_tip).size(11),
                    tooltip::Position::Top,
                )
                .style(tooltip_style(*pal))
                .gap(4),
                tooltip(
                    mouse_area(icon_svg(notif_icon_data, 12.0, if is_notif_off { pal.muted } else { pal.fg }))
                        .on_press(Message::ToggleUnitNotifications {
                            env_id: env.id.clone(),
                            unit_name: unit.name.clone(),
                        }),
                    text(unit_notif_tip).size(11),
                    tooltip::Position::Top,
                )
                .style(tooltip_style(*pal))
                .gap(4),
                // indicator dot (fixed 14px container)
                container(text(indicator.to_string()).size(11).color(color)).width(14.0),
                name_cell,
                port_cell,
                state_cell,
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);

            if let Some(ref detail) = unit.detail {
                unit_row = unit_row.push(text(detail.clone()).size(11).color(muted_color));
            }

            // "open" link for units with ports
            if let Some(port) = env.port_for(&unit.name) {
                unit_row = unit_row.push(iced::widget::horizontal_space());
                unit_row = unit_row.push(
                    tooltip(
                        mouse_area(text("\u{2197}").size(13).color(cyan))
                            .on_press(Message::OpenBrowser { port }),
                        text("Open in browser").size(11),
                        tooltip::Position::Top,
                    )
                    .style(tooltip_style(*pal))
                    .gap(4),
                );
            }

            // Wrap the row in a container for hover highlighting
            let is_hovered = hovered_unit
                .as_ref()
                .map(|(eid, uname)| eid == &env.id && uname == &unit.name)
                .unwrap_or(false);

            let row_bg = if is_hovered {
                Some(iced::Background::Color(hover_bg))
            } else {
                None
            };

            let row_container: Element<'static, Message> = container(unit_row)
                .width(iced::Fill)
                .padding(iced::Padding::from([2.0, 4.0]))
                .style(move |_theme| container::Style {
                    background: row_bg,
                    border: iced::Border {
                        color: iced::Color::TRANSPARENT,
                        width: 0.0,
                        radius: 4.0.into(),
                    },
                    shadow: iced::Shadow::default(),
                    text_color: None,
                })
                .into();

            let env_id = env.id.clone();
            let unit_name = unit.name.clone();

            let unit_element: Element<'static, Message> = mouse_area(row_container)
                .on_enter(Message::HoverUnit {
                    env_id,
                    unit_name,
                })
                .on_exit(Message::UnhoverUnit)
                .into();

            unit_col = unit_col.push(unit_element);
        }

        card_col = card_col.push(unit_col);
    }

    let card_bg = pal.card_bg;
    let card_border = pal.card_border;
    let card_shadow = pal.card_shadow;

    container(card_col.width(iced::Fill))
        .padding(iced::Padding::from([14.0, 16.0]))
        .width(iced::Fill)
        .style(move |_theme| container::Style {
            background: Some(iced::Background::Color(card_bg)),
            border: iced::Border {
                color: card_border,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: iced::Shadow {
                color: card_shadow,
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 4.0,
            },
            text_color: None,
        })
        .into()
}

fn state_color(state: &State, pal: &Palette) -> iced::Color {
    match state {
        State::Running | State::Ready => pal.green,
        State::Building | State::Starting => pal.yellow,
        State::Failed => pal.red,
        State::Stopped | State::None | State::Other(_) => pal.gray,
    }
}

fn theme(app: &App) -> Theme {
    if app.dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
}

fn subscription(_app: &App) -> Subscription<Message> {
    let tick = iced::time::every(std::time::Duration::from_secs(2)).map(|_| Message::Tick);

    let watcher = Subscription::run(watch_registry);

    let keyboard = iced::keyboard::on_key_press(|key, modifiers| {
        if modifiers.command() {
            if let iced::keyboard::Key::Character(c) = key.as_ref() {
                if c == "q" {
                    return Some(Message::Quit);
                }
            }
        }
        None
    });

    Subscription::batch([tick, watcher, keyboard])
}

fn watch_registry() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(32, |mut sender| async move {
        use iced::futures::SinkExt;
        use iced::futures::StreamExt;

        let Ok(watcher) = RegistryWatcher::new() else {
            std::future::pending::<()>().await;
            return;
        };
        let rx = watcher.rx;

        // Bridge the std::sync::mpsc channel to an async futures::channel::mpsc
        // so we don't block iced's event loop.
        let (mut async_tx, mut async_rx) =
            iced::futures::channel::mpsc::channel::<WatchEvent>(32);
        std::thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                // Use try_send to avoid needing async; drop events if the
                // channel is full (the next tick will catch up).
                if async_tx.try_send(event).is_err() {
                    // Channel closed or full — exit thread
                    if async_tx.is_closed() {
                        break;
                    }
                }
            }
        });

        while async_rx.next().await.is_some() {
            let _ = sender.send(Message::WatchEvent).await;
        }
    })
}
