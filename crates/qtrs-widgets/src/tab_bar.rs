use crate::widget::{Widget, WidgetBase};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;

#[derive(Clone, Debug)]
struct Tab {
    text: String,
    enabled: bool,
}

/// Selectable tab strip (`QTabBar`).
pub struct TabBar {
    base: WidgetBase,
    tabs: Vec<Tab>,
    current: Option<usize>,
    movable: bool,
    closable: bool,
    pressed: Option<usize>,
    pub current_changed: Signal<usize>,
    pub tab_moved: Signal<(usize, usize)>,
    pub tab_close_requested: Signal<usize>,
}
pub type QTabBar = TabBar;
impl TabBar {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            tabs: Vec::new(),
            current: None,
            movable: false,
            closable: false,
            pressed: None,
            current_changed: Signal::new(),
            tab_moved: Signal::new(),
            tab_close_requested: Signal::new(),
        }
    }
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
    pub fn count(&self) -> usize {
        self.tabs.len()
    }
    pub fn add_tab(&mut self, text: impl Into<String>) -> usize {
        self.insert_tab(self.tabs.len(), text)
    }
    pub fn insert_tab(&mut self, index: usize, text: impl Into<String>) -> usize {
        let i = index.min(self.tabs.len());
        self.tabs.insert(
            i,
            Tab {
                text: text.into(),
                enabled: true,
            },
        );
        if self.current.is_none() {
            self.current = Some(0)
        } else if self.current.unwrap() >= i {
            self.current = Some(self.current.unwrap() + 1)
        }
        self.update();
        i
    }
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        let next = match self.current {
            None => None,
            Some(_) if self.tabs.is_empty() => None,
            Some(c) if c > index => Some(c - 1),
            Some(c) if c == index => Some(c.min(self.tabs.len() - 1)),
            v => v,
        };
        self.change_current(next);
        self.update();
    }
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.text.as_str())
    }
    pub fn set_tab_text(&mut self, index: usize, text: impl Into<String>) {
        if let Some(t) = self.tabs.get_mut(index) {
            t.text = text.into();
            self.update()
        }
    }
    pub fn is_tab_enabled(&self, index: usize) -> bool {
        self.tabs.get(index).is_some_and(|t| t.enabled)
    }
    pub fn set_tab_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(t) = self.tabs.get_mut(index) {
            t.enabled = enabled;
        }
        if !enabled && self.current == Some(index) {
            let next = self.tabs.iter().position(|x| x.enabled);
            self.change_current(next);
        }
        self.update()
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }
    pub fn set_current_index(&mut self, index: usize) {
        if self.tabs.get(index).is_some_and(|t| t.enabled) {
            self.change_current(Some(index));
            self.update()
        }
    }
    fn change_current(&mut self, value: Option<usize>) {
        if self.current != value {
            self.current = value;
            if let Some(i) = value {
                self.current_changed.emit(&i)
            }
        }
    }
    pub fn set_movable(&mut self, v: bool) {
        self.movable = v
    }
    pub fn is_movable(&self) -> bool {
        self.movable
    }
    pub fn set_tabs_closable(&mut self, v: bool) {
        self.closable = v
    }
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return;
        }
        let old = self.current;
        let t = self.tabs.remove(from);
        self.tabs.insert(to, t);
        self.current = self.current.map(|c| {
            if c == from {
                to
            } else if from < to && c > from && c <= to {
                c - 1
            } else if to < from && c >= to && c < from {
                c + 1
            } else {
                c
            }
        });
        if self.current != old {
            if let Some(i) = self.current {
                self.current_changed.emit(&i)
            }
        }
        self.tab_moved.emit(&(from, to));
        self.update()
    }
    fn hit(&self, x: i32) -> Option<usize> {
        let w = (self.base.geometry.width / self.tabs.len().max(1) as i32).max(1);
        let i = (x / w) as usize;
        (i < self.tabs.len()).then_some(i)
    }
    fn select_step(&mut self, step: i32) {
        if self.tabs.is_empty() {
            return;
        }
        let start = self.current.unwrap_or(0) as i32;
        for n in 1..=self.tabs.len() {
            let i = (start + step * n as i32).rem_euclid(self.tabs.len() as i32) as usize;
            if self.tabs[i].enabled {
                self.set_current_index(i);
                break;
            }
        }
    }
}
impl Default for TabBar {
    fn default() -> Self {
        Self::new()
    }
}
impl QObject for TabBar {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        match e.kind {
            EventKind::MouseButtonPress { x, y: _, button } if button == 1 => {
                self.pressed = self.hit(x);
                if let Some(i) = self.pressed {
                    self.set_current_index(i)
                }
                true
            }
            EventKind::MouseButtonRelease { x, y: _, button } if button == 1 => {
                let at = self.hit(x);
                if self.movable {
                    if let (Some(a), Some(b)) = (self.pressed, at) {
                        self.move_tab(a, b)
                    }
                } else if self.closable {
                    if let Some(i) = at {
                        self.tab_close_requested.emit(&i)
                    }
                }
                self.pressed = None;
                true
            }
            EventKind::KeyPress { key, .. } => {
                match key {
                    0x25 | 0x01000012 => self.select_step(-1),
                    0x27 | 0x01000014 => self.select_step(1),
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }
}
impl Widget for TabBar {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.update()
    }
    fn size_hint(&self) -> Size {
        Size::new((self.tabs.len() as i32 * 90).max(40), 28)
    }
    fn minimum_size(&self) -> Size {
        Size::new(0, 24)
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.base.visible = v;
        self.update()
    }
    fn is_enabled(&self) -> bool {
        self.base.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.base.enabled = v;
        self.update()
    }
    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ));
    }
    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }
    fn clear_dirty(&mut self) {
        self.base.dirty = None
    }
    fn layout(&self) -> Option<&dyn crate::layout::Layout> {
        None
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn crate::layout::Layout>> {
        None
    }
    fn set_layout(&mut self, _: Box<dyn crate::layout::Layout>) {}
    fn parent_widget(&self) -> Option<crate::widget::WidgetWeak> {
        self.base.parent.clone()
    }
    fn set_parent_widget(&mut self, p: Option<crate::widget::WidgetWeak>) {
        self.base.parent = p
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }
    fn set_window_id(&mut self, id: Option<ObjectId>) {
        self.base.window_id = id
    }
    fn children(&self) -> Vec<crate::widget::WidgetRef> {
        vec![]
    }
    fn add_child(&mut self, _: crate::widget::WidgetRef) {}
    fn remove_child(&mut self, _: ObjectId) {}
    fn paint_event(&mut self, _: &mut Painter) {}
    fn mouse_press_event(&mut self, p: Point, b: u32, _: u32) {
        if b == 1 {
            if let Some(i) = self.hit(p.x) {
                self.pressed = Some(i);
                self.set_current_index(i)
            }
        }
    }
    fn mouse_release_event(&mut self, p: Point, b: u32, _: u32) {
        if b == 1 {
            if let (Some(a), Some(z)) = (self.pressed, self.hit(p.x)) {
                if self.movable {
                    self.move_tab(a, z)
                } else if self.closable {
                    self.tab_close_requested.emit(&z)
                }
            }
            self.pressed = None
        }
    }
    fn key_press_event(&mut self, k: u32, _: u32, _: bool) {
        match k {
            0x25 | 0x01000012 => self.select_step(-1),
            0x27 | 0x01000014 => self.select_step(1),
            _ => (),
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
