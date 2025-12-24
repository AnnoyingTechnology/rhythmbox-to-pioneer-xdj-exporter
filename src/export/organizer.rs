//! USB file organization and structure

use anyhow::{Context, Result};
use binrw::BinWrite;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// Manages Pioneer USB directory structure
pub struct UsbOrganizer {
    /// Root USB path
    usb_root: PathBuf,

    /// PIONEER/rekordbox directory
    rekordbox_dir: PathBuf,

    /// PIONEER/USBANLZ directory
    usbanlz_dir: PathBuf,

    /// Contents directory (where rekordbox puts audio files)
    contents_dir: PathBuf,
}

/// Maximum length for path components on FAT32 (safe limit)
const MAX_PATH_COMPONENT_LEN: usize = 200;

/// Maximum length for filenames on FAT32 (255 chars including extension)
const MAX_FILENAME_LEN: usize = 250;

/// Sanitize a string for use as a path component (artist, album names)
/// Handles FAT32 restrictions: illegal characters, case-insensitivity, length limits
fn sanitize_path_component(s: &str) -> String {
    // Replace filesystem-unsafe characters with underscores
    // FAT32 illegal: / \ : * ? " < > |
    // Also replace control characters and leading/trailing spaces/dots
    let sanitized: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            _ => c,
        })
        .collect();

    // Trim whitespace and dots from start/end (FAT32 doesn't like these)
    let trimmed = sanitized.trim().trim_matches('.');

    // Truncate to max length while preserving valid UTF-8
    truncate_to_chars(trimmed, MAX_PATH_COMPONENT_LEN)
}

/// Sanitize a filename for FAT32, preserving extension
fn sanitize_filename(filename: &str) -> String {
    // Split into name and extension
    let (name, ext) = match filename.rfind('.') {
        Some(pos) if pos > 0 => (&filename[..pos], Some(&filename[pos..])),
        _ => (filename, None),
    };

    // Sanitize the name part
    let sanitized_name: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            _ => c,
        })
        .collect();

    let trimmed_name = sanitized_name.trim().trim_matches('.');

    // Calculate max name length (leave room for extension)
    let ext_len = ext.map(|e| e.len()).unwrap_or(0);
    let max_name_len = MAX_FILENAME_LEN.saturating_sub(ext_len);

    let truncated_name = truncate_to_chars(trimmed_name, max_name_len);

    // Reconstruct filename
    match ext {
        Some(e) => format!("{}{}", truncated_name, e),
        None => truncated_name,
    }
}

/// Truncate a string to a maximum number of characters, preserving UTF-8
fn truncate_to_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Compute ANLZ path components from a file path using FNV-1a hash
/// Returns (p_value, hash_value) for the hierarchical path structure
/// Path format: /PIONEER/USBANLZ/P{XXX}/{XXXXXXXX}/ANLZ0000.{ext}
fn compute_anlz_path_hash(file_path: &str) -> (u16, u32) {
    let bytes = file_path.as_bytes();

    // Compute a 32-bit hash using FNV-1a algorithm
    let mut hash: u32 = 0x811c9dc5; // FNV offset basis
    for &byte in bytes {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(0x01000193); // FNV prime
    }

    // P value: use upper 12 bits, masked to 3 hex digits (0x000-0xFFF)
    let p_value = ((hash >> 20) & 0xFFF) as u16;

    // Hash value: use lower 24 bits, extended to 8 hex digits
    let hash_value = hash & 0x00FFFFFF;

    (p_value, hash_value)
}

impl UsbOrganizer {
    /// Create a new USB organizer for the given USB mount point
    pub fn new(usb_root: PathBuf) -> Result<Self> {
        let rekordbox_dir = usb_root.join("PIONEER").join("rekordbox");
        let usbanlz_dir = usb_root.join("PIONEER").join("USBANLZ");
        let contents_dir = usb_root.join("Contents");

        Ok(Self {
            usb_root,
            rekordbox_dir,
            usbanlz_dir,
            contents_dir,
        })
    }

    /// Initialize the USB directory structure
    pub fn init(&self) -> Result<()> {
        log::info!("Creating Pioneer USB directory structure");

        fs::create_dir_all(&self.rekordbox_dir)
            .context("Failed to create PIONEER/rekordbox directory")?;

        fs::create_dir_all(&self.usbanlz_dir)
            .context("Failed to create PIONEER/USBANLZ directory")?;

        fs::create_dir_all(&self.contents_dir).context("Failed to create Contents directory")?;

        // Write setting files required by Pioneer hardware
        self.write_setting_files()?;

        log::info!("USB directory structure created at {:?}", self.usb_root);
        Ok(())
    }

