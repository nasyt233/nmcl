use crate::app::{App, ClickTarget, ResourceModalFocus};
//use crate::worker::modrinth::ModVersionInfo;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use super::common::{centered, clickable, BG, GOLD, GRAY, GRAY_DIM, GRASS, RED, SKY, TEXT};
// 1. 资源下载弹窗（MC 版本 / 子版本 / 安装实例 三栏）
pub fn draw_resource_modal(f: &mut Frame, app: &mut App) {
    let area = centered(f.area(), 96, 20);
    f.render_widget(Clear, area);

    let title = app
        .resource_modal_hit
        .as_ref()
        .map(|h| format!(" 下载资源 · {} ", h.title))
        .unwrap_or_else(|| " 下载资源 ".into());

    let block = Block::default()
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(inner);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(24),
            Constraint::Percentage(46),
            Constraint::Percentage(30),
        ])
        .split(rows[0]);

    let filtered_len = app.resource_modal_filtered_indices().len();

    // ---- 左：MC 版本列表 ----
    let mc_title = if app.resource_modal_fetching {
        " MC 版本 · 加载中… ".to_string()
    } else {
        format!(" MC 版本 · {} 个 ", app.resource_modal_mc_versions.len())
    };
    let mc_block = Block::default()
        .title(Line::from(Span::styled(
            mc_title,
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.resource_modal_focus == ResourceModalFocus::McVersion { GOLD } else { GRAY_DIM },
        ))
        .style(Style::default().bg(BG));
    let mc_inner = mc_block.inner(cols[0]);
    f.render_widget(mc_block, cols[0]);
    render_mc_version_list(f, app, mc_inner);

    // ---- 中：子版本列表 ----
    let sub_title = {
        let mc = app
            .resource_modal_mc_idx
            .and_then(|i| app.resource_modal_mc_versions.get(i))
            .cloned()
            .unwrap_or_else(|| "—".into());
        format!(" 子版本 · MC {} · {} 个 ", mc, filtered_len)
    };
    let sub_block = Block::default()
        .title(Line::from(Span::styled(
            sub_title,
            Style::default().fg(SKY).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.resource_modal_focus == ResourceModalFocus::SubVersion { GOLD } else { GRAY_DIM },
        ))
        .style(Style::default().bg(BG));
    let sub_inner = sub_block.inner(cols[1]);
    f.render_widget(sub_block, cols[1]);
    render_sub_version_list(f, app, sub_inner);

    // ---- 右：安装实例列表 ----
    let inst_block = Block::default()
        .title(Line::from(Span::styled(
            " 安装到 ",
            Style::default().fg(GRASS).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if app.resource_modal_focus == ResourceModalFocus::Instance { GOLD } else { GRAY_DIM },
        ))
        .style(Style::default().bg(BG));
    let inst_inner = inst_block.inner(cols[2]);
    f.render_widget(inst_block, cols[2]);
    render_instance_list(f, app, inst_inner);

    // ---- 底部按钮 ----
    let btn_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Length(2),
            Constraint::Length(14),
            Constraint::Fill(1),
        ])
        .split(rows[1]);

    let confirm_rect = Rect::new(btn_cols[1].x, btn_cols[1].y + 1, btn_cols[1].width, 1);
    let cancel_rect = Rect::new(btn_cols[3].x, btn_cols[3].y + 1, btn_cols[3].width, 1);

    let can_confirm = app.resource_modal_mc_idx.is_some()
        && app.resource_modal_sub_idx.is_some()
        && !app.resource_modal_fetching
        && filtered_len > 0;
    let confirm_style = if can_confirm {
        Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(GRAY_DIM).fg(GRAY)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" 开始下载 ", confirm_style)))
            .alignment(Alignment::Center)
            .style(confirm_style),
        confirm_rect,
    );
    if can_confirm {
        clickable(&mut app.layout, confirm_rect, ClickTarget::ResourceModalConfirm);
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" 取消 ", Style::default().bg(GRAY_DIM).fg(TEXT))))
            .alignment(Alignment::Center)
            .style(Style::default().bg(GRAY_DIM)),
        cancel_rect,
    );
    clickable(&mut app.layout, cancel_rect, ClickTarget::ResourceModalCancel);

    if inner.height > 16 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " Tab 切焦点 · ↑/↓ 选择 · Enter 确认 · Esc 取消",
                Style::default().fg(GRAY),
            )))
            .style(Style::default().bg(BG)),
            Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
        );
    }
}

