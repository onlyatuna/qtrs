//! Popup menus with keyboard navigation and cascading sub-menus (`QMenu`).

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use qtrs_core::event::{Event, EventKind};
use qtrs_core::object::{ObjectData, ObjectId, QObject};
use qtrs_core::signal::Signal;
use qtrs_gui::geometry::primitives::{Point, PointF, Rect, RectF, Size};
use qtrs_gui::image::{Icon, IconMode, IconState};
use qtrs_gui::paint::brush::Brush;
use qtrs_gui::paint::painter::{Painter, Pen};
use qtrs_gui::text::{Font, FontMetrics};
use qtrs_gui::tiny_skia::Color;

use crate::action::{keys, Action, ActionRef, ActionWeak};
use crate::focus::FocusPolicy;
use crate::layout::Layout;
use crate::size_policy::{Policy, QSizePolicy};
use crate::widget::{Widget, WidgetBase, WidgetRef, WidgetWeak};

/// Shared handle to a [`Menu`]; sub-menus, menu bars and tool bar extensions hold menus this way.
pub type MenuRef = Rc<RefCell<Menu>>;

const FRAME: i32 = 1;
const V_PADDING: i32 = 4;
const SEPARATOR_HEIGHT: i32 = 9;
const CHECK_COLUMN: i32 = 28;
const SHORTCUT_GAP: i32 = 28;
const ARROW_COLUMN: i32 = 20;
const RIGHT_PADDING: i32 = 8;
const MIN_WIDTH: i32 = 120;
const SUBMENU_OVERLAP: i32 = 2;
const ICON_SIZE: i32 = 16;

/// Result of routing one input event through a chain of open menus.
pub(crate) enum MenuOutcome {
    /// The event was not used by the menu chain.
    Ignored,
    /// The event was consumed.
    Handled,
    /// The receiving menu closed itself (Escape or click outside).
    Closed,
    /// Left was pressed in the deepest menu with no sub-menu to close.
    NavigateLeft,
    /// Right was pressed on an item without a sub-menu.
    NavigateRight,
    /// An action was chosen. All menus up to the receiver are already hidden; the
    /// signals are the `triggered` signals of those menus, deepest first.
    Activate(ActionRef, Vec<Signal<ActionRef>>),
}

impl MenuOutcome {
    /// Triggers `action` and then emits every collected menu `triggered` signal.
    pub(crate) fn fire(action: &ActionRef, signals: &[Signal<ActionRef>]) {
        Action::trigger(action);
        for signal in signals {
            signal.emit(action);
        }
    }
}

/// Popup menu (`QMenu`).
///
/// A menu paints and routes events for its open sub-menu itself, so a whole cascade
/// behaves as one widget. Sub-menu geometry is relative to the parent menu's origin.
pub struct Menu {
    base: WidgetBase,
    title: String,
    icon: Icon,
    actions: Vec<ActionRef>,
    active: Option<usize>,
    open_submenu: Option<(usize, MenuRef)>,
    menu_action: ActionWeak,
    popup_bounds: Option<Rect>,
    last_triggered: Option<ActionRef>,
    last_covered: Rect,
    font: Font,

    background_color: Color,
    border_color: Color,
    text_color: Color,
    disabled_text_color: Color,
    highlight_color: Color,
    highlight_text_color: Color,
    separator_color: Color,

    /// Emitted with the action chosen in this menu or one of its sub-menus.
    pub triggered: Signal<ActionRef>,
    /// Emitted with the action highlighted by mouse or keyboard.
    pub hovered: Signal<ActionRef>,
    /// Emitted just before the menu is shown.
    pub about_to_show: Signal<()>,
    /// Emitted just before the menu is hidden.
    pub about_to_hide: Signal<()>,
}

pub type QMenu = Menu;

