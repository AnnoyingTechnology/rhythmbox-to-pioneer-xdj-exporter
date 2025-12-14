//! Main export pipeline orchestration

use super::config::ExportConfig;
use super::organizer::UsbOrganizer;
use crate::analysis::{AnalysisResult, AudioAnalyzer};
use crate::model::Library;
use anyhow::{Context, Result};
use std::collections::HashMap;

/// Main export pipeline
pub struct ExportPipeline<A: AudioAnalyzer> {
    config: ExportConfig,
    organizer: UsbOrganizer,
    analyzer: A,
}

impl<A: AudioAnalyzer> ExportPipeline<A> {
    /// Create a new export pipeline
    pub fn new(config: ExportConfig, analyzer: A) -> Result<Self> {
        let organizer = UsbOrganizer::new(config.usb_path.clone())?;

        Ok(Self {
            config,
            organizer,
            analyzer,
        })
    }

    /// Run the complete export process
    pub fn export(&self, library: &Library) -> Result<()> {
        log::info!("Starting Pioneer USB export");
        log::info!("Target: {:?}", self.config.usb_path);

        // Filter library if playlist filter is specified
        let (filtered_library, track_ids) = if let Some(ref filter) = self.config.playlist_filter {
            log::info!("Filtering to playlists: {:?}", filter);
            self.filter_library(library, filter)?
        } else {
            // No filter, export everything
            let all_track_ids: std::collections::HashSet<String> =
                library.tracks().map(|t| t.id.clone()).collect();
            (library.clone(), all_track_ids)
        };

        log::info!(
            "Exporting {} tracks, {} playlists",
            track_ids.len(),
            filtered_library.playlist_count()
        );

        // Step 1: Initialize USB structure
        self.organizer.init()?;

        // Step 2: Analyze and copy tracks (filtered library already contains only needed tracks)
        let analysis_results = self.process_tracks(&filtered_library)?;

        // Step 3: Write ANLZ files
        self.write_anlz_files(&filtered_library, &analysis_results)?;

        // Step 4: Write PDB file
        self.write_pdb(&filtered_library, &analysis_results)?;

        log::info!("Export complete!");
        Ok(())
    }

    /// Filter library to only include specified playlists and their tracks
    fn filter_library(
        &self,
        library: &Library,
        playlist_names: &[String],
    ) -> Result<(Library, std::collections::HashSet<String>)> {
        use std::collections::HashSet;

        let mut filtered_lib = Library::new();
        let mut track_ids = HashSet::new();

        // Filter playlists
        for playlist in library.playlists() {
            if playlist_names.contains(&playlist.name) {
                log::info!("Including playlist: {} ({} tracks)", playlist.name, playlist.len());

                // Collect track IDs from this playlist
                for entry in &playlist.entries {
                    track_ids.insert(entry.track_id.clone());
                }

                filtered_lib.add_playlist(playlist.clone());
            }
        }

        // Add only the tracks that are in the filtered playlists
        for track_id in &track_ids {
            if let Some(track) = library.get_track(track_id) {
                filtered_lib.add_track(track.clone());
            }
        }

        log::info!(
            "Filtered to {} tracks from {} playlists",
            track_ids.len(),
            filtered_lib.playlist_count()
        );

        Ok((filtered_lib, track_ids))
    }

    /// Process all tracks: analyze and copy audio files
    fn process_tracks(&self, library: &Library) -> Result<HashMap<String, AnalysisResult>> {
        log::info!("Processing tracks...");
        let mut results = HashMap::new();

        for (i, track) in library.tracks().enumerate() {
            log::info!(
                "[{}/{}] Processing: {} - {}",
                i + 1,
                library.track_count(),
                track.artist,
                track.title
            );

            // Analyze the track
            let analysis = self
                .analyzer
                .analyze(&track.file_path)
                .with_context(|| format!("Failed to analyze track: {:?}", track.file_path))?;

            // Copy audio file to USB
            if self.config.copy_audio {
                let dest_path = self.organizer.music_file_path(&track.file_path);
                self.organizer
                    .copy_music_file(&track.file_path, &dest_path)
                    .with_context(|| format!("Failed to copy track: {:?}", track.file_path))?;

                log::debug!("Copied to: {:?}", dest_path);
            }

            results.insert(track.id.clone(), analysis);
        }

        log::info!("Track processing complete");
        Ok(results)
    }

    /// Write ANLZ files for all tracks
    fn write_anlz_files(
        &self,
        library: &Library,
        analysis_results: &HashMap<String, AnalysisResult>,
    ) -> Result<()> {
        log::info!("Writing ANLZ files...");

        for track in library.tracks() {
            let analysis = analysis_results
                .get(&track.id)
                .context("Missing analysis result for track")?;

            // Write .DAT file
            let dat_path = self.organizer.anlz_path(&track.id, "DAT");
            crate::anlz::write_dat_file(&dat_path, track, analysis)?;

            // Write .EXT file
            let ext_path = self.organizer.anlz_path(&track.id, "EXT");
            crate::anlz::write_ext_file(&ext_path, track, analysis)?;
        }

        log::info!("ANLZ files written");
        Ok(())
    }

    /// Write the PDB database file
    fn write_pdb(
        &self,
        library: &Library,
        analysis_results: &HashMap<String, AnalysisResult>,
    ) -> Result<()> {
        log::info!("Writing PDB file...");

        let pdb_path = self.organizer.pdb_path();

        // Build track metadata with file paths and ANLZ paths
        let mut track_metadata = Vec::new();
        for track in library.tracks() {
            let music_path = self.organizer.music_file_path(&track.file_path);
            let relative_music_path = self
                .organizer
                .relative_music_path(&music_path)
                .context("Failed to compute relative music path")?;

            let relative_anlz_path = self
                .organizer
                .relative_anlz_path(&track.id, "DAT")
                .context("Failed to compute relative ANLZ path")?;

            let analysis = analysis_results
                .get(&track.id)
                .context("Missing analysis result")?;

            track_metadata.push(crate::pdb::TrackMetadata {
                track: track.clone(),
                file_path: relative_music_path,
                anlz_path: relative_anlz_path,
                analysis: analysis.clone(),
            });
        }

        crate::pdb::write_pdb(&pdb_path, &track_metadata, library.playlists())?;

        log::info!("PDB file written to: {:?}", pdb_path);
        Ok(())
    }
}
