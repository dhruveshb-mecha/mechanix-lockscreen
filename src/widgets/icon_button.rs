use taffy::prelude::*;
use taffy::{Size, Style};
use ui::widgets::{BorderColor, Div, Icon, Sprite};
use ui::{Damage, OnChange, Point, Render, RenderCommand};
use utils::Rect as UtilsRect;
use window_manager::Color;

#[ui::widget]
pub struct IconButton {
    #[widget(child)]
    pub children: Div<(Icon,)>,
}

impl Render for IconButton {
    fn render(&self, _layout: &taffy::Layout, _abs_pos: Point) -> Vec<RenderCommand> {
        Vec::new()
    }
}

impl IconButton {
    pub fn new(sprite: ::assets::SpriteRegion, atlas_id: ::assets::AtlasId, style: Style) -> Self {
        let icon_style = Style {
            size: Size {
                width: length(32.0_f32),
                height: length(32.0_f32),
            },
            ..Style::default()
        };
        let mut icon = Icon::new(icon_style);
        icon.sprite = Some(Sprite {
            atlas_id,
            region: sprite,
        });
        icon.color = Color::from_rgb8(161, 161, 165);

        let mut container = Div::new(style.clone(), (icon,));
        container.color = Color::TRANSPARENT;
        container.border_color = BorderColor(Color::TRANSPARENT);
        container.border_thickness = 0.0;
        container.border_radius = 0.0;

        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style,
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: false,
            children: container,
        }
    }
}

impl OnChange<bool> for IconButton {
    fn damage(&self, _new: &bool) -> Damage {
        Damage::None
    }

    fn change(&mut self, is_active: bool) {
        if is_active {
            self.children.set(Color::from_rgb8(35, 35, 37));
            self.children.set(BorderColor(Color::TRANSPARENT));
        } else {
            self.children.set(Color::TRANSPARENT);
            self.children.set(BorderColor(Color::TRANSPARENT));
        }
    }
}
