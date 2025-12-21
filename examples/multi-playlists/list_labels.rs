use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).expect("Usage: list_labels <pdb_file>");

    let mut file = File::open(path).expect("Failed to open file");
    let header = Header::read(&mut file).expect("Failed to read header");

    println!("=== Labels Table ===\n");
    if let Some(table) = header
        .tables
        .iter()
        .find(|t| matches!(t.page_type, PageType::Labels))
    {
        println!(
            "First page: {:?}, Last page: {:?}",
            table.first_page, table.last_page
        );

        let pages = header
            .read_pages(
                &mut file,
                binrw::Endian::Little,
                (&table.first_page, &table.last_page),
            )
            .expect("Failed to read labels pages");

        for page in pages {
            println!("Page has {} row groups", page.row_groups.len());
            for row_group in &page.row_groups {
                for row in row_group.present_rows() {
                    if let Row::Label(label) = row {
                        println!("{:?}", label);
                    }
                }
            }
        }
    } else {
        println!("No Labels table found");
    }
}
