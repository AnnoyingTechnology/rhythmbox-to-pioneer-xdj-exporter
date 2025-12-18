use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: list_tracks <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");

    let header = Header::read(&mut file).expect("Failed to read header");

    println!("PDB file: {}", path);
    println!("Page size: {}", header.page_size);
    println!("Tables: {}", header.tables.len());
    println!();

    // Find the tracks table
    let tracks_table = header
        .tables
        .iter()
        .find(|t| matches!(t.page_type, PageType::Tracks))
        .expect("No tracks table found");

    println!(
        "Tracks table: first_page={:?}, last_page={:?}",
        tracks_table.first_page, tracks_table.last_page
    );

    // Read track pages
    let pages = header
        .read_pages(
            &mut file,
            binrw::Endian::Little,
            (&tracks_table.first_page, &tracks_table.last_page),
        )
        .expect("Failed to read track pages");

    println!("\nTracks (in file order):");
    println!("{:>4} | Title", "Pos");
    println!("{}", "-".repeat(80));

    let mut pos = 1;
    for page in pages {
        for row_group in &page.row_groups {
            for row in row_group.present_rows() {
                if let Row::Track(track) = row {
                    // Track is private, but we can at least print debug
                    println!("{:4} | {:?}", pos, track);
                    pos += 1;
                }
            }
        }
    }
}
