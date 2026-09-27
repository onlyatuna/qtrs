//! QStorageInfo volume and disk space query abstraction (`QStorageInfo` equivalent).
//!
//! Provides disk partition, volume name, file system type, total space, and free space inspection.

use std::path::{Path, PathBuf};

/// Storage volume information (`QStorageInfo` equivalent).
#[derive(Debug, Clone)]
pub struct StorageInfo {
    root_path: PathBuf,
    name: String,
    fs_type: String,
    bytes_total: u64,
    bytes_free: u64,
    bytes_available: u64,
    is_read_only: bool,
    is_ready: bool,
    is_valid: bool,
}

impl Default for StorageInfo {
    fn default() -> Self {
        Self::root()
    }
}

impl StorageInfo {
    /// Creates a new `StorageInfo` referencing the mount point containing `path`.
    pub fn new(path: impl AsRef<Path>) -> Self {
        let mut info = Self {
            root_path: path.as_ref().to_path_buf(),
            name: String::new(),
            fs_type: String::new(),
            bytes_total: 0,
            bytes_free: 0,
            bytes_available: 0,
            is_read_only: false,
            is_ready: false,
            is_valid: false,
        };
        info.refresh();
        info
    }

    /// Queries the root system storage drive.
    pub fn root() -> Self {
        #[cfg(windows)]
        {
            Self::new(r"C:\")
        }
        #[cfg(not(windows))]
        {
            Self::new("/")
        }
    }

    /// Returns the root mount point path (e.g. `C:\` or `/`).
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Returns the human-readable volume name (e.g. "System", "Data").
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the file system type name (e.g. "NTFS", "FAT32", "apfs", "ext4").
    pub fn file_system_type(&self) -> &str {
        &self.fs_type
    }

    /// Returns total storage capacity in bytes.
    pub fn bytes_total(&self) -> u64 {
        self.bytes_total
    }

    /// Returns total free storage space in bytes.
    pub fn bytes_free(&self) -> u64 {
        self.bytes_free
    }

    /// Returns available storage space to the current non-privileged user in bytes.
    pub fn bytes_available(&self) -> u64 {
        self.bytes_available
    }

    /// Returns `true` if the volume is mounted read-only.
    pub fn is_read_only(&self) -> bool {
        self.is_read_only
    }

    /// Returns `true` if the volume is ready for I/O operations.
    pub fn is_ready(&self) -> bool {
        self.is_ready
    }

    /// Returns `true` if the volume query succeeded and metadata is valid.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Refreshes the storage information from the operating system.
    pub fn refresh(&mut self) {
        #[cfg(windows)]
        {
            self.refresh_windows();
        }
        #[cfg(not(windows))]
        {
            self.refresh_unix();
        }
    }

    /// Returns a list of all currently mounted storage volumes.
    pub fn mounted_volumes() -> Vec<StorageInfo> {
        #[cfg(windows)]
        {
            Self::mounted_volumes_windows()
        }
        #[cfg(not(windows))]
        {
            vec![Self::root()]
        }
    }

    #[cfg(windows)]
    fn refresh_windows(&mut self) {
        use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumeInformationW};

        let path_str = self.root_path.to_string_lossy();
        let mut wide: Vec<u16> = path_str.encode_utf16().collect();
        if !wide.ends_with(&[0]) {
            wide.push(0);
        }

        unsafe {
            let mut free_avail: u64 = 0;
            let mut total: u64 = 0;
            let mut free_total: u64 = 0;

            let ok_space =
                GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_avail, &mut total, &mut free_total);

            if ok_space != 0 {
                self.bytes_available = free_avail;
                self.bytes_total = total;
                self.bytes_free = free_total;
                self.is_ready = true;
                self.is_valid = true;
            } else {
                self.is_ready = false;
                self.is_valid = false;
                return;
            }

            let mut volume_name = [0u16; 260];
            let mut fs_name = [0u16; 260];
            let mut flags: u32 = 0;

            let ok_vol = GetVolumeInformationW(
                wide.as_ptr(),
                volume_name.as_mut_ptr(),
                volume_name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut flags,
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            );

            if ok_vol != 0 {
                const FILE_READ_ONLY_VOLUME: u32 = 0x00080000;
                self.is_read_only = (flags & FILE_READ_ONLY_VOLUME) != 0;

                let vol_len = volume_name.iter().position(|&c| c == 0).unwrap_or(0);
                self.name = String::from_utf16_lossy(&volume_name[..vol_len]);

                let fs_len = fs_name.iter().position(|&c| c == 0).unwrap_or(0);
                self.fs_type = String::from_utf16_lossy(&fs_name[..fs_len]);
            }
        }
    }

    #[cfg(windows)]
    fn mounted_volumes_windows() -> Vec<StorageInfo> {
        use windows_sys::Win32::Storage::FileSystem::GetLogicalDriveStringsW;

        let mut buffer = [0u16; 512];
        let len = unsafe { GetLogicalDriveStringsW(buffer.len() as u32, buffer.as_mut_ptr()) };
        if len == 0 || len > buffer.len() as u32 {
            return vec![Self::root()];
        }

        let mut drives = Vec::new();
        let mut start = 0;
        for i in 0..len as usize {
            if buffer[i] == 0 {
                if start < i {
                    let drive_str = String::from_utf16_lossy(&buffer[start..i]);
                    let storage = StorageInfo::new(drive_str);
                    if storage.is_valid() {
                        drives.push(storage);
                    }
                }
                start = i + 1;
            }
        }

        if drives.is_empty() {
            vec![Self::root()]
        } else {
            drives
        }
    }

    #[cfg(target_os = "linux")]
    fn refresh_unix(&mut self) {
        use std::ffi::CString;

        let Some(path_str) = self.root_path.to_str() else {
            self.is_ready = false;
            self.is_valid = false;
            return;
        };
        let Ok(c_path) = CString::new(path_str) else {
            self.is_ready = false;
            self.is_valid = false;
            return;
        };

        let mut buf = linux_statvfs::StatVfs::default();
        // SAFETY: c_path is a valid, NUL-terminated string for the duration of the call; buf is
        // a live, correctly-sized out-parameter matching glibc's `struct statvfs` layout.
        let ret = unsafe { linux_statvfs::statvfs(c_path.as_ptr(), &mut buf) };
        if ret != 0 {
            self.is_ready = false;
            self.is_valid = false;
            return;
        }

        self.bytes_total = buf.f_frsize.saturating_mul(buf.f_blocks);
        self.bytes_free = buf.f_frsize.saturating_mul(buf.f_bfree);
        self.bytes_available = buf.f_frsize.saturating_mul(buf.f_bavail);
        self.is_read_only = (buf.f_flag & linux_statvfs::ST_RDONLY) != 0;
        self.is_ready = true;
        self.is_valid = true;
        self.fs_type =
            read_proc_mounts_fs_type(&self.root_path).unwrap_or_else(|| "unknown".to_string());
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    fn refresh_unix(&mut self) {
        // No statvfs backend implemented yet for this Unix target; report unavailable rather
        // than a fabricated "valid" volume with zero capacity.
        self.is_ready = false;
        self.is_valid = false;
    }
}

