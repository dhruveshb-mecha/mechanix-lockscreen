use taffy::prelude::*;
use taffy::{Size, Style};
use ui::widgets::{Div, Text};
use ui::{Damage, OnChange, Point, Render, RenderCommand, Widget};
use utils::Rect as UtilsRect;
use window_manager::Color;

use super::circle::Circle;
use super::numpad::{Numpad, NumpadAction};
use crate::atlas;
use crate::events::{PinClick, PinOffer, PinOutcome, PinRelease};

const STATUS_PROMPT: Color = Color::from_rgb8(140, 140, 145);
const STATUS_ACTIVE: Color = Color::from_rgb8(242, 242, 242);
const STATUS_ERROR: Color = Color::from_rgb8(242, 106, 106);
const STATUS_LOCKOUT: Color = Color::from_rgb8(255, 149, 0);

// Upper bound matching the six indicator circles below.
const MAX_PIN_LEN: usize = 6;

/// Layout style for the primary PIN widget container.
fn pin_container_style() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        size: Size {
            width: percent(1.0_f32),
            height: auto(),
        },
        gap: Size {
            width: zero(),
            height: length(24.0_f32),
        },
        ..Style::default()
    }
}

/// Layout style for the row holding the PIN circle indicators.
fn circles_row_style() -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        align_items: Some(AlignItems::Center),
        justify_content: Some(JustifyContent::Center),
        gap: Size {
            width: length(16.0_f32),
            height: zero(),
        },
        margin: Rect {
            top: length(8.0_f32),
            bottom: length(16.0_f32),
            left: zero(),
            right: zero(),
        },
        ..Style::default()
    }
}

type CirclesTriple = (Circle, Circle, Circle);
type CirclesRow = Div<(CirclesTriple, CirclesTriple)>;

type PinHeader = (Text, CirclesRow);
type PinContainer = Div<(PinHeader, Numpad)>;

/// Widget representing the PIN entry screen, including keypad buttons and indicator circles.
#[ui::widget]
pub struct PinWidget {
    pin_len: usize,
    pin: String,
    active_button: Option<u8>,
    status: PinOutcome,
    offer: PinOffer,
    #[widget(child)]
    children: PinContainer,
}

impl Render for PinWidget {
    fn render(&self, _layout: &taffy::Layout, _abs_pos: Point) -> Vec<RenderCommand> {
        Vec::new()
    }
}

impl PinWidget {
    /// Creates a new `PinWidget` instance with initialized layout and keypad controls.
    pub fn new(pin_len: usize, offer: PinOffer) -> Self {
        let mut title = Text::new(Style::default());
        title.font = Some(&atlas::LOCKSCREEN_FONT_GEIST_MONO_12);
        title.color = STATUS_PROMPT;
        title.text = "ENTER PIN TO UNLOCK".into();

        let circles = Div::new(
            circles_row_style(),
            (
                (Circle::new(), Circle::new(), Circle::new()),
                (Circle::new(), Circle::new(), Circle::new()),
            ),
        );

        let numpad = Numpad::new();

        let container = Div::new(pin_container_style(), ((title, circles), numpad));

        let mut widget = Self {
            node_id: taffy::NodeId::new(u64::MAX),
            style: Style {
                display: Display::None,
                size: Size {
                    width: percent(1.0_f32),
                    height: auto(),
                },
                padding: Rect {
                    left: length(32.0_f32),
                    right: length(32.0_f32),
                    top: zero(),
                    bottom: zero(),
                },
                ..Style::default()
            },
            bounds: UtilsRect::ZERO,
            pending_damage: Damage::None,
            is_opaque: true,
            pin_len: pin_len.min(MAX_PIN_LEN),
            pin: String::new(),
            active_button: None,
            status: PinOutcome::Prompt,
            offer,
            children: container,
        };
        widget.apply_visible_circles();
        widget
    }

    /// Resets the PIN widget state.
    pub fn reset(&mut self) {
        self.pin.clear();
        self.active_button = None;
        self.apply_visible_circles();
        if matches!(self.status, PinOutcome::Prompt | PinOutcome::Incorrect) {
            self.status = PinOutcome::Prompt;
        }
        self.render_status();
    }

