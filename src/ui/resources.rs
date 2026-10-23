use crate::app::{App, ClickTarget, ScrollTarget, UiLayout, VersionSide, PAGE_SIZE};
use crate::worker::ModLoader;
use crate::worker::SortOrder;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use super::common::{clickable, BG, DIAMOND, GOLD, GRAY, GRAY_DIM, PURPLE, SKY, TEXT};

pub fn draw_resources(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(30)])
        .split(area);

    // 左栏分类
    let side_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(GRAY_DIM))
        .style(Style::default().bg(BG));
    let side_inner = side_block.inner(cols[0]);
    f.render_widget(side_block, cols[0]);

    let mut y = side_inner.y;

    // 渲染辅助闭包
    let render_side_item = |f: &mut Frame, layout: &mut UiLayout, y: &mut u16, label: &str, target: Option<ClickTarget>, active: bool, active_color: Color| {
        let rect = Rect::new(side_inner.x, *y, side_inner.width, 1);
        let style = if active {
            Style::default().bg(active_color).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(TEXT).bg(BG)
        };
        f.render_widget(Paragraph::new(Line::from(Span::styled(format!(" {} ", label), style))), rect);
        if let Some(t) = target {
            clickable(layout, rect, t);
        }
        *y += 1;
    };

    render_side_item(f, layout, &mut y, "[Minecraft]", None, false, GRAY);
    
    for side in VersionSide::mc_group() {
        if side == VersionSide::Local { continue; }
        let active = app.version_side == side;
        render_side_item(f, layout, &mut y, side.title(), Some(ClickTarget::VersionSideItem(side)), active, SKY);
    }
    
    y += 1;
    render_side_item(f, layout, &mut y, "[社区资源]", None, false, GRAY);
    
    for side in VersionSide::community_group() {
        let active = app.version_side == side;
        render_side_item(f, layout, &mut y, side.title(), Some(ClickTarget::VersionSideItem(side)), active, PURPLE);
    }

    // 右侧
    if app.version_side.is_community() {
        draw_community(f, app, cols[1], layout);
    } else {
        draw_mc_list(f, app, cols[1], layout);
    }
}

fn draw_community(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), 
            Constraint::Length(1), 
            Constraint::Min(4),
            Constraint::Length(1) // 底部翻页栏
        ])
        .split(area);

    // 搜索栏
    f.render_widget(
        Paragraph::new(" 搜索: ").style(Style::default().fg(GRAY)),
        Rect::new(rows[0].x, rows[0].y, 8, 1),
    );
    let search_rect = Rect::new(rows[0].x + 8, rows[0].y, rows[0].width.saturating_sub(10), 1);
    let (text, style) = if app.search_editing {
        (format!(" {}_", app.search_buffer), Style::default().fg(GOLD).bg(Color::Rgb(45, 45, 60)))
    } else if app.resource_query.is_empty() {
        (" 输入关键词后 Enter（当前为热门）".to_string(), Style::default().fg(GRAY).bg(Color::Rgb(40, 42, 50)))
    } else {
        (format!(" {}", app.resource_query), Style::default().fg(TEXT).bg(Color::Rgb(40, 42, 50)))
    };
    f.render_widget(Paragraph::new(Line::from(Span::styled(text, style))).style(style), search_rect);
    clickable(layout, search_rect, ClickTarget::SearchSubmit);

    // 筛选栏
    let mut x = rows[1].x;
    f.render_widget(
        Paragraph::new(" 加载器: ").style(Style::default().fg(GRAY)),
        Rect::new(x, rows[1].y, 8, 1),
    );
    x += 8;
    
    for ld in [ModLoader::Forge, ModLoader::NeoForge, ModLoader::Fabric, ModLoader::Quilt] {
        let active = app.resource_loader == Some(ld);
        let label = format!(" {} ", ld.title());
        let w = label.width() as u16;
        let r = Rect::new(x, rows[1].y, w, 1);
        let style = if active { Style::default().bg(DIAMOND).fg(Color::Black) } else { Style::default().bg(GRAY_DIM).fg(TEXT) };
        f.render_widget(Paragraph::new(Line::from(Span::styled(label, style))), r);
        clickable(layout, r, ClickTarget::LoaderFilter(ld));
        x += w + 1;
    }
    
    let sort_label = match app.resource_sort { 
        SortOrder::Downloads => " ↓下载量 ", 
        SortOrder::Follows => " ♥关注 ", 
        SortOrder::Newest => " ◷最新 ", 
        _ => " ◎相关 " 
    };
    let sw = sort_label.width() as u16;
    let s_rect = Rect::new(rows[1].x + rows[1].width.saturating_sub(sw + 1), rows[1].y, sw, 1);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(sort_label, Style::default().bg(PURPLE).fg(Color::Black)))),
        s_rect,
    );
    clickable(layout, s_rect, ClickTarget::SortToggle);

    // 结果列表
    let block = Block::default()
        .title(Line::from(Span::styled(format!(" {} ", app.version_side.title()), Style::default().fg(PURPLE).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(PURPLE));
    let inner = block.inner(rows[2]);
    f.render_widget(block, rows[2]);

    if app.resource_results.is_empty() {
        f.render_widget(
            Paragraph::new(if app.resource_searching { " 加载中… " } else { " 无结果 " }).style(Style::default().fg(GRAY)),
            inner,
        );
    } else {
        let h = inner.height as usize;
        let offset = if let Some(s) = app.resource_selected { (s + 1).saturating_sub(h) } else { 0 };
        for i in offset..(offset + h).min(app.resource_results.len()) {
            let hit = &app.resource_results[i];
            let y = inner.y + (i - offset) as u16;
            let row = Rect::new(inner.x, y, inner.width, 1);
            let is_sel = Some(i) == app.resource_selected;
            let style = if is_sel { Style::default().bg(PURPLE).fg(Color::Black) } else { Style::default().fg(TEXT) };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" {} [{}] ↓{} ", hit.title, hit.author, hit.downloads), style))).style(style),
                row,
            );
            clickable(layout, row, ClickTarget::ResourceHit(i));
        }
        layout.scroll_areas.push((inner, ScrollTarget::ResourceList));
    }

    draw_pager(f, app, rows[3], layout);
}