#[cfg(target_os = "linux")]
mod linux_statvfs {
    use std::os::raw::{c_char, c_int};

    /// Matches glibc's `struct statvfs` layout on 64-bit Linux (`<sys/statvfs.h>`): every
    /// `unsigned long`/`fsblkcnt_t`/`fsfilcnt_t` field is 8 bytes there.
    #[repr(C)]
    #[derive(Default)]
    pub struct StatVfs {
        pub f_bsize: u64,
        pub f_frsize: u64,
        pub f_blocks: u64,
        pub f_bfree: u64,
        pub f_bavail: u64,
        pub f_files: u64,
        pub f_ffree: u64,
        pub f_favail: u64,
        pub f_fsid: u64,
        pub f_flag: u64,
        pub f_namemax: u64,
        pub f_spare: [i32; 6],
    }

    pub const ST_RDONLY: u64 = 0x0001;

    extern "C" {
        pub fn statvfs(path: *const c_char, buf: *mut StatVfs) -> c_int;
    }
}

/// Looks up the filesystem type (e.g. "ext4", "tmpfs", "overlay") for the mount point that best
/// matches `path`, by finding the longest mount-point prefix in `/proc/mounts`. Returns `None`
/// if `/proc/mounts` is unavailable or unparsable (e.g. a sandboxed environment without procfs).
#[cfg(target_os = "linux")]
fn read_proc_mounts_fs_type(path: &Path) -> Option<String> {
    let contents = std::fs::read_to_string("/proc/mounts").ok()?;
    let mut best: Option<(usize, String)> = None;
    for line in contents.lines() {
        let mut fields = line.split_whitespace();
        let mount_point = fields.next()?;
        let fs_type = fields.next()?;
        if path.starts_with(mount_point) {
            let len = mount_point.len();
            if best.as_ref().is_none_or(|(best_len, _)| len > *best_len) {
                best = Some((len, fs_type.to_string()));
            }
        }
    }
    best.map(|(_, fs_type)| fs_type)
}