    /// Renders the status line text and color for the current [`PinOutcome`].
    fn render_status(&mut self) {
        match self.status {
            PinOutcome::Prompt => self.set_status("ENTER PIN TO UNLOCK", STATUS_PROMPT),
            PinOutcome::Incorrect => self.set_status("PIN INCORRECT", STATUS_ERROR),
            PinOutcome::Checking => self.set_status("CHECKING…", STATUS_ACTIVE),
            PinOutcome::LockedOut { secs } => {
                self.set_status(&format!("TRY AGAIN IN {secs}S"), STATUS_LOCKOUT)
            }
            PinOutcome::Unavailable => self.set_status("AUTH UNAVAILABLE", STATUS_ERROR),
        }
    }

    fn circles_mut(&mut self) -> [&mut Circle; 6] {
        let row = &mut self.children.children.0.1;
        let c = &mut row.children;
        [
            &mut c.0.0, &mut c.0.1, &mut c.0.2, &mut c.1.0, &mut c.1.1, &mut c.1.2,
        ]
    }

    /// Updates the fill colors of the PIN circle indicators according to current PIN length.
    fn apply_visible_circles(&mut self) {
        let (filled, visible) = (self.pin.len(), self.pin_len);
        for (i, circle) in self.circles_mut().into_iter().enumerate() {
            let mut style = circle.style().clone();
            style.display = if i < visible {
                Display::Flex
            } else {
                Display::None
            };
            circle.set(style);
            if i < visible {
                circle.set(i < filled);
            }
        }
    }

    fn header_text(&mut self) -> &mut Text {
        &mut self.children.children.0.0
    }

    fn numpad(&mut self) -> &mut Numpad {
        &mut self.children.children.1
    }

    fn set_status(&mut self, text: &str, color: Color) {
        let header = self.header_text();
        header.set(text.to_string());
        header.color = color;
    }

    /// Input is accepted only while visible and awaiting a submission.
    fn interactive(&self) -> bool {
        self.style.display != Display::None
            && matches!(self.status, PinOutcome::Prompt | PinOutcome::Incorrect)
    }

    fn highlight_action(&mut self, action: NumpadAction) {
        self.clear_highlight();
        // Button ids mirror the keypad layout: 1-9, 0, 10 for backspace.
        let id = match action {
            NumpadAction::Digit(c) => (c as u8).saturating_sub(b'0'),
            NumpadAction::Backspace => 10,
        };
        if let Some(btn) = self.numpad().button_mut(id) {
            btn.set(true);
        }
        self.active_button = Some(id);
    }

    fn clear_highlight(&mut self) {
        if let Some(old) = self.active_button.take()
            && let Some(btn) = self.numpad().button_mut(old)
        {
            btn.set(false);
        }
    }

    fn apply_action(&mut self, action: NumpadAction) {
        match action {
            NumpadAction::Digit(c) => {
                if !c.is_ascii_digit() {
                    return;
                }
                if self.pin.len() == self.pin_len {
                    self.pin.clear();
                }
                if self.pin.len() < self.pin_len {
                    self.pin.push(c);
                    self.apply_visible_circles();
                    if self.pin.len() == self.pin_len {
                        *self.offer.borrow_mut() = Some(self.pin.clone());
                    }
                }
            }
            NumpadAction::Backspace => {
                if !self.pin.is_empty() {
                    self.pin.pop();
                    self.apply_visible_circles();
                }
            }
        }
    }
}

impl OnChange<PinClick> for PinWidget {
    fn damage(&self, _new: &PinClick) -> Damage {
        Damage::None
    }

    /// Processes clicks on numeric buttons (0-9) and backspace.
    fn change(&mut self, click: PinClick) {
        if !self.interactive() {
            return;
        }
        if let Some(action) = self.numpad().hit_action(click.0) {
            self.highlight_action(action);
            self.apply_action(action);
        }
    }
}

impl OnChange<PinRelease> for PinWidget {
    fn damage(&self, _new: &PinRelease) -> Damage {
        Damage::None
    }

    /// Resets active button color on click release.
    fn change(&mut self, _new: PinRelease) {
        self.clear_highlight();
    }
}

impl OnChange<PinOutcome> for PinWidget {
    fn damage(&self, _new: &PinOutcome) -> Damage {
        Damage::None
    }

    fn change(&mut self, outcome: PinOutcome) {
        self.status = outcome;
        if !matches!(outcome, PinOutcome::Checking) {
            self.pin.clear();
            self.apply_visible_circles();
        }
        self.render_status();
    }
}
