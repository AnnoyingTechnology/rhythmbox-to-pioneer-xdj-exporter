use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: list_playlists <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");

    let header = Header::read(&mut file).expect("Failed to read header");

    // List playlist tree
    println!("=== Playlist Tree ===\n");
    if let Some(table) = header.tables.iter()
        .find(|t| matches!(t.page_type, PageType::PlaylistTree)) {

        let pages = header.read_pages(
            &mut file,
            binrw::Endian::Little,
            (&table.first_page, &table.last_page),
        ).expect("Failed to read playlist tree pages");

        for page in pages {
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::PlaylistTreeNode(node) = row {
                        println!("{:?}", node);
                    }
                }
            }
        }
    }

    // List playlist entries
    println!("\n=== Playlist Entries ===\n");
    if let Some(table) = header.tables.iter()
        .find(|t| matches!(t.page_type, PageType::PlaylistEntries)) {

        let pages = header.read_pages(
            &mut file,
            binrw::Endian::Little,
            (&table.first_page, &table.last_page),
        ).expect("Failed to read playlist entries pages");

        for page in pages {
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::PlaylistEntry(entry) = row {
                        println!("{:?}", entry);
                    }
                }
            }
        }
    }
}
