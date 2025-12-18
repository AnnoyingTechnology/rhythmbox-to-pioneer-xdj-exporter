//! Round-trip validation using rekordcrate

use anyhow::{Context, Result};
use binrw::BinRead;
use rekordcrate::pdb::Header;
use std::fs::File;
use std::path::Path;

/// Validate an exported USB stick by parsing with rekordcrate
///
/// # Arguments
/// * `usb_path` - Root path of the USB stick to validate
///
/// # Returns
/// Ok(()) if validation passes, Err otherwise
pub fn validate_export(usb_path: &Path) -> Result<()> {
    log::info!("Validating export at: {:?}", usb_path);

    // Find the PDB file (check both PIONEER/ and rekordbox/ directly)
    let pdb_path = if usb_path.join("PIONEER/rekordbox/export.pdb").exists() {
        usb_path.join("PIONEER/rekordbox/export.pdb")
    } else if usb_path.join("rekordbox/export.pdb").exists() {
        usb_path.join("rekordbox/export.pdb")
    } else {
        anyhow::bail!(
            "PDB file not found at: {:?}",
            usb_path.join("PIONEER/rekordbox/export.pdb")
        );
    };

    log::info!("Found PDB at: {:?}", pdb_path);

    // Get file size
    let metadata = std::fs::metadata(&pdb_path)?;
    let file_size = metadata.len();
    log::info!("PDB file size: {} bytes", file_size);

    // Open and parse the PDB with rekordcrate
    log::info!("Attempting to parse PDB with rekordcrate...");

    let mut file =
        File::open(&pdb_path).with_context(|| format!("Failed to open PDB: {:?}", pdb_path))?;

    match Header::read(&mut file) {
        Ok(header) => {
            log::info!("✅ rekordcrate successfully parsed the PDB header!");

            // Print header info
            log::info!("PDB Header:");
            log::info!("  - Page size: {} bytes", header.page_size);
            log::info!("  - Sequence: {}", header.sequence);
            log::info!("  - Tables: {}", header.tables.len());

            // Print table info
            for (idx, table) in header.tables.iter().enumerate() {
                log::info!("  Table {}: {:?}", idx, table.page_type);
                log::info!("    First page: {:?}", table.first_page);
                log::info!("    Last page: {:?}", table.last_page);
            }

            // Try to read pages from the album table first
            if let Some(album_table) = header
                .tables
                .iter()
                .find(|t| matches!(t.page_type, rekordcrate::pdb::PageType::Albums))
            {
                log::info!("Attempting to read album pages...");
                match header.read_pages(
                    &mut file,
                    binrw::Endian::Little,
                    (&album_table.first_page, &album_table.last_page),
                ) {
                    Ok(pages) => {
                        log::info!("✅ Successfully read {} album page(s)!", pages.len());
                        for (idx, page) in pages.iter().enumerate() {
                            log::info!("  Album page {}: {} rows", idx, page.num_rows());
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to read album pages!");
                        log::error!("Error: {:?}", e);
                        return Err(anyhow::anyhow!("Failed to read album pages: {}", e));
                    }
                }
            }

            // Try to read pages from the track table
            if let Some(track_table) = header
                .tables
                .iter()
                .find(|t| matches!(t.page_type, rekordcrate::pdb::PageType::Tracks))
            {
                log::info!("Attempting to read track pages...");
                match header.read_pages(
                    &mut file,
                    binrw::Endian::Little,
                    (&track_table.first_page, &track_table.last_page),
                ) {
                    Ok(pages) => {
                        log::info!("✅ Successfully read {} track page(s)!", pages.len());
                        for (idx, page) in pages.iter().enumerate() {
                            log::info!("  Track page {}: {} rows", idx, page.num_rows());
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to read track pages!");
                        log::error!("Error: {:?}", e);
                        return Err(anyhow::anyhow!("Failed to read track pages: {}", e));
                    }
                }
            }

            Ok(())
        }
        Err(e) => {
            log::error!("❌ rekordcrate FAILED to parse PDB!");
            log::error!("Error: {:?}", e);
            anyhow::bail!("PDB validation failed: {}", e)
        }
    }
}