impl Menu {
    /// Creates a hidden menu with the given title (used by menu bars and parent menus).
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = WidgetBase::new();
        base.visible = false;
        base.dirty = None;
        base.focus_policy = FocusPolicy::StrongFocus;
        base.size_policy = QSizePolicy::new(Policy::Fixed, Policy::Fixed);
        base.geometry = Rect::new(0, 0, MIN_WIDTH, 2 * V_PADDING);
        Self {
            base,
            title: title.into(),
            icon: Icon::default(),
            actions: Vec::new(),
            active: None,
            open_submenu: None,
            menu_action: Weak::new(),
            popup_bounds: None,
            last_triggered: None,
            last_covered: Rect::default(),
            font: Font::new("Segoe UI", 13.0),

            background_color: Color::from_rgba8(250, 250, 250, 255),
            border_color: Color::from_rgba8(160, 160, 160, 255),
            text_color: Color::from_rgba8(20, 20, 20, 255),
            disabled_text_color: Color::from_rgba8(160, 160, 160, 255),
            highlight_color: Color::from_rgba8(0, 120, 215, 255),
            highlight_text_color: Color::from_rgba8(255, 255, 255, 255),
            separator_color: Color::from_rgba8(210, 210, 210, 255),

            triggered: Signal::new(),
            hovered: Signal::new(),
            about_to_show: Signal::new(),
            about_to_hide: Signal::new(),
        }
    }

    /// Creates a new shared menu.
    pub fn new_ref(title: impl Into<String>) -> MenuRef {
        Rc::new(RefCell::new(Self::new(title)))
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the title and keeps the menu action text in sync.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
        if let Some(action) = self.menu_action.upgrade() {
            if let Ok(mut a) = action.try_borrow_mut() {
                a.set_text(self.title.clone());
            }
        }
        self.update();
    }

    pub fn icon(&self) -> &Icon {
        &self.icon
    }

    pub fn set_icon(&mut self, icon: Icon) {
        self.icon = icon.clone();
        if let Some(action) = self.menu_action.upgrade() {
            if let Ok(mut a) = action.try_borrow_mut() {
                a.set_icon(icon);
            }
        }
    }

    pub fn font(&self) -> &Font {
        &self.font
    }

    pub fn set_font(&mut self, font: Font) {
        self.font = font;
        self.update();
    }

    /// Returns the action that represents `menu` inside a menu bar or parent menu
    /// (`QMenu::menuAction`), creating it on first use.
    ///
    /// The owner of the returned action keeps it alive; the menu holds only a weak link.
    pub fn menu_action(menu: &MenuRef) -> ActionRef {
        if let Some(action) = menu.borrow().menu_action.upgrade() {
            return action;
        }
        let (title, icon) = {
            let m = menu.borrow();
            (m.title.clone(), m.icon.clone())
        };
        let action = Action::new_ref(title);
        {
            let mut a = action.borrow_mut();
            a.set_icon(icon);
            a.set_menu(Some(menu.clone()));
        }
        menu.borrow_mut().menu_action = Rc::downgrade(&action);
        action
    }

    /// Appends an action; an action already in the menu is moved to the end.
    pub fn add_action(&mut self, action: ActionRef) -> ActionRef {
        self.remove_action(&action);
        self.actions.push(action.clone());
        self.update();
        action
    }

    /// Creates an action with `text` and appends it (`QMenu::addAction(text)`).
    pub fn add_new_action(&mut self, text: impl Into<String>) -> ActionRef {
        self.add_action(Action::new_ref(text))
    }

    /// Appends a separator action.
    pub fn add_separator(&mut self) -> ActionRef {
        self.add_action(Action::separator_ref())
    }

    /// Creates a sub-menu with `title`, appends its menu action and returns it.
    pub fn add_menu(&mut self, title: impl Into<String>) -> MenuRef {
        let submenu = Menu::new_ref(title);
        self.add_menu_ref(&submenu);
        submenu
    }

    /// Appends the menu action of an existing menu.
    pub fn add_menu_ref(&mut self, menu: &MenuRef) -> ActionRef {
        let action = Menu::menu_action(menu);
        self.add_action(action)
    }

    /// Inserts an action before `index` (clamped to the end).
    pub fn insert_action(&mut self, index: usize, action: ActionRef) -> ActionRef {
        self.remove_action(&action);
        let index = index.min(self.actions.len());
        self.actions.insert(index, action.clone());
        self.close_submenu();
        self.active = None;
        self.update();
        action
    }

    /// Removes an action from the menu.
    pub fn remove_action(&mut self, action: &ActionRef) {
        if let Some(pos) = self.actions.iter().position(|a| Rc::ptr_eq(a, action)) {
            if self.open_submenu.as_ref().is_some_and(|(i, _)| *i == pos) {
                self.close_submenu();
            }
            self.actions.remove(pos);
            if let Some((i, _)) = self.open_submenu.as_mut() {
                if *i > pos {
                    *i -= 1;
                }
            }
            self.active = match self.active {
                Some(a) if a == pos => None,
                Some(a) if a > pos => Some(a - 1),
                other => other,
            };
            self.update();
        }
    }

    /// Removes all actions.
    pub fn clear(&mut self) {
        self.close_submenu();
        self.actions.clear();
        self.active = None;
        self.update();
    }

    pub fn actions(&self) -> Vec<ActionRef> {
        self.actions.clone()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Currently highlighted action.
    pub fn active_action(&self) -> Option<ActionRef> {
        self.active.and_then(|i| self.actions.get(i).cloned())
    }

    /// Highlights an action (or clears the highlight), closing an unrelated open sub-menu.
    pub fn set_active_action(&mut self, action: Option<&ActionRef>) {
        let idx = action.and_then(|a| self.actions.iter().position(|x| Rc::ptr_eq(x, a)));
        if self.open_submenu.as_ref().is_some_and(|(i, _)| Some(*i) != idx) {
            self.close_submenu();
        }
        self.set_active_index(idx);
    }

    /// Currently open sub-menu, if any.
    pub fn open_submenu(&self) -> Option<MenuRef> {
        self.open_submenu.as_ref().map(|(_, m)| m.clone())
    }

    /// Action chosen the last time this menu (or a sub-menu) was activated.
    pub fn last_triggered_action(&self) -> Option<ActionRef> {
        self.last_triggered.clone()
    }

    /// Area, in the coordinate space of this menu's geometry, the popup must stay inside.
    pub fn popup_bounds(&self) -> Option<Rect> {
        self.popup_bounds
    }

    pub fn set_popup_bounds(&mut self, bounds: Option<Rect>) {
        self.popup_bounds = bounds;
    }

    /// Computes the geometry `popup(pos)` would use, keeping the menu inside the popup bounds.
    ///
    /// Like `QMenu::popup`, a menu overflowing the bottom edge opens upwards from `pos`
    /// when there is room, and a menu overflowing the right edge is shifted left.
    pub fn popup_geometry(&self, pos: Point) -> Rect {
        let size = self.size_hint();
        let mut x = pos.x;
        let mut y = pos.y;
        if let Some(b) = self.popup_bounds {
            if x + size.width > b.right() {
                x = b.right() - size.width;
            }
            if y + size.height > b.bottom() {
                y = if pos.y - size.height >= b.y {
                    pos.y - size.height
                } else {
                    b.bottom() - size.height
                };
            }
            x = x.max(b.x);
            y = y.max(b.y);
        }
        Rect::new(x, y, size.width, size.height)
    }

    /// Shows the menu at `pos` (`QMenu::popup`).
    pub fn popup(&mut self, pos: Point) {
        self.popup_at(pos, None);
    }

    /// Shows the menu at `pos` with `at` highlighted.
    pub fn popup_at(&mut self, pos: Point, at: Option<&ActionRef>) {
        self.about_to_show.emit(&());
        let geometry = self.popup_geometry(pos);
        self.show_with_geometry(geometry);
        if let Some(action) = at {
            let idx = self.actions.iter().position(|a| Rc::ptr_eq(a, action));
            self.set_active_index(idx);
        }
    }

    /// Runs the menu modally (`QMenu::exec`): pops it up at `pos`, feeds it `events`
    /// until it closes, and returns the triggered action.
    ///
    /// `events` may be any (possibly blocking) event source, e.g. a platform event pump.
    pub fn exec<I>(&mut self, pos: Point, events: I) -> Option<ActionRef>
    where
        I: IntoIterator<Item = Event>,
    {
        self.last_triggered = None;
        self.popup(pos);
        for mut event in events {
            self.event(&mut event);
            if !self.base.visible {
                break;
            }
        }
        if self.base.visible {
            self.hide_menu();
        }
        self.last_triggered.clone()
    }

    /// Hides the menu and every open sub-menu.
    pub fn hide_menu(&mut self) {
        if let Some((_, submenu)) = self.open_submenu.take() {
            if let Ok(mut s) = submenu.try_borrow_mut() {
                s.hide_menu();
            }
        }
        if !self.base.visible {
            return;
        }
        self.about_to_hide.emit(&());
        self.base.visible = false;
        self.active = None;
        self.update();
    }

    /// Geometry of an action's item in menu coordinates.
    pub fn action_geometry(&self, action: &ActionRef) -> Option<Rect> {
        let idx = self.actions.iter().position(|a| Rc::ptr_eq(a, action))?;
        self.item_rects().get(idx).copied()
    }

    /// Action whose item contains `pos` (menu coordinates).
    pub fn action_at(&self, pos: Point) -> Option<ActionRef> {
        self.index_at(pos).and_then(|i| self.actions.get(i).cloned())
    }

    fn item_height(&self) -> i32 {
        let metrics = FontMetrics::from_font(&self.font);
        (metrics.height.ceil() as i32 + 8).max(24)
    }

    fn item_rects(&self) -> Vec<Rect> {
        let width = self.base.geometry.width - 2 * FRAME;
        let item_h = self.item_height();
        let mut y = V_PADDING;
        self.actions
            .iter()
            .map(|action| {
                let a = action.borrow();
                if !a.is_visible() {
                    return Rect::new(FRAME, y, 0, 0);
                }
                let h = if a.is_separator() { SEPARATOR_HEIGHT } else { item_h };
                let rect = Rect::new(FRAME, y, width, h);
                y += h;
                rect
            })
            .collect()
    }

    fn index_at(&self, pos: Point) -> Option<usize> {
        self.item_rects().iter().position(|r| r.contains(pos))
    }

    fn is_selectable(&self, idx: usize) -> bool {
        self.actions.get(idx).is_some_and(|a| {
            let a = a.borrow();
            a.is_visible() && !a.is_separator() && a.is_enabled()
        })
    }

    fn has_submenu(&self, idx: usize) -> bool {
        self.actions
            .get(idx)
            .is_some_and(|a| a.borrow().menu().is_some())
    }

    fn set_active_index(&mut self, idx: Option<usize>) {
        if self.active == idx {
            return;
        }
        self.active = idx;
        if let Some(action) = idx.and_then(|i| self.actions.get(i).cloned()) {
            Action::hover(&action);
            self.hovered.emit(&action);
        }
        self.update();
    }

    fn step_active(&mut self, forward: bool) {
        let n = self.actions.len();
        if n == 0 {
            return;
        }
        let start = self.active.unwrap_or(if forward { n - 1 } else { 0 });
        for step in 1..=n {
            let i = if forward { (start + step) % n } else { (start + n - step) % n };
            if self.is_selectable(i) {
                self.set_active_index(Some(i));
                return;
            }
        }
    }

    /// Highlights the first selectable item (used when opening via keyboard).
    pub(crate) fn select_first(&mut self) {
        if let Some(i) = (0..self.actions.len()).find(|&i| self.is_selectable(i)) {
            self.set_active_index(Some(i));
        }
    }

    fn select_last(&mut self) {
        if let Some(i) = (0..self.actions.len()).rev().find(|&i| self.is_selectable(i)) {
            self.set_active_index(Some(i));
        }
    }

    /// Shows the menu at `geometry` without emitting `about_to_show` (callers do).
    pub(crate) fn show_with_geometry(&mut self, geometry: Rect) {
        self.close_submenu();
        self.active = None;
        self.base.geometry = geometry;
        self.base.visible = true;
        self.update();
    }

    fn open_submenu_at(&mut self, idx: usize, select_first: bool) {
        let Some(action) = self.actions.get(idx).cloned() else {
            return;
        };
        let (submenu, enabled) = {
            let a = action.borrow();
            (a.menu(), a.is_enabled())
        };
        let Some(submenu) = submenu else {
            return;
        };
        if let Some((open_idx, open)) = &self.open_submenu {
            if *open_idx == idx {
                if select_first {
                    open.borrow_mut().select_first();
                }
                return;
            }
        }
        self.close_submenu();
        if !enabled {
            return;
        }
        let item = self.item_rects()[idx];
        let geom = self.base.geometry;
        let local_bounds = self.popup_bounds.map(|b| b.translated(-geom.x, -geom.y));
        {
            let mut sub = submenu.borrow_mut();
            sub.about_to_show.emit(&());
            sub.popup_bounds = local_bounds;
            let size = sub.size_hint();
            let mut x = geom.width - SUBMENU_OVERLAP;
            let mut y = item.y - V_PADDING;
            if let Some(b) = local_bounds {
                if x + size.width > b.right() {
                    x = (SUBMENU_OVERLAP - size.width).max(b.x);
                }
                if y + size.height > b.bottom() {
                    y = b.bottom() - size.height;
                }
                y = y.max(b.y);
            }
            sub.show_with_geometry(Rect::new(x, y, size.width, size.height));
            if select_first {
                sub.select_first();
            }
        }
        self.open_submenu = Some((idx, submenu));
        self.update();
    }

    fn close_submenu(&mut self) {
        if let Some((_, submenu)) = self.open_submenu.take() {
            if let Ok(mut s) = submenu.try_borrow_mut() {
                s.hide_menu();
            }
            self.update();
        }
    }

    /// Chooses the item at `idx`: opens its sub-menu or hides the menu and reports activation.
    fn activate_index(&mut self, idx: usize) -> MenuOutcome {
        let Some(action) = self.actions.get(idx).cloned() else {
            return MenuOutcome::Handled;
        };
        let (enabled, has_menu, separator) = {
            let a = action.borrow();
            (a.is_enabled(), a.menu().is_some(), a.is_separator())
        };
        if !enabled || separator {
            return MenuOutcome::Handled;
        }
        if has_menu {
            self.set_active_index(Some(idx));
            self.open_submenu_at(idx, true);
            return MenuOutcome::Handled;
        }
        self.hide_menu();
        self.last_triggered = Some(action.clone());
        MenuOutcome::Activate(action, vec![self.triggered.clone()])
    }

    /// Converts the outcome reported by the open sub-menu into this menu's outcome.
    fn outcome_from_child(&mut self, outcome: MenuOutcome) -> MenuOutcome {
        match outcome {
            MenuOutcome::Ignored => MenuOutcome::Ignored,
            MenuOutcome::Handled => {
                self.update();
                MenuOutcome::Handled
            }
            MenuOutcome::Closed | MenuOutcome::NavigateLeft => {
                self.close_submenu();
                MenuOutcome::Handled
            }
            MenuOutcome::NavigateRight => MenuOutcome::NavigateRight,
            MenuOutcome::Activate(action, mut signals) => {
                self.open_submenu = None;
                self.hide_menu();
                self.last_triggered = Some(action.clone());
                signals.push(self.triggered.clone());
                MenuOutcome::Activate(action, signals)
            }
        }
    }

    /// Returns true if `pos` (menu coordinates) is inside this menu or an open sub-menu.
    pub(crate) fn covers(&self, pos: Point) -> bool {
        if !self.base.visible {
            return false;
        }
        let g = self.base.geometry;
        if Rect::new(0, 0, g.width, g.height).contains(pos) {
            return true;
        }
        self.open_submenu.as_ref().is_some_and(|(_, submenu)| {
            submenu.try_borrow().is_ok_and(|s| {
                let sg = s.base.geometry;
                s.covers(Point::new(pos.x - sg.x, pos.y - sg.y))
            })
        })
    }

    /// Bounding rectangle of this menu and its open sub-menus, in menu coordinates.
    pub(crate) fn covered_rect(&self) -> Rect {
        let g = self.base.geometry;
        let mut rect = Rect::new(0, 0, g.width, g.height);
        if let Some((_, submenu)) = &self.open_submenu {
            if let Ok(s) = submenu.try_borrow() {
                let sg = s.base.geometry;
                rect = rect.united(&s.covered_rect().translated(sg.x, sg.y));
            }
        }
        rect
    }

    fn route_to_submenu<F>(&mut self, pos: Point, f: F) -> Option<MenuOutcome>
    where
        F: FnOnce(&mut Menu, Point) -> MenuOutcome,
    {
        let (_, submenu) = self.open_submenu.clone()?;
        let origin = {
            let s = submenu.borrow();
            let sg = s.base.geometry;
            if !s.covers(Point::new(pos.x - sg.x, pos.y - sg.y)) {
                return None;
            }
            sg
        };
        let outcome = f(&mut submenu.borrow_mut(), Point::new(pos.x - origin.x, pos.y - origin.y));
        Some(self.outcome_from_child(outcome))
    }

    /// Keyboard handling (`QMenu::keyPressEvent`); the deepest open sub-menu gets the key first.
    pub(crate) fn handle_key(&mut self, key: u32, modifiers: u32) -> MenuOutcome {
        let key = keys::normalize_key(key);
        let modifiers = keys::normalize_modifiers(modifiers);
        if let Some((_, submenu)) = self.open_submenu.clone() {
            let outcome = submenu.borrow_mut().handle_key(key, modifiers);
            return self.outcome_from_child(outcome);
        }
        match key {
            keys::UP => {
                self.step_active(false);
                MenuOutcome::Handled
            }
            keys::DOWN => {
                self.step_active(true);
                MenuOutcome::Handled
            }
            keys::HOME | keys::PAGE_UP => {
                self.select_first();
                MenuOutcome::Handled
            }
            keys::END | keys::PAGE_DOWN => {
                self.select_last();
                MenuOutcome::Handled
            }
            keys::LEFT => MenuOutcome::NavigateLeft,
            keys::RIGHT => match self.active {
                Some(idx) if self.has_submenu(idx) && self.is_selectable(idx) => {
                    self.open_submenu_at(idx, true);
                    MenuOutcome::Handled
                }
                _ => MenuOutcome::NavigateRight,
            },
            keys::RETURN | keys::ENTER | keys::SPACE => match self.active {
                Some(idx) => self.activate_index(idx),
                None => MenuOutcome::Handled,
            },
            keys::ESCAPE => {
                self.hide_menu();
                MenuOutcome::Closed
            }
            _ => self.handle_mnemonic(key, modifiers),
        }
    }

    /// Mnemonic handling: a unique match is activated, clashing matches are cycled.
    fn handle_mnemonic(&mut self, key: u32, modifiers: u32) -> MenuOutcome {
        let allowed = modifiers & !(keys::MOD_ALT | keys::MOD_SHIFT) == 0;
        let Some(c) = keys::key_char(key).filter(|_| allowed) else {
            return MenuOutcome::Ignored;
        };
        let matches: Vec<usize> = (0..self.actions.len())
            .filter(|&i| {
                let a = self.actions[i].borrow();
                a.is_visible() && !a.is_separator() && a.mnemonic() == Some(c)
            })
            .collect();
        let Some(&first) = matches.first() else {
            return MenuOutcome::Ignored;
        };
        let next = match self.active {
            Some(current) if matches.len() > 1 => matches
                .iter()
                .copied()
                .find(|&i| i > current)
                .unwrap_or(first),
            _ => first,
        };
        if matches.len() == 1 {
            return self.activate_index(next);
        }
        self.set_active_index(Some(next));
        MenuOutcome::Handled
    }

    pub(crate) fn handle_mouse_move(&mut self, pos: Point) -> MenuOutcome {
        if let Some(outcome) = self.route_to_submenu(pos, |s, p| s.handle_mouse_move(p)) {
            return outcome;
        }
        if let Some(idx) = self.index_at(pos) {
            if self.is_selectable(idx) && self.active != Some(idx) {
                self.set_active_index(Some(idx));
                if self.has_submenu(idx) {
                    self.open_submenu_at(idx, false);
                } else {
                    self.close_submenu();
                }
            }
            return MenuOutcome::Handled;
        }
        if self.covers(pos) {
            MenuOutcome::Handled
        } else {
            MenuOutcome::Ignored
        }
    }

    pub(crate) fn handle_mouse_press(&mut self, pos: Point, button: u32) -> MenuOutcome {
        if let Some(outcome) = self.route_to_submenu(pos, |s, p| s.handle_mouse_press(p, button)) {
            return outcome;
        }
        if !self.covers(pos) {
            self.hide_menu();
            return MenuOutcome::Closed;
        }
        if let Some(idx) = self.index_at(pos) {
            if self.is_selectable(idx) && self.has_submenu(idx) {
                if self.open_submenu.as_ref().is_some_and(|(i, _)| *i == idx) {
                    self.close_submenu();
                } else {
                    self.set_active_index(Some(idx));
                    self.open_submenu_at(idx, false);
                }
            }
        }
        MenuOutcome::Handled
    }

    pub(crate) fn handle_mouse_release(&mut self, pos: Point, button: u32) -> MenuOutcome {
        if let Some(outcome) = self.route_to_submenu(pos, |s, p| s.handle_mouse_release(p, button)) {
            return outcome;
        }
        if let Some(idx) = self.index_at(pos) {
            if self.is_selectable(idx) && !self.has_submenu(idx) {
                return self.activate_index(idx);
            }
            return MenuOutcome::Handled;
        }
        if self.covers(pos) {
            MenuOutcome::Handled
        } else {
            MenuOutcome::Ignored
        }
    }

    pub(crate) fn handle_leave(&mut self) {
        if self.open_submenu.is_none() {
            self.set_active_index(None);
        }
    }

    fn finish_top_level(&mut self, outcome: MenuOutcome) -> bool {
        match outcome {
            MenuOutcome::Ignored => false,
            MenuOutcome::Activate(action, signals) => {
                MenuOutcome::fire(&action, &signals);
                true
            }
            MenuOutcome::Handled
            | MenuOutcome::Closed
            | MenuOutcome::NavigateLeft
            | MenuOutcome::NavigateRight => true,
        }
    }

    fn paint_item(&self, painter: &mut Painter, action: &Action, rect: Rect, active: bool) {
        let metrics = FontMetrics::from_font(&self.font);
        let width = self.base.geometry.width;
        if action.is_separator() {
            let y = rect.y as f32 + rect.height as f32 / 2.0;
            painter.set_pen(Pen::new(self.separator_color, 1.0));
            painter.draw_line(
                PointF::new(CHECK_COLUMN as f32 - 4.0, y),
                PointF::new((width - RIGHT_PADDING) as f32, y),
            );
            return;
        }
        let enabled = action.is_enabled();
        if active && enabled {
            painter.fill_rect(
                RectF::new(rect.x as f32 + 2.0, rect.y as f32, rect.width as f32 - 4.0, rect.height as f32),
                self.highlight_color,
            );
        }
        let color = if !enabled {
            self.disabled_text_color
        } else if active {
            self.highlight_text_color
        } else {
            self.text_color
        };
        let mid_y = rect.y as f32 + rect.height as f32 / 2.0;

        if action.is_checkable() && action.is_checked() {
            if action.is_exclusive_in_group() {
                painter.set_brush(Brush::Color(color));
                painter.set_pen(None);
                painter.draw_ellipse(RectF::new(10.0, mid_y - 4.0, 8.0, 8.0));
            } else {
                painter.set_pen(Pen::new(color, 2.0));
                painter.draw_line(PointF::new(9.0, mid_y), PointF::new(12.5, mid_y + 3.5));
                painter.draw_line(PointF::new(12.5, mid_y + 3.5), PointF::new(19.0, mid_y - 4.0));
            }
        } else if !action.icon().is_null() {
            let mode = if enabled { IconMode::Normal } else { IconMode::Disabled };
            let pixmap = action.icon().pixmap(Size::new(ICON_SIZE, ICON_SIZE), mode, IconState::Off);
            painter.draw_pixmap(
                RectF::new(6.0, mid_y - ICON_SIZE as f32 / 2.0, ICON_SIZE as f32, ICON_SIZE as f32),
                &pixmap,
                None,
            );
        }

        let text = action.display_text();
        let baseline = mid_y - metrics.height / 2.0 + metrics.ascent;
        painter.draw_text_colored(PointF::new(CHECK_COLUMN as f32, baseline), &text, &self.font, color);
        if let Some(offset) = keys::mnemonic_offset(action.text()) {
            let prefix_w = metrics.horizontal_advance(&text[..offset], &self.font);
            let ch: String = text[offset..].chars().take(1).collect();
            let ch_w = metrics.horizontal_advance(&ch, &self.font);
            let underline_y = baseline + 2.0;
            painter.set_pen(Pen::new(color, 1.0));
            painter.draw_line(
                PointF::new(CHECK_COLUMN as f32 + prefix_w, underline_y),
                PointF::new(CHECK_COLUMN as f32 + prefix_w + ch_w, underline_y),
            );
        }

        let shortcut = action.shortcut();
        if !shortcut.is_empty() {
            let sc_text = shortcut.to_string();
            let sc_w = metrics.horizontal_advance(&sc_text, &self.font);
            let x = (width - ARROW_COLUMN) as f32 - sc_w;
            painter.draw_text_colored(PointF::new(x, baseline), &sc_text, &self.font, color);
        }

        if action.menu().is_some() {
            let ax = (width - 14) as f32;
            painter.set_pen(Pen::new(color, 1.5));
            painter.draw_line(PointF::new(ax, mid_y - 4.0), PointF::new(ax + 4.0, mid_y));
            painter.draw_line(PointF::new(ax + 4.0, mid_y), PointF::new(ax, mid_y + 4.0));
        }
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new("")
    }
}

