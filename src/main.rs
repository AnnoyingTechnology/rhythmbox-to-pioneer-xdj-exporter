use anyhow::Result;
use clap::Parser;
use pioneer_exporter::{ExportConfig, ExportPipeline};
use pioneer_exporter::analysis::StubAnalyzer;
use pioneer_exporter::validation::validate_export;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pioneer-exporter")]
#[command(about = "Export Rhythmbox library to Pioneer USB format", long_about = None)]
struct Args {
    /// Path to Rhythmbox database (rhythmdb.xml)
    #[arg(short = 'd', long, default_value = "~/.local/share/rhythmbox/rhythmdb.xml")]
    database: String,

    /// Path to Rhythmbox playlists (playlists.xml)
    #[arg(short = 'p', long, default_value = "~/.local/share/rhythmbox/playlists.xml")]
    playlists: String,

    /// Target USB mount point
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Export only specific playlists (can be specified multiple times)
    #[arg(long = "playlist")]
    playlists_filter: Vec<String>,

    /// Verbose logging
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Only validate existing export (don't create new export)
    #[arg(long)]
    validate: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    log::info!("Pioneer Exporter - Phase 1 (Stub Analysis)");
    log::info!("===========================================");

    // If validate-only mode, just validate and exit
    if args.validate {
        log::info!("Validation mode - checking existing export...");
        validate_export(&args.output)?;
        log::info!("✅ Validation completed!");
        return Ok(());
    }

    // Expand ~ in paths
    let db_path = shellexpand::tilde(&args.database);
    let playlists_path = shellexpand::tilde(&args.playlists);

    // Parse Rhythmbox library
    log::info!("Loading Rhythmbox library...");
    let library = pioneer_exporter::rhythmbox::parse_library(
        PathBuf::from(db_path.as_ref()).as_path(),
        PathBuf::from(playlists_path.as_ref()).as_path(),
    )?;

    log::info!("Library loaded: {} tracks, {} playlists",
        library.track_count(),
        library.playlist_count()
    );

    // Create export configuration
    let mut config = ExportConfig::new(args.output.clone());

    // Apply playlist filter if specified
    if !args.playlists_filter.is_empty() {
        log::info!("Filtering to {} playlist(s): {:?}", args.playlists_filter.len(), args.playlists_filter);
        config = config.with_playlists(args.playlists_filter);
    }

    // Create export pipeline with stub analyzer (Phase 1)
    let analyzer = StubAnalyzer::new();
    let pipeline = ExportPipeline::new(config, analyzer)?;

    // Run export
    pipeline.export(&library)?;

    log::info!("Export completed successfully!");
    log::info!("USB stick ready at: {:?}", args.output);

    // Auto-validate after export
    log::info!("Running post-export validation...");
    validate_export(&args.output)?;
    log::info!("✅ Validation passed!");

    Ok(())
}
