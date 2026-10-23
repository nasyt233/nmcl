use crate::app::{App, ClickTarget, EditField, SettingsSide, UiLayout};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::common::{clickable, BG, DIRT, GOLD, GRAY, GRAY_DIM, GRASS, TEXT};

pub fn draw_settings(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(20)])
        .split(area);

    draw_side(f, app, cols[0], layout);
    draw_content(f, app, cols[1], layout);
}

fn draw_side(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(GRAY_DIM))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    for (i, side) in SettingsSide::all().iter().enumerate() {
        let active = app.settings_side == *side;
        let label = format!(" {} ", side.title());
        let rect = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
        let style = if active {
            Style::default().bg(GOLD).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(BG).fg(TEXT)
        };
        f.render_widget(Paragraph::new(Line::from(Span::styled(label, style))), rect);
        clickable(layout, rect, ClickTarget::SettingsSideItem(*side));
    }
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let block = Block::default()
        .title(Line::from(Span::styled(
            format!(" {} ", app.settings_side.title()),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIRT))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    match app.settings_side {
        SettingsSide::General => draw_general(f, app, inner, layout),
        SettingsSide::Java => draw_java(f, app, inner, layout),
        SettingsSide::Launcher => draw_launcher(f, app, inner),
        SettingsSide::About => draw_about(f, app, inner),
    }
}

fn draw_general(f: &mut Frame, app: &App, area: Rect, layout: &mut UiLayout) {
    render_input(
        f, app, area, 0, "玩家名",
        &app.profiles[app.current_profile].name,
        app.editing == EditField::PlayerName,
        ClickTarget::EditField(EditField::PlayerName),
        layout,
    );
    render_note(f, area, 3, "离线模式下使用此用户名进入游戏");
    render_note(f, area, 5, "正版登录功能将在后续版本提供");
}

fn draw_java(f: &mut Frame, app: &App, area: Rect, layout: &mut UiLayout) {
    render_input(
        f, app, area, 0, "Java 路径",
        &app.settings.java_path,
        app.editing == EditField::JavaPath,
        ClickTarget::EditField(EditField::JavaPath),
        layout,
    );
    render_note(f, area, 3, "留空则使用环境变量 NMCL_JAVA 或系统 PATH 中的 java");
    render_note(f, area, 5, "每个版本可在「版本」页的版本管理设置中单独指定 Java 路径");
}

fn draw_launcher(f: &mut Frame, app: &App, area: Rect) {
    let mem = app.settings.default_max_memory_mb;
    let items = vec![
        Line::from(Span::styled(" 启动器设置", Style::default().fg(GOLD).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  默认最大内存  ", Style::default().fg(TEXT)),
            Span::styled(format!("{} MB", mem), Style::default().fg(GRASS).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(Span::styled("   (版本管理中的值优先于此默认值)", Style::default().fg(GRAY))),
        Line::from(""),
        Line::from(Span::styled("  窗口大小: 854 x 480 (由版本 JSON 决定)", Style::default().fg(TEXT))),
        Line::from(Span::styled("  下载源: Mojang Piston / Modrinth", Style::default().fg(TEXT))),
        Line::from(""),
        Line::from(Span::styled("  更多设置项将在后续版本提供", Style::default().fg(GRAY_DIM))),
    ];
    f.render_widget(Paragraph::new(items).style(Style::default().bg(BG)), area);
}

fn draw_about(f: &mut Frame, app: &App, area: Rect) {
    let items = vec![
        Line::from(Span::styled(" nmcl v0.2.0", Style::default().fg(GRASS).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled(" 用 Rust + Ratatui 编写的终端 Minecraft 启动器", Style::default().fg(TEXT))),
        Line::from(Span::styled(" 离线模式 · 仅用于学习研究", Style::default().fg(GRAY))),
        Line::from(""),
        Line::from(Span::styled(format!(" MC 目录: {}", app.mc_dir.display()), Style::default().fg(GRAY))),
        Line::from(Span::styled(format!(" 日志文件: {}", app.mc_dir.join("nmcl.log").display()), Style::default().fg(GRAY))),
        Line::from(""),
        Line::from(Span::styled(" 致谢: Modrinth API · Mojang Piston · Fabric Meta · Forge", Style::default().fg(GRAY_DIM))),
    ];
    f.render_widget(Paragraph::new(items).style(Style::default().bg(BG)), area);
}

fn render_input(
    f: &mut Frame,
    app: &App,
    parent: Rect,
    row: u16,
    label: &str,
    value: &str,
    editing: bool,
    target: ClickTarget,
    layout: &mut UiLayout,
) {
    let y = parent.y + row;
    if y >= parent.y + parent.height {
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {label}"), Style::default().fg(GRAY))))
            .style(Style::default().bg(BG)),
        Rect::new(parent.x, y, 12, 1),
    );
    let vx = parent.x + 12;
    let vw = parent.width.saturating_sub(13);
    if vw == 0 {
        return;
    }
    let vrect = Rect::new(vx, y, vw, 1);
    let (text, style) = if editing {
        (format!(" {}_", app.edit_buffer), Style::default().fg(GOLD).bg(Color::Rgb(45, 45, 60)))
    } else {
        (format!(" {value}"), Style::default().fg(TEXT).bg(Color::Rgb(40, 42, 50)))
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))).style(style),
        vrect,
    );
    clickable(layout, vrect, target);
    clickable(layout, Rect::new(parent.x, y, parent.width, 1), target);
}

fn render_note(f: &mut Frame, parent: Rect, row: u16, text: &str) {
    let y = parent.y + row;
    if y >= parent.y + parent.height {
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {text}"), Style::default().fg(GRAY))))
            .style(Style::default().bg(BG)),
        Rect::new(parent.x, y, parent.width, 1),
    );
}