fn render_mc_version_list(f: &mut Frame, app: &mut App, area: Rect) {
    if app.resource_modal_fetching {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(" 正在加载… ", Style::default().fg(GRAY)))),
            area,
        );
        return;
    }
    if app.resource_modal_mc_versions.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(" 无可用版本 ", Style::default().fg(RED)))),
            area,
        );
        return;
    }

    let h = area.height as usize;
    let len = app.resource_modal_mc_versions.len();
    let sel = app.resource_modal_mc_idx;
    let offset = if let Some(s) = sel { (s + 1).saturating_sub(h) } else { 0 }.min(len.saturating_sub(h.max(1)));

    for i in offset..(offset + h).min(len) {
        let mc = app.resource_modal_mc_versions[i].clone();
        let count = app.resource_modal_versions.iter().filter(|v| v.game_versions.first().map(|s| s.as_str()) == Some(mc.as_str())).count();
        let y = area.y + (i - offset) as u16;
        let row = Rect::new(area.x, y, area.width, 1);
        let is_sel = Some(i) == sel;

        let line = if is_sel {
            Line::from(Span::styled(format!(" ▶ {} ({}个) ", mc, count), Style::default().fg(Color::Black)))
        } else {
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(mc, Style::default().fg(TEXT)),
                Span::styled(format!(" ({}个)", count), Style::default().fg(GRAY_DIM)),
            ])
        };
        let style = if is_sel { Style::default().bg(GOLD).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT).bg(BG) };
        f.render_widget(Paragraph::new(line).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::ResourceModalMcVersion(i));
    }
}

fn render_sub_version_list(f: &mut Frame, app: &mut App, area: Rect) {
    let indices = app.resource_modal_filtered_indices();
    if indices.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(" 该 MC 版本下无发布 ", Style::default().fg(GRAY)))),
            area,
        );
        return;
    }

    let h = area.height as usize;
    let len = indices.len();
    let sel = app.resource_modal_sub_idx;
    let offset = if let Some(s) = sel { (s + 1).saturating_sub(h) } else { 0 }.min(len.saturating_sub(h.max(1)));

    for i in offset..(offset + h).min(len) {
        let abs_i = indices[i];
        let (vnum, vloaders) = {
            let v = &app.resource_modal_versions[abs_i];
            (v.version_number.clone(), v.loaders.join("/"))
        };
        let y = area.y + (i - offset) as u16;
        let row = Rect::new(area.x, y, area.width, 1);
        let is_sel = Some(i) == sel;

        let line = if is_sel {
            Line::from(Span::styled(format!(" ▶ {} [{}] ", vnum, vloaders), Style::default().fg(Color::Black)))
        } else {
            Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(vnum, Style::default().fg(TEXT)),
                Span::styled(format!(" [{}]", vloaders), Style::default().fg(GRAY)),
            ])
        };
        let style = if is_sel { Style::default().bg(SKY).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT).bg(BG) };
        f.render_widget(Paragraph::new(line).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::ResourceModalSubVersion(i));
    }
}

fn render_instance_list(f: &mut Frame, app: &mut App, area: Rect) {
    let h = area.height as usize;
    let len = app.resource_modal_instances.len();
    let sel = app.resource_modal_instance_idx;
    let offset = if let Some(s) = sel { (s + 1).saturating_sub(h) } else { 0 }.min(len.saturating_sub(h.max(1)));

    for i in offset..(offset + h).min(len) {
        let raw = app.resource_modal_instances[i].clone();
        let label = if raw.is_empty() { "（全局目录）".to_string() } else { raw };
        let y = area.y + (i - offset) as u16;
        let row = Rect::new(area.x, y, area.width, 1);
        let is_sel = Some(i) == sel;
        let line = if is_sel {
            Line::from(Span::styled(format!(" ▶ {} ", label), Style::default().fg(Color::Black)))
        } else {
            Line::from(Span::styled(format!("   {}", label), Style::default().fg(TEXT)))
        };
        let style = if is_sel { Style::default().bg(GRASS).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT).bg(BG) };
        f.render_widget(Paragraph::new(line).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::ResourceModalInstance(i));
    }
}

// ============================================================
// 2. 整合包实例选择弹窗
// ============================================================

pub fn draw_modpack_modal(f: &mut Frame, app: &mut App) {
    let area = centered(f.area(), 60, 20);
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Line::from(Span::styled(
            format!(" 选择游戏版本 · {} ", app.modpack_instance.as_deref().unwrap_or("")),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(GOLD))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let list_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
    let items = &app.modpack_game_versions;
    let h = list_area.height as usize;
    let offset = if let Some(sel) = app.modpack_selected_gv { (sel + 1).saturating_sub(h) } else { 0 }.min(items.len().saturating_sub(h.max(1)));

    for i in offset..(offset + h).min(items.len()) {
        let y = list_area.y + (i - offset) as u16;
        let row = Rect::new(list_area.x, y, list_area.width, 1);
        let selected = Some(i) == app.modpack_selected_gv;
        let style = if selected { Style::default().bg(SKY).fg(Color::Black).add_modifier(Modifier::BOLD) } else { Style::default().fg(TEXT).bg(BG) };
        let line = if selected {
            Line::from(Span::styled(format!(" ▶ {} ", items[i]), Style::default().fg(Color::Black)))
        } else {
            Line::from(Span::styled(format!("   {}", items[i]), Style::default().fg(TEXT)))
        };
        f.render_widget(Paragraph::new(line).style(style), row);
        clickable(&mut app.layout, row, ClickTarget::ModpackGameVersion(i));
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(" ↑/↓ 选择 · Enter 确认安装 · Esc 取消", Style::default().fg(GRAY)))),
        Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1),
    );
}