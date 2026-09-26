use qtrs_gui::paint::Pixmap;
use crate::menu::PlatformMenu;

pub trait PlatformTrayIcon: Send + Sync {
    fn set_icon(&mut self, pixmap: &Pixmap) -> Result<(), &'static str>;
    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), &'static str>;
    fn set_menu(&mut self, menu: Box<dyn PlatformMenu>);
    fn show(&mut self) -> Result<(), &'static str>;
    fn hide(&mut self) -> Result<(), &'static str>;
}

pub struct GenericTrayIcon {
    tooltip: String,
    pixmap: Option<Pixmap>,
    visible: bool,
    menu: Option<Box<dyn PlatformMenu>>,
}

impl GenericTrayIcon {
    pub fn new(tooltip: &str, pixmap: &Pixmap) -> Self {
        Self {
            tooltip: tooltip.to_string(),
            pixmap: Some(pixmap.clone()),
            visible: false,
            menu: None,
        }
    }

    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn menu(&self) -> Option<&dyn PlatformMenu> {
        self.menu.as_deref()
    }
}

impl PlatformTrayIcon for GenericTrayIcon {
    fn set_icon(&mut self, pixmap: &Pixmap) -> Result<(), &'static str> {
        self.pixmap = Some(pixmap.clone());
        Ok(())
    }

    fn set_tooltip(&mut self, tooltip: &str) -> Result<(), &'static str> {
        self.tooltip = tooltip.to_string();
        Ok(())
    }

    fn set_menu(&mut self, menu: Box<dyn PlatformMenu>) {
        self.menu = Some(menu);
    }

    fn show(&mut self) -> Result<(), &'static str> {
        self.visible = true;
        Ok(())
    }

    fn hide(&mut self) -> Result<(), &'static str> {
        self.visible = false;
        Ok(())
    }
}
