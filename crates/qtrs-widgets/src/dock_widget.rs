use crate::widget::{Widget, WidgetBase, WidgetRef};
use qtrs_core::event::Event;
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::Painter;
pub type QDockWidget = DockWidget;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DockFeatures(pub u32);
impl DockFeatures {
    pub const NONE: Self = Self(0);
    pub const CLOSABLE: Self = Self(1);
    pub const MOVABLE: Self = Self(2);
    pub const FLOATABLE: Self = Self(4);
    pub const ALL: Self = Self(7);
    pub fn contains(self, f: Self) -> bool {
        self.0 & f.0 == f.0
    }
}
pub struct DockWidget {
    base: WidgetBase,
    title: String,
    features: DockFeatures,
    floating: bool,
    content: Option<WidgetRef>,
}
impl DockWidget {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(),
            title: title.into(),
            features: DockFeatures::ALL,
            floating: false,
            content: None,
        }
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_title(&mut self, t: impl Into<String>) {
        self.title = t.into();
        self.update()
    }
    pub fn features(&self) -> DockFeatures {
        self.features
    }
    pub fn set_features(&mut self, f: DockFeatures) {
        self.features = f
    }
    pub fn is_floating(&self) -> bool {
        self.floating
    }
    pub fn set_floating(&mut self, v: bool) {
        if !self.features.contains(DockFeatures::FLOATABLE) && v {
            return;
        }
        self.floating = v;
        self.update()
    }
    pub fn set_widget(&mut self, w: WidgetRef) {
        if let Some(old) = self.content.replace(w.clone()) {
            self.base.remove_child(old.borrow().id())
        }
        self.base.add_child(w);
        self.layout_content()
    }
    pub fn widget(&self) -> Option<WidgetRef> {
        self.content.clone()
    }
    fn layout_content(&mut self) {
        if let Some(w) = &self.content {
            let g = self.base.geometry;
            w.borrow_mut()
                .set_geometry(Rect::new(g.x, g.y + 24, g.width, (g.height - 24).max(0)));
            w.borrow_mut().set_visible(self.base.visible)
        }
    }
}
impl Default for DockWidget {
    fn default() -> Self {
        Self::new("")
    }
}
impl QObject for DockWidget {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, _: &mut Event) -> bool {
        false
    }
}
impl Widget for DockWidget {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, r: Rect) {
        self.base.geometry = r;
        self.layout_content()
    }
    fn size_hint(&self) -> Size {
        Size::new(240, 180)
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.base.visible = v;
        self.layout_content()
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
        self.set_widget(w)
    }
    fn remove_child(&mut self, id: ObjectId) {
        self.base.remove_child(id);
        if self.content.as_ref().is_some_and(|w| w.borrow().id() == id) {
            self.content = None
        }
    }
    fn paint_event(&mut self, _: &mut Painter) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
