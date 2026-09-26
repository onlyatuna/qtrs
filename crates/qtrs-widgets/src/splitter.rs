use crate::widget::{Widget, WidgetBase, WidgetRef};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}
pub type QSplitter = Splitter;
/// Resizable row/column of widgets (`QSplitter`).
pub struct Splitter {
    base: WidgetBase,
    orientation: Orientation,
    children: Vec<WidgetRef>,
    sizes: Vec<i32>,
    minimums: Vec<i32>,
    handle: i32,
    drag: Option<(usize, i32)>,
    pub splitter_moved: Signal<(i32, usize)>,
}
impl Splitter {
    pub fn new(orientation: Orientation) -> Self {
        Self {
            base: WidgetBase::new(),
            orientation,
            children: vec![],
            sizes: vec![],
            minimums: vec![],
            handle: 5,
            drag: None,
            splitter_moved: Signal::new(),
        }
    }
    pub fn horizontal() -> Self {
        Self::new(Orientation::Horizontal)
    }
    pub fn vertical() -> Self {
        Self::new(Orientation::Vertical)
    }
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }
    pub fn add_widget(&mut self, w: WidgetRef) {
        self.base.add_child(w.clone());
        self.children.push(w);
        self.minimums.push(0);
        let n = self.children.len();
        self.sizes = vec![100 / n as i32; n];
        self.resize_children();
    }
    pub fn count(&self) -> usize {
        self.children.len()
    }
    pub fn sizes(&self) -> Vec<i32> {
        self.sizes.clone()
    }
    pub fn set_sizes(&mut self, v: Vec<i32>) {
        if v.len() != self.children.len() {
            return;
        }
        self.sizes = v
            .into_iter()
            .zip(self.minimums.iter())
            .map(|(x, m)| x.max(*m))
            .collect();
        self.normalize();
        self.resize_children();
    }
    pub fn set_minimum_size(&mut self, index: usize, size: i32) {
        if let Some(m) = self.minimums.get_mut(index) {
            *m = size.max(0);
            if let Some(s) = self.sizes.get_mut(index) {
                *s = (*s).max(*m)
            }
            self.normalize();
            self.resize_children()
        }
    }
    pub fn minimum_sizes(&self) -> Vec<i32> {
        self.minimums.clone()
    }
    fn total(&self) -> i32 {
        match self.orientation {
            Orientation::Horizontal => self.base.geometry.width,
            Orientation::Vertical => self.base.geometry.height,
        }
        .max(0)
    }
    fn normalize(&mut self) {
        let n = self.sizes.len();
        if n == 0 {
            return;
        }
        let target = (self.total() - self.handle * (n as i32 - 1)).max(0);
        let sum: i32 = self.sizes.iter().sum();
        if sum <= 0 {
            self.sizes = vec![target / n as i32; n];
            let current: i32 = self.sizes.iter().sum();
            if let Some(x) = self.sizes.last_mut() {
                *x += target - current
            }
            return;
        }
        let vals = self.sizes.clone();
        let mut used = 0;
        for (i, s) in self.sizes.iter_mut().enumerate() {
            if i + 1 == n {
                *s = (target - used).max(self.minimums[i])
            } else {
                *s = ((vals[i] as i64 * target as i64) / sum as i64) as i32;
                *s = (*s).max(self.minimums[i]);
                used += *s
            }
        }
    }
    fn resize_children(&mut self) {
        let mut pos = 0;
        for (i, w) in self.children.iter().enumerate() {
            let s = *self.sizes.get(i).unwrap_or(&0);
            let rect = match self.orientation {
                Orientation::Horizontal => Rect::new(
                    self.base.geometry.x + pos,
                    self.base.geometry.y,
                    s,
                    self.base.geometry.height,
                ),
                Orientation::Vertical => Rect::new(
                    self.base.geometry.x,
                    self.base.geometry.y + pos,
                    self.base.geometry.width,
                    s,
                ),
            };
            w.borrow_mut().set_geometry(rect);
            pos += s + self.handle;
        }
    }
    pub fn move_splitter(&mut self, index: usize, delta: i32) {
        if index + 1 >= self.sizes.len() {
            return;
        }
        let min_a = self.minimums[index];
        let min_b = self.minimums[index + 1];
        let actual = delta
            .max(min_a - self.sizes[index])
            .min(self.sizes[index + 1] - min_b);
        self.sizes[index] += actual;
        self.sizes[index + 1] -= actual;
        self.resize_children();
        self.splitter_moved.emit(&(actual, index));
    }
    fn split_at(&self, p: Point) -> Option<(usize, i32)> {
        let coord = match self.orientation {
            Orientation::Horizontal => p.x,
            Orientation::Vertical => p.y,
        };
        let mut cur = 0;
        for i in 0..self.sizes.len().saturating_sub(1) {
            cur += self.sizes[i];
            if (coord - cur).abs() <= self.handle {
                return Some((i, coord));
            }
            cur += self.handle;
        }
        None
    }
}
impl Default for Splitter {
    fn default() -> Self {
        Self::horizontal()
    }
}
impl QObject for Splitter {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        match e.kind {
            EventKind::MouseButtonPress { x, y, button } if button == 1 => {
                self.drag = self.split_at(Point::new(x, y));
                self.drag.is_some()
            }
            EventKind::MouseMove { x, y } if self.drag.is_some() => {
                let (i, last) = self.drag.unwrap();
                let now = match self.orientation {
                    Orientation::Horizontal => x,
                    Orientation::Vertical => y,
                };
                self.move_splitter(i, now - last);
                self.drag = Some((i, now));
                true
            }
            EventKind::MouseButtonRelease { button: 1, .. } => {
                let b = self.drag.is_some();
                self.drag = None;
                b
            }
            _ => false,
        }
    }
}
impl Widget for Splitter {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.normalize();
        self.resize_children()
    }
    fn size_hint(&self) -> Size {
        let mut w = 0;
        let mut h = 0;
        for c in &self.children {
            let s = c.borrow().size_hint();
            match self.orientation {
                Orientation::Horizontal => {
                    w += s.width;
                    h = h.max(s.height)
                }
                Orientation::Vertical => {
                    h += s.height;
                    w = w.max(s.width)
                }
            }
        }
        Size::new(w, h)
    }
    fn minimum_size(&self) -> Size {
        let m: i32 = self.minimums.iter().sum();
        match self.orientation {
            Orientation::Horizontal => Size::new(
                m,
                self.children
                    .iter()
                    .map(|w| w.borrow().minimum_size().height)
                    .max()
                    .unwrap_or(0),
            ),
            Orientation::Vertical => Size::new(
                self.children
                    .iter()
                    .map(|w| w.borrow().minimum_size().width)
                    .max()
                    .unwrap_or(0),
                m,
            ),
        }
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.base.visible = v
    }
    fn is_enabled(&self) -> bool {
        self.base.enabled
    }
    fn set_enabled(&mut self, v: bool) {
        self.base.enabled = v
    }
    fn update(&mut self) {
        self.base.dirty = Some(Rect::new(
            0,
            0,
            self.base.geometry.width,
            self.base.geometry.height,
        ))
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
    fn children(&self) -> Vec<WidgetRef> {
        self.children.clone()
    }
    fn add_child(&mut self, w: WidgetRef) {
        self.add_widget(w)
    }
    fn remove_child(&mut self, id: ObjectId) {
        self.children.retain(|w| w.borrow().id() != id);
        self.base.remove_child(id);
        self.sizes.truncate(self.children.len());
        self.minimums.truncate(self.children.len());
        self.normalize();
        self.resize_children()
    }
    fn paint_event(&mut self, _: &mut Painter) {}
    fn mouse_press_event(&mut self, p: Point, b: u32, _: u32) {
        if b == 1 {
            self.drag = self.split_at(p)
        }
    }
    fn mouse_move_event(&mut self, p: Point) {
        if let Some((i, last)) = self.drag {
            let now = if self.orientation == Orientation::Horizontal {
                p.x
            } else {
                p.y
            };
            self.move_splitter(i, now - last);
            self.drag = Some((i, now))
        }
    }
    fn mouse_release_event(&mut self, _: Point, b: u32, _: u32) {
        if b == 1 {
            self.drag = None
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
