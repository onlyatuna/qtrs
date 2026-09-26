use crate::tab_bar::TabBar;
use crate::widget::{Widget, WidgetBase, WidgetRef};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::Painter;
pub type QTabWidget = TabWidget;
pub struct TabWidget {
    base: WidgetBase,
    bar: TabBar,
    pages: Vec<(String, WidgetRef)>,
    pub current_changed: Signal<usize>,
}
impl TabWidget {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            bar: TabBar::new(),
            pages: vec![],
            current_changed: Signal::new(),
        }
    }
    pub fn count(&self) -> usize {
        self.pages.len()
    }
    pub fn add_tab(&mut self, page: WidgetRef, title: impl Into<String>) -> usize {
        self.insert_tab(self.pages.len(), page, title)
    }
    pub fn insert_tab(&mut self, index: usize, page: WidgetRef, title: impl Into<String>) -> usize {
        let i = index.min(self.pages.len());
        let t = title.into();
        self.base.add_child(page.clone());
        self.pages.insert(i, (t.clone(), page));
        self.bar.insert_tab(i, t);
        self.sync();
        i
    }
    pub fn remove_tab(&mut self, index: usize) -> Option<WidgetRef> {
        if index >= self.pages.len() {
            return None;
        }
        let (_, page) = self.pages.remove(index);
        self.base.remove_child(page.borrow().id());
        self.bar.remove_tab(index);
        self.sync();
        Some(page)
    }
    pub fn current_index(&self) -> Option<usize> {
        self.bar.current_index()
    }
    pub fn set_current_index(&mut self, i: usize) {
        let old = self.bar.current_index();
        self.bar.set_current_index(i);
        self.sync();
        if old != self.bar.current_index() {
            if let Some(n) = self.bar.current_index() {
                self.current_changed.emit(&n)
            }
        }
    }
    pub fn current_widget(&self) -> Option<WidgetRef> {
        self.current_index()
            .and_then(|i| self.pages.get(i).map(|x| x.1.clone()))
    }
    pub fn tab_bar(&self) -> &TabBar {
        &self.bar
    }
    pub fn tab_bar_mut(&mut self) -> &mut TabBar {
        &mut self.bar
    }
    fn sync(&mut self) {
        let active = self.bar.current_index();
        let g = self.base.geometry;
        let content = Rect::new(g.x, g.y + 26, g.width, (g.height - 26).max(0));
        for (i, (_, w)) in self.pages.iter().enumerate() {
            let mut b = w.borrow_mut();
            b.set_geometry(content);
            b.set_visible(Some(i) == active);
        }
    }
}
impl Default for TabWidget {
    fn default() -> Self {
        Self::new()
    }
}
impl QObject for TabWidget {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        if let EventKind::MouseButtonPress { x, y, button: 1 } = e.kind {
            if y < 26 {
                self.bar
                    .mouse_press_event(qtrs_gui::geometry::primitives::Point::new(x, y), 1, 0);
                self.sync();
                return true;
            }
        }
        false
    }
}
impl Widget for TabWidget {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.sync();
        self.update()
    }
    fn size_hint(&self) -> Size {
        Size::new(240, 160)
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
    fn paint_event(&mut self, p: &mut Painter) {
        self.bar.paint_event(p)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
