//! Windows Vista/7 style command link button (`QCommandLinkButton`).

use qtrs_core::event::Event;
use qtrs_core::object::QObject;
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::paint::{Brush, Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics, FontWeight};
use qtrs_gui::tiny_skia::Color;

use crate::focus::FocusPolicy;
use crate::input_common;
use crate::input_keys;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommandLinkState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

/// A command link button with a prominent title, descriptive subtext,
/// and an arrow glyph matching `QCommandLinkButton`.
pub struct CommandLinkButton {
    pub base: WidgetBase,
    title: String,
    description: String,
    title_font: Font,
    desc_font: Font,
    state: CommandLinkState,
    pub clicked: Signal<()>,
    border_radius: f32,
}

pub type QCommandLinkButton = CommandLinkButton;

impl CommandLinkButton {
    /// Creates a new command link button with title text.
    pub fn new(title: impl Into<String>) -> Self {
        Self::with_description(title, "")
    }

    /// Creates a new command link button with title text and descriptive subtext.
    pub fn with_description(title: impl Into<String>, description: impl Into<String>) -> Self {
        let title_str = title.into();
        let desc_str = description.into();

        let mut title_font = Font::new("Segoe UI", 13.0);
        title_font.weight = FontWeight::Bold;
        let desc_font = Font::new("Segoe UI", 11.0);

        let mut base = WidgetBase::new();
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Preferred, Policy::Fixed);
        base.geometry = Rect::new(0, 0, 240, 56);

        Self {
            base,
            title: title_str,
            description: desc_str,
            title_font,
            desc_font,
            state: CommandLinkState::Normal,
            clicked: Signal::new(),
            border_radius: 4.0,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        self.update();
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn set_description(&mut self, desc: impl Into<String>) {
        self.description = desc.into();
        self.update();
    }

    pub fn state(&self) -> CommandLinkState {
        self.state
    }

    pub fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
    }

    fn calculate_size_hint(&self) -> Size {
        let t_metrics = FontMetrics::from_font(&self.title_font);
        let d_metrics = FontMetrics::from_font(&self.desc_font);

        let t_w = t_metrics
            .horizontal_advance(&self.title, &self.title_font)
            .ceil() as i32;
        let d_w = if self.description.is_empty() {
            0
        } else {
            d_metrics
                .horizontal_advance(&self.description, &self.desc_font)
                .ceil() as i32
        };

        let content_w = t_w.max(d_w) + 40;
        let content_h = if self.description.is_empty() {
            44
        } else {
            t_metrics.height.ceil() as i32 + d_metrics.height.ceil() as i32 + 24
        };

        Size::new(content_w.max(160), content_h.max(50))
    }
}

impl QObject for CommandLinkButton {
    leaf_qobject_common!();

    fn event(&mut self, event: &mut Event) -> bool {
        input_common::dispatch_input_event(self, event)
    }
}

impl Widget for CommandLinkButton {
    leaf_widget_common!();

    fn size_hint(&self) -> Size {
        self.calculate_size_hint()
    }

    fn update(&mut self) {
        self.base.dirty = Some(self.base.geometry);
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        self.update();
    }

    fn set_window_id(&mut self, window_id: Option<qtrs_core::object::ObjectId>) {
        self.base.window_id = window_id;
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        let r = self.geometry();
        let rf = RectF::new(0.0, 0.0, r.width as f32, r.height as f32);

        let (bg, border) = match self.state {
            CommandLinkState::Normal => (
                Color::from_rgba8(250, 250, 250, 255),
                Color::from_rgba8(220, 220, 220, 255),
            ),
            CommandLinkState::Hovered => (
                Color::from_rgba8(235, 244, 252, 255),
                Color::from_rgba8(160, 205, 240, 255),
            ),
            CommandLinkState::Pressed => (
                Color::from_rgba8(215, 234, 250, 255),
                Color::from_rgba8(120, 180, 230, 255),
            ),
        };

        painter.set_brush(Brush::Color(bg));
        painter.set_pen(Pen::new(border, 1.0));
        painter.draw_rounded_rect(rf, self.border_radius, self.border_radius);

        let icon_x = 12.0f32;
        let icon_y = 16.0f32;
        let arrow_color = match self.state {
            CommandLinkState::Pressed => Color::from_rgba8(0, 102, 204, 255),
            _ => Color::from_rgba8(0, 120, 215, 255),
        };

        painter.set_pen(Pen::new(arrow_color, 2.0));
        painter.draw_line(
            PointF::new(icon_x, icon_y),
            PointF::new(icon_x + 8.0, icon_y + 6.0),
        );
        painter.draw_line(
            PointF::new(icon_x + 8.0, icon_y + 6.0),
            PointF::new(icon_x, icon_y + 12.0),
        );

        let text_x = 30.0f32;
        let t_metrics = FontMetrics::from_font(&self.title_font);
        let title_baseline = 14.0 + t_metrics.ascent;
        let title_color = Color::from_rgba8(20, 20, 20, 255);
        painter.set_pen(Pen::new(title_color, 1.0));
        painter.draw_text(
            PointF::new(text_x, title_baseline),
            &self.title,
            &self.title_font,
        );

        if !self.description.is_empty() {
            let d_metrics = FontMetrics::from_font(&self.desc_font);
            let desc_baseline = title_baseline + t_metrics.descent + 4.0 + d_metrics.ascent;
            let desc_color = Color::from_rgba8(110, 110, 110, 255);
            painter.set_pen(Pen::new(desc_color, 1.0));
            painter.draw_text(
                PointF::new(text_x, desc_baseline),
                &self.description,
                &self.desc_font,
            );
        }

        if self.has_focus() {
            let focus_rect = rf.adjusted(2.0, 2.0, -2.0, -2.0);
            painter.set_brush(Brush::Color(Color::TRANSPARENT));
            painter.set_pen(Pen::new(Color::from_rgba8(0, 120, 215, 160), 1.0));
            painter.draw_rounded_rect(focus_rect, self.border_radius, self.border_radius);
        }
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button == 1
            && Rect::new(0, 0, self.base.geometry.width, self.base.geometry.height).contains(pos)
        {
            self.state = CommandLinkState::Pressed;
            self.base.has_focus = true;
            self.update();
        }
    }

    fn mouse_release_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        if button == 1 && self.state == CommandLinkState::Pressed {
            let inside =
                Rect::new(0, 0, self.base.geometry.width, self.base.geometry.height).contains(pos);
            self.state = if inside {
                CommandLinkState::Hovered
            } else {
                CommandLinkState::Normal
            };
            self.update();
            if inside {
                self.clicked.emit(&());
            }
        }
    }

    fn enter_event(&mut self, _pos: Point) {
        if self.state == CommandLinkState::Normal {
            self.state = CommandLinkState::Hovered;
            self.update();
        }
    }

    fn leave_event(&mut self) {
        if self.state == CommandLinkState::Hovered {
            self.state = CommandLinkState::Normal;
            self.update();
        }
    }

    fn key_press_event(&mut self, key: u32, _modifiers: u32, _is_repeat: bool) {
        if key == input_keys::KEY_SPACE || input_keys::is_enter(key) {
            self.state = CommandLinkState::Pressed;
            self.update();
        }
    }

    fn key_release_event(&mut self, key: u32, _modifiers: u32) {
        if (key == input_keys::KEY_SPACE || input_keys::is_enter(key))
            && self.state == CommandLinkState::Pressed
        {
            self.state = CommandLinkState::Normal;
            self.update();
            self.clicked.emit(&());
        }
    }
}
