//! Export configuration

use std::path::PathBuf;

/// Configuration for the export process
#[derive(Debug, Clone)]
pub struct ExportConfig {
    /// Target USB mount point (e.g., /media/usb or /Volumes/USB)
    pub usb_path: PathBuf,

    /// Whether to copy audio files (true) or reference existing (false)
    /// Phase 1: always true (Pioneer USB exports copy files)
    pub copy_audio: bool,

    /// Specific playlist names to export (None = export all)
    pub playlist_filter: Option<Vec<String>>,

    /// Device compatibility target
    pub device_target: DeviceTarget,
}

/// Target Pioneer device model
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceTarget {
    /// XDJ-XZ (supports .DAT + .EXT analysis files)
    XdjXz,

    /// Generic Rekordbox 5-era device
    Rekordbox5,

    /// CDJ-3000 (would need .2EX for 3-band waveforms, future)
    #[allow(dead_code)]
    Cdj3000,
}

impl ExportConfig {
    /// Create a new export configuration
    pub fn new(usb_path: PathBuf) -> Self {
        Self {
            usb_path,
            copy_audio: true,
            playlist_filter: None,
            device_target: DeviceTarget::XdjXz,
        }
    }

    /// Set specific playlists to export
    pub fn with_playlists(mut self, playlists: Vec<String>) -> Self {
        self.playlist_filter = Some(playlists);
        self
    }

    /// Set device target
    pub fn with_device(mut self, device: DeviceTarget) -> Self {
        self.device_target = device;
        self
    }
}
