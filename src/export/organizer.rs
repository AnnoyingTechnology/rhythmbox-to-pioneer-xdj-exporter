//! USB file organization and structure

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Manages Pioneer USB directory structure
pub struct UsbOrganizer {
    /// Root USB path
    usb_root: PathBuf,

    /// PIONEER/rekordbox directory
    rekordbox_dir: PathBuf,

    /// PIONEER/USBANLZ directory
    usbanlz_dir: PathBuf,

    /// Music files directory (we'll put music in a Music/ folder)
    music_dir: PathBuf,
}

impl UsbOrganizer {
    /// Create a new USB organizer for the given USB mount point
    pub fn new(usb_root: PathBuf) -> Result<Self> {
        let rekordbox_dir = usb_root.join("PIONEER").join("rekordbox");
        let usbanlz_dir = usb_root.join("PIONEER").join("USBANLZ");
        let music_dir = usb_root.join("Music");

        Ok(Self {
            usb_root,
            rekordbox_dir,
            usbanlz_dir,
            music_dir,
        })
    }

    /// Initialize the USB directory structure
    pub fn init(&self) -> Result<()> {
        log::info!("Creating Pioneer USB directory structure");

        fs::create_dir_all(&self.rekordbox_dir)
            .context("Failed to create PIONEER/rekordbox directory")?;

        fs::create_dir_all(&self.usbanlz_dir)
            .context("Failed to create PIONEER/USBANLZ directory")?;

        fs::create_dir_all(&self.music_dir)
            .context("Failed to create Music directory")?;

        log::info!("USB directory structure created at {:?}", self.usb_root);
        Ok(())
    }

    /// Get the path for the export.pdb file
    pub fn pdb_path(&self) -> PathBuf {
        self.rekordbox_dir.join("export.pdb")
    }

    /// Get the path for an ANLZ file for a given track ID
    ///
    /// Pioneer's ANLZ file naming: ANLZnnnn.DAT and ANLZnnnn.EXT
    /// where nnnn is typically a hash or sequential number
    pub fn anlz_path(&self, track_id: &str, extension: &str) -> PathBuf {
        // Use first 8 chars of track ID for ANLZ filename
        let anlz_name = format!("ANLZ{}", &track_id[..8.min(track_id.len())]);
        self.usbanlz_dir.join(format!("{}.{}", anlz_name, extension))
    }

    /// Get the destination path for a music file
    ///
    /// Preserves some directory structure to avoid name conflicts
    pub fn music_file_path(&self, original_path: &Path) -> PathBuf {
        // Get the filename
        let filename = original_path
            .file_name()
            .unwrap_or_else(|| original_path.as_os_str());

        // For now, put all music files in a flat Music/ directory
        // Future: could organize by artist/album
        self.music_dir.join(filename)
    }

    /// Copy a music file to the USB
    pub fn copy_music_file(&self, source: &Path, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(source, dest)
            .with_context(|| format!("Failed to copy {:?} to {:?}", source, dest))?;

        Ok(())
    }

    /// Get the relative path of a music file from the USB root
    /// This is what goes in the PDB file_path field
    pub fn relative_music_path(&self, absolute_path: &Path) -> Option<PathBuf> {
        absolute_path.strip_prefix(&self.usb_root).ok().map(|p| p.to_path_buf())
    }

    /// Get the relative path for an ANLZ file from USB root
    /// This is what goes in the PDB analyze_path field
    pub fn relative_anlz_path(&self, track_id: &str, extension: &str) -> Option<PathBuf> {
        let anlz_abs = self.anlz_path(track_id, extension);
        anlz_abs.strip_prefix(&self.usb_root).ok().map(|p| p.to_path_buf())
    }
}