    /// Write the setting files required by Pioneer hardware
    fn write_setting_files(&self) -> Result<()> {
        use rekordcrate::setting::Setting;

        let pioneer_dir = self.usb_root.join("PIONEER");

        // DEVSETTING.DAT
        let devsetting_path = pioneer_dir.join("DEVSETTING.DAT");
        let devsetting = Setting::default_devsetting();
        let file = File::create(&devsetting_path)
            .with_context(|| format!("Failed to create {:?}", devsetting_path))?;
        let mut writer = BufWriter::new(file);
        devsetting
            .write_le(&mut writer)
            .with_context(|| "Failed to write DEVSETTING.DAT")?;
        log::debug!("Written DEVSETTING.DAT");

        // MYSETTING.DAT
        let mysetting_path = pioneer_dir.join("MYSETTING.DAT");
        let mysetting = Setting::default_mysetting();
        let file = File::create(&mysetting_path)
            .with_context(|| format!("Failed to create {:?}", mysetting_path))?;
        let mut writer = BufWriter::new(file);
        mysetting
            .write_le(&mut writer)
            .with_context(|| "Failed to write MYSETTING.DAT")?;
        log::debug!("Written MYSETTING.DAT");

        // MYSETTING2.DAT
        let mysetting2_path = pioneer_dir.join("MYSETTING2.DAT");
        let mysetting2 = Setting::default_mysetting2();
        let file = File::create(&mysetting2_path)
            .with_context(|| format!("Failed to create {:?}", mysetting2_path))?;
        let mut writer = BufWriter::new(file);
        mysetting2
            .write_le(&mut writer)
            .with_context(|| "Failed to write MYSETTING2.DAT")?;
        log::debug!("Written MYSETTING2.DAT");

        // djprofile.nxs - DJ profile file required by Pioneer hardware
        let djprofile_path = pioneer_dir.join("djprofile.nxs");
        let mut djprofile_data = vec![0u8; 160];
        // Header bytes from reference (first 32 bytes)
        djprofile_data[0..4].copy_from_slice(&[0x00, 0x1e, 0x8c, 0x3c]);
        djprofile_data[4..8].copy_from_slice(&[0x00, 0x00, 0x01, 0x70]);
        djprofile_data[8..12].copy_from_slice(&[0xa1, 0x75, 0x29, 0x0f]);
        // Zeros from 12-29
        djprofile_data[30..32].copy_from_slice(&[0xde, 0x01]);
        // Profile name at offset 32 (padded to fill rest)
        let name = b"Pioneer Export";
        djprofile_data[32..32 + name.len()].copy_from_slice(name);
        fs::write(&djprofile_path, &djprofile_data)
            .with_context(|| "Failed to write djprofile.nxs")?;
        log::debug!("Written djprofile.nxs");

        Ok(())
    }

    /// Get the path for the export.pdb file
    pub fn pdb_path(&self) -> PathBuf {
        self.rekordbox_dir.join("export.pdb")
    }

    /// Get the path for the exportExt.pdb file
    pub fn pdb_ext_path(&self) -> PathBuf {
        self.rekordbox_dir.join("exportExt.pdb")
    }

    /// Get the path for an ANLZ file using hierarchical structure
    ///
    /// Pioneer's ANLZ file structure: /PIONEER/USBANLZ/P{XXX}/{XXXXXXXX}/ANLZ0000.{ext}
    /// where P{XXX} and {XXXXXXXX} are derived from the audio file path
    pub fn anlz_path(&self, audio_path: &str, extension: &str) -> PathBuf {
        let (p_value, hash_value) = compute_anlz_path_hash(audio_path);

        // Format: P{XXX}/{XXXXXXXX}/ANLZ0000.{ext}
        let p_dir = format!("P{:03X}", p_value);
        let hash_dir = format!("{:08X}", hash_value);

        self.usbanlz_dir
            .join(p_dir)
            .join(hash_dir)
            .join(format!("ANLZ0000.{}", extension))
    }

    /// Get the destination path for a music file
    ///
    /// Organizes files into Contents/Artist/Album/filename structure like Rekordbox does
    /// Handles FAT32 case-insensitivity by normalizing to lowercase for collision detection
    pub fn music_file_path(&self, original_path: &Path, artist: &str, album: &str) -> PathBuf {
        // Get the filename and sanitize for FAT32/exFAT compatibility
        let filename = original_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let safe_filename = sanitize_filename(&filename);

        // Organize by Artist/Album like Rekordbox does
        // Sanitize artist and album names for filesystem safety
        let safe_artist = sanitize_path_component(artist);
        let safe_album = sanitize_path_component(album);

        // Handle empty names after sanitization
        let safe_artist = if safe_artist.is_empty() {
            "Unknown Artist".to_string()
        } else {
            safe_artist
        };
        let safe_album = if safe_album.is_empty() {
            "Unknown Album".to_string()
        } else {
            safe_album
        };

        self.contents_dir.join(safe_artist).join(safe_album).join(safe_filename)
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
    /// Paths must start with / for Pioneer compatibility
    pub fn relative_music_path(&self, absolute_path: &Path) -> Option<PathBuf> {
        absolute_path.strip_prefix(&self.usb_root).ok().map(|p| {
            // Pioneer paths must start with /
            PathBuf::from("/").join(p)
        })
    }

    /// Get the relative path for an ANLZ file from USB root
    /// This is what goes in the PDB analyze_path field
    /// Paths must start with / for Pioneer compatibility
    pub fn relative_anlz_path(&self, audio_path: &str, extension: &str) -> Option<PathBuf> {
        let anlz_abs = self.anlz_path(audio_path, extension);
        anlz_abs.strip_prefix(&self.usb_root).ok().map(|p| {
            // Pioneer paths must start with /
            PathBuf::from("/").join(p)
        })
    }
}
