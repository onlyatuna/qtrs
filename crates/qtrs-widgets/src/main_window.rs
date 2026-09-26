use qtrs_core::event::Event;
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_gui::geometry::primitives::{Rect, Size};
use qtrs_gui::paint::Painter;

use crate::menu_bar::MenuBar;
use crate::status_bar::StatusBar;
use crate::widget::{Widget, WidgetBase, WidgetRef};

pub type QMainWindow = MainWindow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DockArea {
    Left,
    Right,
    Top,
    Bottom,
}

/// Application main window that arranges its chrome, dock widgets, and central page.
pub struct MainWindow {
    base: WidgetBase,
    central: Option<WidgetRef>,
    menu_bar: MenuBar,
    status_bar: StatusBar,
    tool_bars: Vec<WidgetRef>,
    docks: Vec<(DockArea, WidgetRef)>,
}

impl MainWindow {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            central: None,
            menu_bar: MenuBar::new(),
            status_bar: StatusBar::new(),
            tool_bars: Vec::new(),
            docks: Vec::new(),
        }
    }

    pub fn set_central_widget(&mut self, widget: WidgetRef) {
        if let Some(old) = self.central.replace(widget.clone()) {
            self.base.remove_child(old.borrow().id());
        }
        self.base.add_child(widget);
        self.layout_regions();
    }

    pub fn central_widget(&self) -> Option<WidgetRef> {
        self.central.clone()
    }

    pub fn menu_bar(&self) -> &MenuBar {
        &self.menu_bar
    }

    pub fn menu_bar_mut(&mut self) -> &mut MenuBar {
        &mut self.menu_bar
    }

    pub fn status_bar(&self) -> &StatusBar {
        &self.status_bar
    }

    pub fn status_bar_mut(&mut self) -> &mut StatusBar {
        &mut self.status_bar
    }

    pub fn add_tool_bar(&mut self, toolbar: WidgetRef) {
        self.base.add_child(toolbar.clone());
        self.tool_bars.push(toolbar);
        self.layout_regions();
    }

    pub fn add_dock_widget(&mut self, area: DockArea, dock: WidgetRef) {
        self.base.add_child(dock.clone());
        self.docks.push((area, dock));
        self.layout_regions();
    }

    pub fn dock_widgets(&self, area: DockArea) -> Vec<WidgetRef> {
        self.docks
            .iter()
            .filter(|(dock_area, _)| *dock_area == area)
            .map(|(_, widget)| widget.clone())
            .collect()
    }

    fn layout_regions(&mut self) {
        let geometry = self.base.geometry;
        let mut top = geometry.y;
        if self.menu_bar.is_visible() {
            self.menu_bar
                .set_geometry(Rect::new(geometry.x, top, geometry.width, 26));
            top += 26;
        }
        for toolbar in &self.tool_bars {
            let height = toolbar.borrow().size_hint().height.max(24);
            toolbar
                .borrow_mut()
                .set_geometry(Rect::new(geometry.x, top, geometry.width, height));
            top += height;
        }

        let status_height = if self.status_bar.is_visible() { 24 } else { 0 };
        let mut bottom = geometry.y + geometry.height - status_height;
        self.status_bar
            .set_geometry(Rect::new(geometry.x, bottom, geometry.width, status_height));
        let mut left = geometry.x;
        let mut right = geometry.x + geometry.width;

        let left_docks: Vec<_> = self
            .docks
            .iter()
            .filter(|(a, _)| *a == DockArea::Left)
            .collect();
        let right_docks: Vec<_> = self
            .docks
            .iter()
            .filter(|(a, _)| *a == DockArea::Right)
            .collect();
        for (area, docks) in [(DockArea::Left, left_docks), (DockArea::Right, right_docks)] {
            let width = 200.min((right - left).max(0) / docks.len().max(1) as i32);
            for (_, dock) in docks {
                let x = if area == DockArea::Left {
                    let x = left;
                    left += width;
                    x
                } else {
                    right -= width;
                    right
                };
                dock.borrow_mut()
                    .set_geometry(Rect::new(x, top, width, (bottom - top).max(0)));
            }
        }

        let top_docks: Vec<_> = self
            .docks
            .iter()
            .filter(|(a, _)| *a == DockArea::Top)
            .collect();
        let bottom_docks: Vec<_> = self
            .docks
            .iter()
            .filter(|(a, _)| *a == DockArea::Bottom)
            .collect();
        let content_width = (right - left).max(0);
        for (_, dock) in top_docks {
            let height = 140.min((bottom - top).max(0));
            dock.borrow_mut()
                .set_geometry(Rect::new(left, top, content_width, height));
            top += height;
        }
        for (_, dock) in bottom_docks {
            let height = 140.min((bottom - top).max(0));
            bottom -= height;
            dock.borrow_mut()
                .set_geometry(Rect::new(left, bottom, content_width, height));
        }

        if let Some(central) = &self.central {
            central.borrow_mut().set_geometry(Rect::new(
                left,
                top,
                (right - left).max(0),
                (bottom - top).max(0),
            ));
        }
    }
}

impl Default for MainWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl QObject for MainWindow {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, _event: &mut Event) -> bool {
        false
    }
}

impl Widget for MainWindow {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }
    fn geometry(&self) -> Rect {
        self.base.geometry
    }
    fn set_geometry(&mut self, geometry: Rect) {
        self.base.geometry = geometry;
        self.layout_regions();
    }
    fn size_hint(&self) -> Size {
        Size::new(640, 480)
    }
    fn is_visible(&self) -> bool {
        self.base.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.base.visible = visible;
    }
    fn is_enabled(&self) -> bool {
        self.base.enabled
    }
    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
    }
    fn update(&mut self) {
        self.layout_regions();
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
        self.base.dirty = None;
    }
    fn layout(&self) -> Option<&dyn crate::layout::Layout> {
        None
    }
    fn layout_mut(&mut self) -> Option<&mut Box<dyn crate::layout::Layout>> {
        None
    }
    fn set_layout(&mut self, _layout: Box<dyn crate::layout::Layout>) {}
    fn parent_widget(&self) -> Option<crate::widget::WidgetWeak> {
        self.base.parent.clone()
    }
    fn set_parent_widget(&mut self, parent: Option<crate::widget::WidgetWeak>) {
        self.base.parent = parent;
    }
    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }
    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
    }
    fn children(&self) -> Vec<WidgetRef> {
        self.base.children.clone()
    }
    fn add_child(&mut self, child: WidgetRef) {
        self.set_central_widget(child);
    }
    fn remove_child(&mut self, id: ObjectId) {
        self.base.remove_child(id);
        if self.central.as_ref().is_some_and(|w| w.borrow().id() == id) {
            self.central = None;
        }
        self.tool_bars.retain(|w| w.borrow().id() != id);
        self.docks.retain(|(_, w)| w.borrow().id() != id);
        self.layout_regions();
    }
    fn paint_event(&mut self, _painter: &mut Painter) {}
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
