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
    let is_focus = app.focus_area == crate::app::FocusArea::Left;
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(if is_focus { GOLD } else { GRAY_DIM }))
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
        SettingsSide::Launcher => draw_launcher(f, app, inner, layout),
        SettingsSide::About => draw_about(f, app, inner),
    }
}

fn draw_general(f: &mut Frame, app: &App, area: Rect, layout: &mut UiLayout) {
    render_input(
        f, app, area, 1, "玩家名",
        &app.profiles[app.current_profile].name,
        app.editing == EditField::PlayerName,
        ClickTarget::EditField(EditField::PlayerName),
        layout,
    );
    render_note(f, area, 3, " 离线模式下使用此用户名进入游戏");
    render_note(f, area, 4, " 正版登录功能将在后续版本提供");
}

fn draw_java(f: &mut Frame, app: &App, area: Rect, layout: &mut UiLayout) {
    render_input(
        f, app, area, 1, "Java 路径",
        &app.settings.java_path,
        app.editing == EditField::JavaPath,
        ClickTarget::EditField(EditField::JavaPath),
        layout,
    );
    render_note(f, area, 3, " 留空则使用系统 PATH 中的 java");
    
    render_note(f, area, 5, " 自动扫描到的 Java:");
    let mut y = 6;
    for (i, j) in app.java_installs.iter().take(6).enumerate() {
        if y >= area.y + area.height { break; }
        let rect = Rect::new(area.x + 2, area.y + y, area.width.saturating_sub(4), 1);
        let active = app.settings.java_path == *j;
        let style = if active {
            Style::default().bg(GOLD).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT)
        };
        f.render_widget(Paragraph::new(format!(" [{}] {} ", i + 1, j)).style(style), rect);
        clickable(layout, rect, ClickTarget::SetJavaPath(i));
        y += 1;
    }
}

fn draw_launcher(f: &mut Frame, app: &App, area: Rect, layout: &mut UiLayout) {
    // 默认内存
    render_input(
        f, app, area, 1, "默认最大内存",
        &format!("{} MB", app.settings.default_max_memory_mb),
        app.editing == EditField::DefaultMemory,
        ClickTarget::EditField(EditField::DefaultMemory),
        layout,
    );
    
    render_note(f, area, 3, " 版本管理中设置的特定内存优先于此默认值");
    render_note(f, area, 4, " 建议值: 2048 / 4096 / 8192");
    
    render_note(f, area, 6, " 窗口大小: 854 x 480");
    render_note(f, area, 7, " 下载源: Mojang Piston / Modrinth");
}

fn draw_about(f: &mut Frame, app: &App, area: Rect) {
    let items = vec![
        Line::from(Span::styled(format!(" nmcl v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(GRASS).add_modifier(Modifier::BOLD))),
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

// 优化后的输入框组件
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
    
    // 左侧标签
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(format!(" {} ", label), Style::default().fg(GRAY))))
            .style(Style::default().bg(BG)),
        Rect::new(parent.x, y, 14, 1),
    );
    
    // 右侧输入框
    let vx = parent.x + 14;
    let vw = parent.width.saturating_sub(16).min(50); // 限制最大宽度，更加美观
    if vw == 0 {
        return;
    }
    let vrect = Rect::new(vx, y, vw, 1);
    let (text, style) = if editing {
        (format!(" {}_", app.edit_buffer), Style::default().fg(GOLD).bg(Color::Rgb(45, 45, 60)))
    } else {
        (format!(" {} ", value), Style::default().fg(TEXT).bg(Color::Rgb(40, 42, 50)))
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style))).style(style),
        vrect,
    );
    
    clickable(layout, Rect::new(parent.x, y, parent.width, 1), target);
}

fn render_note(f: &mut Frame, parent: Rect, row: u16, text: &str) {
    let y = parent.y + row;
    if y >= parent.y + parent.height {
        return;
    }
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, Style::default().fg(GRAY_DIM))))
            .style(Style::default().bg(BG)),
        Rect::new(parent.x, y, parent.width, 1),
    );
}