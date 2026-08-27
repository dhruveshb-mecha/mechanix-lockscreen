use taffy::prelude::*;
use taffy::{Size, Style};
use ui::{Damage, OnChange, Point, Render, RenderCommand, Widget};
use utils::Rect as UtilsRect;

use super::datetime::DateTimeUpdate;
use super::{BottomBar, PinWidget, TimeWidget};
use crate::events::{PinClick, PinOffer, PinOutcome, PinRelease};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState {
    TapToExpand,
    PinLock,
}

fn root_style() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: Some(JustifyContent::SpaceBetween),
        align_items: Some(AlignItems::Start),
        size: Size {
            width: percent(1.0_f32),
            height: percent(1.0_f32),
        },
        padding: Rect {
            left: length(48.0_f32),
            right: length(48.0_f32),
            top: length(48.0_f32),
            bottom: length(48.0_f32),
        },
        gap: Size {
            width: zero(),
            height: zero(),
        },
        ..Style::default()
    }
}

#[ui::widget]
pub struct Lockscreen {
    screen: ScreenState,
    open: bool,
    #[widget(child)]
    children: (TimeWidget, PinWidget, BottomBar),
}

impl Render for Lockscreen {
    fn render(&self, _layout: &taffy::Layout, _abs_pos: Point) -> Vec<RenderCommand> {
        Vec::new()
    }
}

impl Lockscreen {
    pub fn new(pin_len: usize, offer: PinOffer, open: bool) -> Self {
        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style: root_style(),
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            screen: ScreenState::TapToExpand,
            open,
            children: (
                TimeWidget::new(),
                PinWidget::new(pin_len, offer),
                BottomBar::new(if open {
                    "TAP TO UNLOCK"
                } else {
                    "TAP TO EXPAND"
                }),
            ),
        }
    }

    fn expand(&mut self) {
        if self.open || self.screen == ScreenState::PinLock {
            return;
        }
        self.screen = ScreenState::PinLock;
        let mut pin_style = self.children.1.style().clone();
        pin_style.display = Display::Flex;
        self.children.1.set(pin_style);
        self.children.2.set(self.screen);
    }

    fn collapse(&mut self) {
        if self.screen == ScreenState::TapToExpand {
            return;
        }
        self.screen = ScreenState::TapToExpand;
        let mut pin_style = self.children.1.style().clone();
        pin_style.display = Display::None;
        self.children.1.set(pin_style);
        self.children.1.reset();
        self.children.2.set(self.screen);
    }
}

impl OnChange<DateTimeUpdate> for Lockscreen {
    fn damage(&self, _new: &DateTimeUpdate) -> Damage {
        Damage::None
    }

    fn change(&mut self, new: DateTimeUpdate) {
        self.children.0.set(new);
    }
}

impl OnChange<PinClick> for Lockscreen {
    fn damage(&self, _new: &PinClick) -> Damage {
        Damage::None
    }

    fn change(&mut self, new: PinClick) {
        match self.screen {
            ScreenState::TapToExpand => self.expand(),
            ScreenState::PinLock => {
                if self.children.2.own_bounds().contains_point(new.0) {
                    self.collapse();
                    return;
                }
                self.children.1.set(new);
            }
        }
    }
}

impl OnChange<PinRelease> for Lockscreen {
    fn damage(&self, _new: &PinRelease) -> Damage {
        Damage::None
    }

    fn change(&mut self, new: PinRelease) {
        self.children.1.set(new);
    }
}

impl OnChange<PinOutcome> for Lockscreen {
    fn damage(&self, _new: &PinOutcome) -> Damage {
        Damage::None
    }

    fn change(&mut self, new: PinOutcome) {
        self.children.1.set(new);
    }
}
