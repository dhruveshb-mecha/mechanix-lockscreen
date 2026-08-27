use super::{Button, IconButton};
use crate::atlas;
use taffy::prelude::*;
use taffy::{Size, Style};
use ui::widgets::Div;
use ui::{Damage, Point, Render, RenderCommand, Widget};

/// Layout style for the keypad grid layout.
fn keypad_grid_style() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Center),
        gap: Size {
            width: zero(),
            height: length(8.0_f32),
        },
        ..Style::default()
    }
}

/// Layout style for each row of keypad buttons.
fn keypad_row_style() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        gap: Size {
            width: length(8.0_f32),
            height: zero(),
        },
        ..Style::default()
    }
}

/// Layout style for the ENTER P/W button.
fn enter_pw_button_style() -> Style {
    Style {
        display: Display::Flex,
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size {
            width: length(162.0_f32),
            height: length(72.0_f32),
        },
        ..Style::default()
    }
}

/// Layout style for the backspace button.
fn backspace_button_style() -> Style {
    Style {
        display: Display::Flex,
        justify_content: Some(JustifyContent::Center),
        align_items: Some(AlignItems::Center),
        size: Size {
            width: length(162.0_f32),
            height: length(72.0_f32),
        },
        ..Style::default()
    }
}

type KeypadRow3 = Div<(Button, Button, Button)>;
type KeypadRow4 = Div<(Button, Button, IconButton)>;

type GridPair1 = (KeypadRow3, KeypadRow3);
type GridPair2 = (KeypadRow3, KeypadRow4);
type KeypadGrid = Div<(GridPair1, GridPair2)>;

/// Action triggered by interacting with the numpad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumpadAction {
    Digit(char),
    Backspace,
}

/// Widget encapsulating numeric buttons (0-9) and backspace in a grid layout.
#[ui::widget]
pub struct Numpad {
    #[widget(child)]
    pub children: KeypadGrid,
}

impl Render for Numpad {
    fn render(&self, _layout: &taffy::Layout, _abs_pos: Point) -> Vec<RenderCommand> {
        Vec::new()
    }
}

impl Numpad {
    pub fn new() -> Self {
        let row1 = Div::new(
            keypad_row_style(),
            (Button::new("1"), Button::new("2"), Button::new("3")),
        );
        let row2 = Div::new(
            keypad_row_style(),
            (Button::new("4"), Button::new("5"), Button::new("6")),
        );
        let row3 = Div::new(
            keypad_row_style(),
            (Button::new("7"), Button::new("8"), Button::new("9")),
        );

        let row4 = Div::new(
            keypad_row_style(),
            (
                Button::transparent_with_style("ENTER P/W", enter_pw_button_style()),
                Button::new("0"),
                IconButton::new(
                    atlas::LOCKSCREEN_BACKSPACE,
                    atlas::LOCKSCREEN.id,
                    backspace_button_style(),
                ),
            ),
        );

        let grid = Div::new(keypad_grid_style(), ((row1, row2), (row3, row4)));

        Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style: keypad_grid_style(),
            bounds: utils::Rect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            children: grid,
        }
    }

    fn buttons(&self) -> [(&Button, NumpadAction); 10] {
        let (row1, row2) = &self.children.children.0;
        let (row3, row4) = &self.children.children.1;
        [
            (&row1.children.0, NumpadAction::Digit('1')),
            (&row1.children.1, NumpadAction::Digit('2')),
            (&row1.children.2, NumpadAction::Digit('3')),
            (&row2.children.0, NumpadAction::Digit('4')),
            (&row2.children.1, NumpadAction::Digit('5')),
            (&row2.children.2, NumpadAction::Digit('6')),
            (&row3.children.0, NumpadAction::Digit('7')),
            (&row3.children.1, NumpadAction::Digit('8')),
            (&row3.children.2, NumpadAction::Digit('9')),
            (&row4.children.1, NumpadAction::Digit('0')),
        ]
    }

    fn icon_buttons(&self) -> [(&IconButton, NumpadAction); 1] {
        let row4 = &self.children.children.1.1;
        [(&row4.children.2, NumpadAction::Backspace)]
    }

    fn buttons_mut(&mut self) -> [(u8, &mut Button); 10] {
        let (row1, row2) = &mut self.children.children.0;
        let (row3, row4) = &mut self.children.children.1;
        [
            (1, &mut row1.children.0),
            (2, &mut row1.children.1),
            (3, &mut row1.children.2),
            (4, &mut row2.children.0),
            (5, &mut row2.children.1),
            (6, &mut row2.children.2),
            (7, &mut row3.children.0),
            (8, &mut row3.children.1),
            (9, &mut row3.children.2),
            (0, &mut row4.children.1),
        ]
    }

    fn icon_buttons_mut(&mut self) -> [(u8, &mut IconButton); 1] {
        let row4 = &mut self.children.children.1.1;
        [(10, &mut row4.children.2)]
    }

    /// Returns the action (Digit or Backspace) at point `p`.
    pub fn hit_action(&self, p: Point) -> Option<NumpadAction> {
        if let Some((_, action)) = self
            .buttons()
            .into_iter()
            .find(|(btn, _)| btn.own_bounds().contains_point(p))
        {
            return Some(action);
        }
        if let Some((_, action)) = self
            .icon_buttons()
            .into_iter()
            .find(|(btn, _)| btn.own_bounds().contains_point(p))
        {
            return Some(action);
        }
        None
    }

    /// Returns a mutable reference to the `Button` associated with the specified button ID.
    pub fn button_mut(&mut self, id: u8) -> Option<&mut Button> {
        self.buttons_mut()
            .into_iter()
            .find(|(btn_id, _)| *btn_id == id)
            .map(|(_, btn)| btn)
    }

    /// Returns a mutable reference to the `IconButton` associated with the specified button ID.
    pub fn icon_button_mut(&mut self, id: u8) -> Option<&mut IconButton> {
        self.icon_buttons_mut()
            .into_iter()
            .find(|(btn_id, _)| *btn_id == id)
            .map(|(_, btn)| btn)
    }
}

impl Default for Numpad {
    fn default() -> Self {
        Self::new()
    }
}
