use crate::action::keys;
use crate::menu::{Menu, MenuRef};
use crate::widget::{Widget, WidgetBase};
use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_gui::geometry::primitives::{Point, Rect, Size};
use qtrs_gui::paint::Painter;
pub type QMenuBar = MenuBar;
pub struct MenuBar {
    base: WidgetBase,
    menus: Vec<MenuRef>,
    active: Option<usize>,
}
impl MenuBar {
    pub fn new() -> Self {
        Self {
            base: WidgetBase::new(),
            menus: vec![],
            active: None,
        }
    }
    pub fn add_menu(&mut self, title: impl Into<String>) -> MenuRef {
        let m = Menu::new_ref(title);
        self.add_menu_ref(&m);
        m
    }
    pub fn add_menu_ref(&mut self, m: &MenuRef) {
        if !self.menus.iter().any(|x| std::rc::Rc::ptr_eq(x, m)) {
            self.menus.push(m.clone());
            self.update()
        }
    }
    pub fn menus(&self) -> Vec<MenuRef> {
        self.menus.clone()
    }
    pub fn active_menu(&self) -> Option<MenuRef> {
        self.active.and_then(|i| self.menus.get(i).cloned())
    }
    pub fn close_active(&mut self) {
        if let Some(m) = self.active_menu() {
            m.borrow_mut().set_visible(false)
        }
        self.active = None;
        self.update()
    }
    pub fn activate_menu(&mut self, index: usize) {
        if index >= self.menus.len() {
            return;
        }
        if self.active == Some(index) {
            self.close_active();
            return;
        }
        if let Some(old) = self.active_menu() {
            old.borrow_mut().set_visible(false)
        }
        self.active = Some(index);
        let w = (self.base.geometry.width / self.menus.len().max(1) as i32).max(1);
        self.menus[index].borrow_mut().show_with_geometry(Rect::new(
            self.base.geometry.x + (index as i32) * w,
            self.base.geometry.y + self.base.geometry.height,
            w,
            0,
        ));
        self.update()
    }
    fn hit(&self, x: i32) -> Option<usize> {
        if self.menus.is_empty() {
            return None;
        }
        let w = (self.base.geometry.width / self.menus.len() as i32).max(1);
        let i = (x / w) as usize;
        (i < self.menus.len()).then_some(i)
    }
    fn mnemonic(&self, c: char) -> Option<usize> {
        self.menus
            .iter()
            .position(|m| keys::mnemonic(m.borrow().title()) == Some(c))
    }
}
impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}
impl QObject for MenuBar {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }
    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }
    fn event(&mut self, e: &mut Event) -> bool {
        match e.kind {
            EventKind::MouseButtonPress { x, y: _, button: 1 } => {
                if let Some(i) = self.hit(x) {
                    self.activate_menu(i);
                    true
                } else {
                    false
                }
            }
            EventKind::KeyPress { key, modifiers, .. } => {
                let n = keys::normalize_key(key);
                if keys::normalize_modifiers(modifiers) & keys::MOD_ALT != 0 {
                    if let Some(c) = keys::key_char(n) {
                        if let Some(i) = self.mnemonic(c as char) {
                            self.activate_menu(i);
                            return true;
                        }
                    }
                }
                if let Some(i) = self.active {
                    match n {
                        keys::LEFT => {
                            self.activate_menu((i + self.menus.len() - 1) % self.menus.len())
                        }
                        keys::RIGHT => self.activate_menu((i + 1) % self.menus.len()),
                        keys::ESCAPE => self.close_active(),
                        _ => return false,
                    }
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}
impl Widget for MenuBar {
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
        Size::new(self.menus.len() as i32 * 80, 26)
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
    fn mouse_press_event(&mut self, p: Point, b: u32, _: u32) {
        if b == 1 {
            if let Some(i) = self.hit(p.x) {
                self.activate_menu(i)
            }
        }
    }
    fn key_press_event(&mut self, k: u32, m: u32, _: bool) {
        let n = keys::normalize_key(k);
        if keys::normalize_modifiers(m) & keys::MOD_ALT != 0 {
            if let Some(c) = keys::key_char(n) {
                if let Some(i) = self.mnemonic(c as char) {
                    self.activate_menu(i)
                }
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
