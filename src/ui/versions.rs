use crate::app::{App, ClickTarget, EditField, FocusArea, VersionManagerMode, UiLayout};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use super::common::*;

pub fn draw_versions(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let cols = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(22), Constraint::Min(30)]).split(area);
    draw_side(f, app, cols[0], layout);
    draw_main(f, app, cols[1], layout);
}

fn draw_side(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let is_focus = app.focus_area == FocusArea::Left;
    let block = Block::default().borders(Borders::RIGHT).border_style(Style::default().fg(if is_focus { GOLD } else { GRAY_DIM })).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut y = inner.y;
    if app.version_manager_mode == VersionManagerMode::List {
        let items = vec![
            (" ← 返回", ClickTarget::Tab(crate::app::Tab::Home)),
            (" 📂 当前目录", ClickTarget::OpenDir),
            (" ➕ 添加目录", ClickTarget::OpenDir),
            (" ⬇ 下载新游戏", ClickTarget::Tab(crate::app::Tab::Downloads)),
        ];
        for (i, (label, target)) in items.into_iter().enumerate() {
            let r = Rect::new(inner.x, y, inner.width, 1);
            let is_sel = is_focus && app.version_left_selected == i;
            let style = if is_sel { Style::default().bg(SKY).fg(Color::Black) } else { Style::default().fg(TEXT) };
            f.render_widget(Paragraph::new(label).style(style), r);
            clickable(layout, r, target);
            // 同时绑定点击更新选中态
            clickable(layout, r, ClickTarget::VersionLeftItem(i));
            y += 1;
        }
    } else {
        let items = vec![
            (" ← 返回列表", ClickTarget::VersionManageBack),
            ("", ClickTarget::OpenDir),
            (" ⚙ 基本设置", ClickTarget::VersionManageSelect(VersionManagerMode::Basic)),
            (" 📦 版本设置", ClickTarget::VersionManageSelect(VersionManagerMode::Settings)),
            (" 🧩 模组管理", ClickTarget::VersionManageSelect(VersionManagerMode::Mods)),
        ];
        for (label, target) in items {
            if label.is_empty() { y += 1; continue; }
            let r = Rect::new(inner.x, y, inner.width, 1);
            let active = match target {
                ClickTarget::VersionManageSelect(m) => app.version_manager_mode == m,
                _ => false,
            };
            let style = if active { Style::default().bg(SKY).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT) };
            f.render_widget(Paragraph::new(label).style(style), r);
            clickable(layout, r, target);
            y += 1;
        }
    }
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    match app.version_manager_mode {
        VersionManagerMode::List => draw_version_list(f, app, area, layout),
        VersionManagerMode::Basic => draw_basic_settings(f, app, area, layout),
        VersionManagerMode::Settings => draw_version_settings(f, app, area, layout),
        VersionManagerMode::Mods => draw_mod_management(f, app, area, layout),
    }
}