fn draw_mc_list(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .title(Line::from(Span::styled(format!(" {} ", app.version_side.title()), Style::default().fg(SKY).add_modifier(Modifier::BOLD))))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(SKY));
    let inner = block.inner(rows[0]);
    f.render_widget(block, rows[0]);
    
    let filtered = app.remote_list();
    if filtered.is_empty() { 
        f.render_widget(
            Paragraph::new(" 加载中或无可用版本 ").style(Style::default().fg(GRAY)),
            inner,
        );
    } else {
        let h = inner.height as usize;
        let sel = app.remote_sel[app.version_side.index()];
        let offset = if let Some(s) = sel { (s + 1).saturating_sub(h) } else { 0 };
        for i in offset..(offset + h).min(filtered.len()) {
            let v = filtered[i];
            let y = inner.y + (i - offset) as u16;
            let row = Rect::new(inner.x, y, inner.width, 1);
            let is_sel = Some(i) == sel;
            let style = if is_sel { Style::default().bg(SKY).fg(Color::Black) } else { Style::default().fg(TEXT) };
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(format!(" ◆ {} ({}) ", v.id, v.version_type), style))).style(style),
                row,
            );
            clickable(layout, row, ClickTarget::RemoteVersion(i));
        }
        layout.scroll_areas.push((inner, ScrollTarget::RemoteList));
    }
    
    draw_pager(f, app, rows[1], layout);
}

fn draw_pager(f: &mut Frame, app: &mut App, area: Rect, layout: &mut UiLayout) {
    let total_items = if app.version_side.is_community() { app.resource_total_hits } else { app.remote_list().len() };
    let total_pages = if total_items == 0 { 1 } else { (total_items + PAGE_SIZE - 1) / PAGE_SIZE };
    let cur_page = if app.version_side.is_community() { app.resource_page } else { app.remote_page[app.version_side.index()] };

    let can_prev = cur_page > 0;
    let can_next = cur_page + 1 < total_pages;

    let btn_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(4),
        ])
        .split(area);

    // 左箭头
    let prev_style = if can_prev { Style::default().bg(SKY).fg(Color::Black) } else { Style::default().bg(GRAY_DIM).fg(GRAY) };
    f.render_widget(
        Paragraph::new(" ◀ ").alignment(Alignment::Center).style(prev_style),
        btn_cols[0],
    );
    if can_prev { clickable(layout, btn_cols[0], ClickTarget::PrevPage); }

    // 中间文字
    let page_text = format!(" 第 {} / {} 页（共 {} 条，按 [ ] 或 PgUp/PgDn 翻页） ", cur_page + 1, total_pages, total_items);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(page_text, Style::default().fg(GRAY)))).alignment(Alignment::Center),
        btn_cols[1],
    );

    // 右箭头
    let next_style = if can_next { Style::default().bg(SKY).fg(Color::Black) } else { Style::default().bg(GRAY_DIM).fg(GRAY) };
    f.render_widget(
        Paragraph::new(" ▶ ").alignment(Alignment::Center).style(next_style),
        btn_cols[2],
    );
    if can_next { clickable(layout, btn_cols[2], ClickTarget::NextPage); }
}