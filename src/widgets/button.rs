use taffy::prelude::*;
use taffy::{Size, Style};
use ui::widgets::{BorderColor, Div, Text};
use ui::{Damage, OnChange, Point, Render, RenderCommand};
use utils::Rect as UtilsRect;
use window_manager::Color;

use crate::atlas;

const BORDER_COLOR: Color = Color::from_rgb8(35, 35, 37);

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

#[ui::widget]
pub struct Button {
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
        btn.border_color = BorderColor(BORDER_COLOR);
        btn.border_thickness = 1.0;
        btn.border_radius = 1.0;

        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style,
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            children: btn,
        }
    }
}

impl OnChange<bool> for Button {
    fn damage(&self, _new: &bool) -> Damage {
        Damage::None
    }

    fn change(&mut self, is_active: bool) {
        if is_active {
            self.children.set(Color::from_rgb8(35, 35, 37));
            self.children.set(BorderColor(BORDER_COLOR));
        } else {
            self.children.set(Color::TRANSPARENT);
            self.children.set(BorderColor(BORDER_COLOR));
        }
    }
}
