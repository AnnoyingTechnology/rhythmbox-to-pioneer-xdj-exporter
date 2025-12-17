use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: list_columns <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");
    let header = Header::read(&mut file).expect("Failed to read header");

    println!("=== Columns Table ===\n");
    if let Some(table) = header.tables.iter()
        .find(|t| matches!(t.page_type, PageType::Columns)) {

        let pages = header.read_pages(
            &mut file,
            binrw::Endian::Little,
            (&table.first_page, &table.last_page),
        ).expect("Failed to read columns pages");

        for page in pages {
            println!("Page has {} row groups", page.row_groups.len());
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    println!("{:?}", row);
                }
            }
        }
    } else {
        println!("No Columns table found");
    }

    // Also check Unknown16, Unknown17, Unknown18 which have data in reference
    for (type_num, name) in [(16, "Unknown16"), (17, "Unknown17"), (18, "Unknown18")] {
        println!("\n=== {} (type {}) ===\n", name, type_num);

        // Find table by type number in page_type
        for table in &header.tables {
            // We need to check if this is our target type
            // PageType enum values should match
            let pages = header.read_pages(
                &mut file,
                binrw::Endian::Little,
                (&table.first_page, &table.last_page),
            );

            if let Ok(pages) = pages {
                for page in &pages {
                    if page.num_rows() > 0 {
                        println!("Table {:?} has {} rows", table.page_type, page.num_rows());
                    }
                }
            }
        }
    }
}
