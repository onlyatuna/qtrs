use crate::action::{keys, ActionRef};
use crate::widget::{Widget, WidgetBase};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;
pub type QToolBar = ToolBar;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolBarOrientation {
    Horizontal,
    Vertical,
}
pub struct ToolBar {
    base: WidgetBase,
    title: String,
    actions: Vec<ActionRef>,
    orientation: ToolBarOrientation,
}
impl ToolBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            base: WidgetBase::new(),
            title: title.into(),
            actions: vec![],
            orientation: ToolBarOrientation::Horizontal,
        }
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn set_title(&mut self, v: impl Into<String>) {
        self.title = v.into();
        self.update()
    }
    pub fn add_action(&mut self, a: ActionRef) {
        self.actions.push(a);
        self.update()
    }
    pub fn actions(&self) -> Vec<ActionRef> {
        self.actions.clone()
    }
    pub fn orientation(&self) -> ToolBarOrientation {
        self.orientation
    }
    pub fn set_orientation(&mut self, o: ToolBarOrientation) {
        self.orientation = o;
        self.update()
    }
    pub fn trigger_action(&mut self, index: usize) {
        if let Some(a) = self.actions.get(index) {
            crate::action::Action::trigger(a)
        }
    }
}
impl QObject for ToolBar {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        match e.kind {
            EventKind::MouseButtonRelease { x, y, button: 1 } => {
                self.mouse_release_event(Point::new(x, y), 1, 0);
                true
            }
            EventKind::KeyPress { key, .. } if key == keys::LEFT || key == keys::RIGHT => true,
            _ => false,
        }
    }
}
impl Widget for ToolBar {
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
        match self.orientation {
            ToolBarOrientation::Horizontal => Size::new(self.actions.len() as i32 * 80, 32),
            ToolBarOrientation::Vertical => Size::new(40, self.actions.len() as i32 * 32),
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
    fn children(&self) -> Vec<crate::widget::WidgetRef> {
        vec![]
    }
    fn add_child(&mut self, _: crate::widget::WidgetRef) {}
    fn remove_child(&mut self, _: ObjectId) {}
    fn paint_event(&mut self, _: &mut Painter) {}
    fn mouse_release_event(&mut self, p: Point, b: u32, _: u32) {
        if b != 1 {
            return;
        }
        let idx = match self.orientation {
            ToolBarOrientation::Horizontal => p.x / 80,
            ToolBarOrientation::Vertical => p.y / 32,
        };
        if idx >= 0 {
            self.trigger_action(idx as usize)
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
