use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: debug_columns <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");
    let header = Header::read(&mut file).expect("Failed to read header");

    // Find Columns table
    let columns_table = header.tables.iter()
        .find(|t| matches!(t.page_type, PageType::Columns))
        .expect("No Columns table");

    println!("Columns table: first={:?} last={:?}",
             columns_table.first_page, columns_table.last_page);

    // Try to read pages
    println!("Attempting to read pages...");
    match header.read_pages(
        &mut file,
        binrw::Endian::Little,
        (&columns_table.first_page, &columns_table.last_page),
    ) {
        Ok(pages) => {
            println!("Success! Read {} pages", pages.len());
            for (i, page) in pages.iter().enumerate() {
                println!("  Page {}: {} rows", i, page.num_rows());
            }
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
