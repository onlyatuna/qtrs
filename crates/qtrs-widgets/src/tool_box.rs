use crate::widget::{Widget, WidgetBase, WidgetRef};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;
pub type QToolBox = ToolBox;
pub struct ToolBox {
    base: WidgetBase,
    pages: Vec<(String, WidgetRef)>,
    current: Option<usize>,
    pub current_changed: Signal<usize>,
}
impl ToolBox {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            pages: vec![],
            current: None,
            current_changed: Signal::new(),
        }
    }
    pub fn count(&self) -> usize {
        self.pages.len()
    }
    pub fn add_item(&mut self, page: WidgetRef, title: impl Into<String>) -> usize {
        self.insert_item(self.pages.len(), page, title)
    }
    pub fn insert_item(
        &mut self,
        index: usize,
        page: WidgetRef,
        title: impl Into<String>,
    ) -> usize {
        let i = index.min(self.pages.len());
        self.base.add_child(page.clone());
        self.pages.insert(i, (title.into(), page));
        if self.current.is_none() {
            self.current = Some(0)
        } else if self.current.unwrap() >= i {
            self.current = Some(self.current.unwrap() + 1)
        }
        self.sync();
        i
    }
    pub fn remove_item(&mut self, i: usize) -> Option<WidgetRef> {
        if i >= self.pages.len() {
            return None;
        }
        let (_, w) = self.pages.remove(i);
        self.base.remove_child(w.borrow().id());
        self.current = match self.current {
            Some(_) if self.pages.is_empty() => None,
            Some(c) if c > i => Some(c - 1),
            Some(c) if c == i => Some(c.min(self.pages.len() - 1)),
            x => x,
        };
        self.sync();
        Some(w)
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }
    pub fn set_current_index(&mut self, i: usize) {
        if i >= self.pages.len() || self.current == Some(i) {
            return;
        }
        self.current = Some(i);
        self.sync();
        self.current_changed.emit(&i)
    }
    pub fn current_widget(&self) -> Option<WidgetRef> {
        self.current
            .and_then(|i| self.pages.get(i).map(|p| p.1.clone()))
    }
    pub fn item_text(&self, i: usize) -> Option<&str> {
        self.pages.get(i).map(|x| x.0.as_str())
    }
    fn sync(&mut self) {
        let g = self.base.geometry;
        for (i, (_, w)) in self.pages.iter().enumerate() {
            let mut p = w.borrow_mut();
            p.set_geometry(Rect::new(g.x, g.y + 24, g.width, (g.height - 24).max(0)));
            p.set_visible(self.current == Some(i));
        }
    }
}
impl Default for ToolBox {
    fn default() -> Self {
        Self::new()
    }
}
impl QObject for ToolBox {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        if let EventKind::MouseButtonPress { x, y: _, button: 1 } = e.kind {
            let i = x / 120;
            if i >= 0 && (i as usize) < self.pages.len() {
                self.set_current_index(i as usize);
                return true;
            }
        }
        false
    }
}
impl Widget for ToolBox {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.sync()
    }
    fn size_hint(&self) -> Size {
        Size::new(180, 240)
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.base.visible = v;
        self.sync()
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
        self.base.children.clone()
    }
    fn add_child(&mut self, w: WidgetRef) {
        self.base.add_child(w)
    }
    fn remove_child(&mut self, id: ObjectId) {
        self.base.remove_child(id);
        self.pages.retain(|(_, w)| w.borrow().id() != id);
        self.sync()
    }
    fn paint_event(&mut self, _: &mut Painter) {}
    fn mouse_press_event(&mut self, p: Point, b: u32, _: u32) {
        if b == 1 {
            let i = p.x / 120;
            if i >= 0 {
                self.set_current_index(i as usize)
            }
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
