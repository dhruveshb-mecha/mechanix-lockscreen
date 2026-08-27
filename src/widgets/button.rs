use taffy::prelude::*;
use taffy::{Size, Style};
use ui::widgets::{BorderColor, Div, Text};
use ui::{Damage, OnChange, Point, Render, RenderCommand};
use utils::Rect as UtilsRect;
use window_manager::Color;

use crate::atlas;

pub fn filled_button_style() -> Style {
    Style {
        display: Display::Flex,
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size {
            width: length(162.0_f32),
            height: length(72.0_f32),
        },
        border: Rect {
            left: length(1.0_f32),
            right: length(1.0_f32),
            top: length(1.0_f32),
            bottom: length(1.0_f32),
        },
        ..Style::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonType {
    #[default]
    Filled,
    Transparent,
}

#[ui::widget]
pub struct Button {
    pub kind: ButtonType,
    #[widget(child)]
    pub children: Div<(Text,)>,
}

impl Render for Button {
    fn render(&self, _layout: &taffy::Layout, _abs_pos: Point) -> Vec<RenderCommand> {
        Vec::new()
    }
}

impl Button {
    pub fn new(num: &str) -> Self {
        Self::filled_with_style(num, filled_button_style())
    }

    pub fn filled_with_style(num: &str, style: Style) -> Self {
        let mut txt = Text::new(Style::default());
        txt.font = Some(&atlas::LOCKSCREEN_FONT_GEIST_MONO_36);
        txt.color = Color::from_rgb8(161, 161, 165);
        txt.text = num.into();

        let mut btn = Div::new(style.clone(), (txt,));
        btn.color = Color::TRANSPARENT;
        btn.border_color = BorderColor(Color::from_rgb8(35, 35, 37));
        btn.border_thickness = 1.0;
        btn.border_radius = 0.0;

        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style,
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            kind: ButtonType::Filled,
            children: btn,
        }
    }

    pub fn transparent_with_style(num: &str, style: Style) -> Self {
        Self::transparent_with_font_and_style(num, &atlas::LOCKSCREEN_FONT_GEIST_MONO_12, style)
    }

    pub fn transparent_with_font_and_style(
        label: &str,
        font: &'static assets::BakedFont,
        style: Style,
    ) -> Self {
        let mut txt = Text::new(Style::default());
        txt.font = Some(font);
        txt.color = Color::from_rgb8(161, 161, 165);
        txt.text = label.into();

        let mut btn = Div::new(style.clone(), (txt,));
        btn.color = Color::TRANSPARENT;
        btn.border_color = BorderColor(Color::TRANSPARENT);
        btn.border_thickness = 0.0;
        btn.border_radius = 0.0;

        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style,
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            kind: ButtonType::Transparent,
            children: btn,
        }
    }
}

impl OnChange<bool> for Button {
    fn damage(&self, _new: &bool) -> Damage {
        Damage::None
    }

    fn change(&mut self, is_active: bool) {
        match (self.kind, is_active) {
            (ButtonType::Transparent, _) => {}
            (ButtonType::Filled, true) => {
                self.children.set(Color::from_rgb8(35, 35, 37));
                self.children
                    .set(BorderColor(Color::from_rgb8(35, 35, 37)));
            }
            (ButtonType::Filled, false) => {
                self.children.set(Color::TRANSPARENT);
                self.children
                    .set(BorderColor(Color::from_rgb8(35, 35, 37)));
            }
        }
    }
}