impl QObject for Menu {
    fn object_data(&self) -> &ObjectData {
        &self.base.object_data
    }

    fn object_data_mut(&mut self) -> &mut ObjectData {
        &mut self.base.object_data
    }

    fn event(&mut self, event: &mut Event) -> bool {
        if !self.base.visible {
            return false;
        }
        let outcome = match &event.kind {
            EventKind::KeyPress { key, modifiers, .. } => self.handle_key(*key, *modifiers),
            EventKind::MouseMove { x, y } => self.handle_mouse_move(Point::new(*x, *y)),
            EventKind::MouseButtonPress { x, y, button } => {
                self.handle_mouse_press(Point::new(*x, *y), *button)
            }
            EventKind::MouseButtonRelease { x, y, button } => {
                self.handle_mouse_release(Point::new(*x, *y), *button)
            }
            EventKind::Leave => {
                self.handle_leave();
                MenuOutcome::Handled
            }
            _ => return false,
        };
        self.finish_top_level(outcome)
    }

    fn as_qobject_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn as_qobject_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl Widget for Menu {
    fn id(&self) -> ObjectId {
        self.base.object_data.id
    }

    fn geometry(&self) -> Rect {
        self.base.geometry
    }

    fn set_geometry(&mut self, rect: Rect) {
        if self.base.geometry != rect {
            self.base.geometry = rect;
            self.update();
        }
    }

    fn size_hint(&self) -> Size {
        let metrics = FontMetrics::from_font(&self.font);
        let item_h = self.item_height();
        let mut height = 2 * V_PADDING;
        let mut max_text = 0i32;
        let mut max_shortcut = 0i32;
        for action in &self.actions {
            let a = action.borrow();
            if !a.is_visible() {
                continue;
            }
            if a.is_separator() {
                height += SEPARATOR_HEIGHT;
                continue;
            }
            height += item_h;
            let text_w = metrics.horizontal_advance(&a.display_text(), &self.font).ceil() as i32;
            max_text = max_text.max(text_w);
            if !a.shortcut().is_empty() {
                let sc_w = metrics
                    .horizontal_advance(&a.shortcut().to_string(), &self.font)
                    .ceil() as i32;
                max_shortcut = max_shortcut.max(sc_w);
            }
        }
        let shortcut_w = if max_shortcut > 0 { SHORTCUT_GAP + max_shortcut } else { 0 };
        let width = (CHECK_COLUMN + max_text + shortcut_w + ARROW_COLUMN + RIGHT_PADDING).max(MIN_WIDTH);
        Size::new(width, height)
    }

    fn minimum_size_hint(&self) -> Size {
        self.size_hint()
    }

    fn size_policy(&self) -> QSizePolicy {
        self.base.size_policy
    }

    fn set_size_policy(&mut self, policy: QSizePolicy) {
        self.base.size_policy = policy;
    }

    fn is_visible(&self) -> bool {
        self.base.visible
    }

    /// Showing emits `about_to_show` and sizes the menu to its hint; hiding closes the cascade.
    fn set_visible(&mut self, visible: bool) {
        if visible == self.base.visible {
            return;
        }
        if visible {
            self.about_to_show.emit(&());
            let g = self.base.geometry;
            let size = self.size_hint();
            self.show_with_geometry(Rect::new(g.x, g.y, size.width, size.height));
        } else {
            self.hide_menu();
        }
    }

    fn is_enabled(&self) -> bool {
        self.base.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.base.enabled = enabled;
        self.update();
    }

    fn update(&mut self) {
        let covered = self.covered_rect();
        self.base.dirty = Some(covered.united(&self.last_covered));
        self.last_covered = covered;
        let target = self.base.window_id.unwrap_or(self.base.object_data.id);
        let _ = qtrs_core::event_loop::post_event_to_thread(
            qtrs_core::object::ThreadId::current(),
            target,
            Event::new(EventKind::UpdateRequest),
        );
    }

    fn dirty_rect(&self) -> Option<Rect> {
        self.base.dirty
    }

    fn clear_dirty(&mut self) {
        self.base.dirty = None;
    }

    fn layout(&self) -> Option<&dyn Layout> {
        None
    }

    fn layout_mut(&mut self) -> Option<&mut Box<dyn Layout>> {
        None
    }

    fn set_layout(&mut self, _layout: Box<dyn Layout>) {}

    fn parent_widget(&self) -> Option<WidgetWeak> {
        self.base.parent.clone()
    }

    fn set_parent_widget(&mut self, parent: Option<WidgetWeak>) {
        self.base.parent = parent;
    }

    fn window_id(&self) -> Option<ObjectId> {
        self.base.window_id
    }

    fn set_window_id(&mut self, window_id: Option<ObjectId>) {
        self.base.window_id = window_id;
    }

    fn children(&self) -> Vec<WidgetRef> {
        Vec::new()
    }

    fn add_child(&mut self, _child: WidgetRef) {}

    fn remove_child(&mut self, _child_id: ObjectId) {}

    fn focus_policy(&self) -> FocusPolicy {
        self.base.focus_policy
    }

    fn set_focus_policy(&mut self, policy: FocusPolicy) {
        self.base.focus_policy = policy;
    }

    fn has_focus(&self) -> bool {
        self.base.has_focus
    }

    fn set_has_focus(&mut self, focus: bool) {
        self.base.has_focus = focus;
    }

    fn mouse_press_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        let outcome = self.handle_mouse_press(pos, button);
        self.finish_top_level(outcome);
    }

