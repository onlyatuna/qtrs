use std::path::PathBuf;

use qtrs_platform::{
    platform_dialogs, DialogIcon, FileFilter, MessageBoxButtons, MessageBoxResult,
};

/// Configuration and entry points for native open/save/folder dialogs.
#[derive(Debug, Clone)]
pub struct FileDialog {
    title: String,
    initial_path: PathBuf,
    filter: Option<FileFilter>,
}

impl FileDialog {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            initial_path: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            filter: None,
        }
    }

    pub fn set_initial_path(&mut self, path: impl Into<PathBuf>) {
        self.initial_path = path.into();
    }

    pub fn set_filter(&mut self, description: impl Into<String>, patterns: Vec<String>) {
        self.filter = Some(FileFilter {
            patterns,
            description: description.into(),
        });
    }

    pub fn clear_filter(&mut self) {
        self.filter = None;
    }

    pub fn get_open_file_name(&self) -> Option<PathBuf> {
        platform_dialogs().open_file(
            &self.title,
            &self.initial_path.to_string_lossy(),
            self.filter.as_ref(),
        )
    }

    pub fn get_open_file_names(&self) -> Option<Vec<PathBuf>> {
        platform_dialogs().open_files(
            &self.title,
            &self.initial_path.to_string_lossy(),
            self.filter.as_ref(),
        )
    }

    pub fn get_save_file_name(&self) -> Option<PathBuf> {
        platform_dialogs().save_file(
            &self.title,
            &self.initial_path.to_string_lossy(),
            self.filter.as_ref(),
        )
    }

    pub fn get_existing_directory(&self) -> Option<PathBuf> {
        platform_dialogs().select_folder(&self.title, &self.initial_path.to_string_lossy())
    }
}

/// Native message-box entry points matching the common Qt button sets.
pub struct MessageBox;

impl MessageBox {
    pub fn show(
        title: &str,
        message: &str,
        icon: DialogIcon,
        buttons: MessageBoxButtons,
    ) -> MessageBoxResult {
        platform_dialogs().message_box(title, message, icon, buttons)
    }

    pub fn information(title: &str, message: &str) {
        Self::show(title, message, DialogIcon::Info, MessageBoxButtons::Ok);
    }

    pub fn warning(title: &str, message: &str) {
        Self::show(title, message, DialogIcon::Warning, MessageBoxButtons::Ok);
    }

    pub fn critical(title: &str, message: &str) {
        Self::show(title, message, DialogIcon::Error, MessageBoxButtons::Ok);
    }

    pub fn question(title: &str, message: &str) -> bool {
        matches!(
            Self::show(
                title,
                message,
                DialogIcon::Question,
                MessageBoxButtons::YesNo,
            ),
            MessageBoxResult::Yes
        )
    }
}