fn draw_version_list(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let is_focus = app.focus_area == FocusArea::Right;
    let block = Block::default()
        .title(Line::from(Span::styled(" 已安装版本 ", Style::default().fg(GRASS).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL).border_style(Style::default().fg(if is_focus { GOLD } else { GRASS_DIM })).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.versions.is_empty() {
        f.render_widget(Paragraph::new(" 暂无版本，请点击左下角「下载新游戏」").alignment(Alignment::Center).style(Style::default().fg(GRAY)), inner);
        return;
    }
    let h = inner.height as usize;
    let offset = if let Some(s) = app.version_selected { (s + 1).saturating_sub(h) } else { 0 }.min(app.versions.len().saturating_sub(h.max(1)));

    for i in offset..(offset + h).min(app.versions.len()) {
        let v = &app.versions[i];
        let y = inner.y + (i - offset) as u16;
        let row = Rect::new(inner.x, y, inner.width, 1);
        let is_sel = Some(i) == app.version_selected;
        let (mark, _) = if v.inherits { ("⚠", GOLD) } else if v.installed { ("●", GRASS) } else { ("○", GRAY) };
        let style = if is_sel { Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT) };
        f.render_widget(Paragraph::new(Line::from(Span::styled(format!(" {} {}  [{}]", mark, v.name, v.loader), style))).style(style), row);
        clickable(layout, row, ClickTarget::LocalVersion(i));
    }
}

fn draw_basic_settings(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let ver = app.managing_version.clone().unwrap_or_default();
    let vs = app.settings.get_version(&ver);
    let is_focus = app.focus_area == FocusArea::Right;
    
    let block = Block::default()
        .title(Line::from(Span::styled(format!(" 基本设置 - {} ", ver), Style::default().fg(SKY).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL).border_style(Style::default().fg(if is_focus { GOLD } else { SKY })).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut y = inner.y + 1;
    let render_row = |f: &mut Frame, layout: &mut UiLayout, y: u16, label: &str, val: &str, field: EditField| {
        if y + 1 >= inner.y + inner.height { return; }
        f.render_widget(Paragraph::new(format!(" {}: ", label)).style(Style::default().fg(GRAY)), Rect::new(inner.x + 2, y, 12, 1));
        let vrect = Rect::new(inner.x + 14, y, inner.width.saturating_sub(16), 1);
        let editing = app.editing == field;
        let (text, style) = if editing { (format!(" {}_", app.edit_buffer), Style::default().fg(GOLD).bg(Color::Rgb(45,45,60))) } else { (format!(" {}", val), Style::default().fg(TEXT).bg(Color::Rgb(40,42,50))) };
        f.render_widget(Paragraph::new(text).style(style), vrect);
        clickable(layout, vrect, ClickTarget::EditField(field));
    };

    render_row(f, layout, y, "版本名字", &ver, EditField::VersionName); y += 2;
    let mem_val = vs.max_memory_mb.map(|m| format!("{} MB", m)).unwrap_or_else(|| "2048 MB".into());
    render_row(f, layout, y, "内存分配", &mem_val, EditField::Memory); y += 2;
    render_row(f, layout, y, "Java 路径", &vs.java_path, EditField::JavaPath); y += 2;
    render_row(f, layout, y, "Java 参数", &vs.jvm_args, EditField::JavaArgs); y += 2;
    render_row(f, layout, y, "启动参数", &vs.game_args, EditField::GameArgs); y += 2;

    if y + 1 < inner.y + inner.height {
        let btn_rect = Rect::new(inner.x + 14, y, 20, 1);
        f.render_widget(Paragraph::new(" 检测文件完整性 ").alignment(Alignment::Center).style(Style::default().bg(GOLD).fg(Color::Black)), btn_rect);
        clickable(layout, btn_rect, ClickTarget::VersionManageDetect);

        let btn2_rect = Rect::new(inner.x + 36, y, 20, 1);
        f.render_widget(Paragraph::new(" ▶ 启动游戏 ").alignment(Alignment::Center).style(Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD)), btn2_rect);
        clickable(layout, btn2_rect, ClickTarget::VersionManageLaunch);
    }
}

fn draw_version_settings(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let is_focus = app.focus_area == FocusArea::Right;
    let block = Block::default()
        .title(Line::from(Span::styled(" 版本操作 ", Style::default().fg(GOLD).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL).border_style(Style::default().fg(if is_focus { GOLD } else { GRAY_DIM })).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut y = inner.y + 2;
    let items = [
        (" 🗑 删除版本", 0, RED),
        (" 📄 复制版本", 1, SKY),
        (" ✏ 重命名版本", 2, GOLD),
    ];
    for (label, idx, color) in items {
        if y + 1 >= inner.y + inner.height { break; }
        let r = Rect::new(inner.x + 2, y, 24, 1);
        let is_sel = is_focus && app.right_action_selected == idx;
        
        let style = if is_sel { Style::default().bg(color).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().bg(GRAY_DIM).fg(TEXT) };
        f.render_widget(Paragraph::new(label).alignment(Alignment::Center).style(style), r);
        clickable(layout, r, match idx {
            0 => ClickTarget::VersionManageDelete,
            1 => ClickTarget::VersionManageCopy,
            2 => ClickTarget::VersionManageRename,
            _ => ClickTarget::VersionManageDelete,
        });
        y += 2;
    }
}

fn draw_mod_management(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let is_focus = app.focus_area == FocusArea::Right;
    let block = Block::default()
        .title(Line::from(Span::styled(" 模组管理 ", Style::default().fg(PURPLE).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL).border_style(Style::default().fg(if is_focus { GOLD } else { PURPLE })).style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let top = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(16), Constraint::Min(0)]).split(Rect::new(inner.x, inner.y, inner.width, 1));
    f.render_widget(Paragraph::new(" ⬇ 下载模组 ").alignment(Alignment::Center).style(Style::default().bg(PURPLE).fg(Color::Black)), top[0]);
    clickable(layout, top[0], ClickTarget::ModDownload);

    let list_inner = Rect::new(inner.x, inner.y + 2, inner.width, inner.height.saturating_sub(2));
    if app.mods.is_empty() {
        f.render_widget(Paragraph::new(" 暂无模组").alignment(Alignment::Center).style(Style::default().fg(GRAY)), list_inner);
        return;
    }
    let h = list_inner.height as usize;
    let offset = if let Some(s) = app.mod_selected { (s + 1).saturating_sub(h) } else { 0 };
    for i in offset..(offset + h).min(app.mods.len()) {
        let m = &app.mods[i];
        let y = list_inner.y + (i - offset) as u16;
        let row = Rect::new(list_inner.x, y, list_inner.width, 1);
        let is_sel = Some(i) == app.mod_selected;
        let status = if m.enabled { "[启用]" } else { "[禁用]" };
        let style = if is_sel { Style::default().bg(PURPLE).fg(Color::Black) } else { Style::default().fg(if m.enabled { TEXT } else { GRAY }) };
        
        let line = Line::from(Span::styled(format!(" {} {} ", status, m.filename), style));
        f.render_widget(Paragraph::new(line).style(style), row);
        clickable(layout, row, ClickTarget::ModToggle(i));

        let del_rect = Rect::new(list_inner.x + list_inner.width.saturating_sub(8), y, 8, 1);
        f.render_widget(Paragraph::new(" [删除] ").style(Style::default().fg(RED)), del_rect);
        clickable(layout, del_rect, ClickTarget::ModDelete(i));
    }
}