    fn mouse_release_event(&mut self, pos: Point, button: u32, _modifiers: u32) {
        let outcome = self.handle_mouse_release(pos, button);
        self.finish_top_level(outcome);
    }

    fn mouse_move_event(&mut self, pos: Point) {
        let outcome = self.handle_mouse_move(pos);
        self.finish_top_level(outcome);
    }

    fn leave_event(&mut self) {
        self.handle_leave();
    }

    fn key_press_event(&mut self, key: u32, modifiers: u32, _is_repeat: bool) {
        if self.base.visible {
            let outcome = self.handle_key(key, modifiers);
            self.finish_top_level(outcome);
        }
    }

    fn paint_event(&mut self, painter: &mut Painter) {
        if !self.base.visible {
            return;
        }
        let g = self.base.geometry;
        painter.set_brush(Brush::Color(self.background_color));
        painter.set_pen(Pen::new(self.border_color, 1.0));
        painter.draw_rect(RectF::new(0.5, 0.5, g.width as f32 - 1.0, g.height as f32 - 1.0));

        let rects = self.item_rects();
        for (i, action) in self.actions.iter().enumerate() {
            let rect = rects[i];
            if rect.width <= 0 || rect.height <= 0 {
                continue;
            }
            let a = action.borrow();
            self.paint_item(painter, &a, rect, self.active == Some(i));
        }

        if let Some((_, submenu)) = &self.open_submenu {
            if let Ok(mut sub) = submenu.try_borrow_mut() {
                let sg = sub.base.geometry;
                painter.save();
                painter.translate(sg.x as f32, sg.y as f32);
                sub.paint_event(painter);
                painter.restore();
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
