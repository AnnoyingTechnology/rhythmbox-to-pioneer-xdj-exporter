use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: list_keys <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");

    let header = Header::read(&mut file).expect("Failed to read header");

    // List keys
    println!("=== Keys ===\n");
    if let Some(table) = header
        .tables
        .iter()
        .find(|t| matches!(t.page_type, PageType::Keys))
    {
        let pages = header
            .read_pages(
                &mut file,
                binrw::Endian::Little,
                (&table.first_page, &table.last_page),
            )
            .expect("Failed to read keys pages");

        for page in pages {
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::Key(key) = row {
                        println!("{:?}", key);
                    }
                }
            }
        }
    }

    // List colors
    println!("\n=== Colors ===\n");
    if let Some(table) = header
        .tables
        .iter()
        .find(|t| matches!(t.page_type, PageType::Colors))
    {
        let pages = header
            .read_pages(
                &mut file,
                binrw::Endian::Little,
                (&table.first_page, &table.last_page),
            )
            .expect("Failed to read colors pages");

        for page in pages {
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::Color(color) = row {
                        println!("{:?}", color);
                    }
                }
            }
        }
    }

    // List labels
    println!("\n=== Labels ===\n");
    if let Some(table) = header
        .tables
        .iter()
        .find(|t| matches!(t.page_type, PageType::Labels))
    {
        let pages = header
            .read_pages(
                &mut file,
                binrw::Endian::Little,
                (&table.first_page, &table.last_page),
            )
            .expect("Failed to read labels pages");

        for page in pages {
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::Label(label) = row {
                        println!("{:?}", label);
                    }
                }
            }
        }
    }
}
