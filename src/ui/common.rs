use crate::app::{ClickTarget, UiLayout};
use ratatui::{
    layout::Rect,
    style::Color,
};


// 主题色
pub const BG: Color = Color::Rgb(22, 24, 30);
pub const NAV_BG: Color = Color::Rgb(32, 36, 46);
pub const GRASS: Color = Color::Rgb(124, 189, 107);

#[allow(dead_code)]
pub const GRASS_HI: Color = Color::Rgb(160, 224, 128);
pub const GRASS_DIM: Color = Color::Rgb(60, 100, 55);
pub const DIRT: Color = Color::Rgb(139, 111, 71);
pub const SKY: Color = Color::Rgb(126, 192, 238);
pub const DIAMOND: Color = Color::Rgb(77, 208, 225);
pub const GOLD: Color = Color::Rgb(255, 213, 79);
pub const RED: Color = Color::Rgb(229, 115, 115);
pub const GRAY: Color = Color::Rgb(107, 114, 128);
pub const GRAY_DIM: Color = Color::Rgb(55, 60, 72);
pub const TEXT: Color = Color::Rgb(220, 220, 230);
pub const PURPLE: Color = Color::Rgb(186, 104, 200);

pub fn clickable(l: &mut UiLayout, r: Rect, t: ClickTarget) {
    l.click_targets.push((r, t));
}

pub fn centered(area: ratatui::layout::Rect, w: u16, h: u16) -> ratatui::layout::Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(2));
    ratatui::layout::Rect::new(
        area.x + (area.width.saturating_sub(w)) / 2,
        area.y + (area.height.saturating_sub(h)) / 2,
        w,
        h,
    )
}