use std::env;
use std::fs::File;
use std::path::PathBuf;

use binrw::io::Seek;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;

mod nds;
use nds::NDS;
use nds::FNTDirectoryMainTable;
use nds::FNTSubtable;
use nds::SubtableEntry;

fn iterate_main_table(file: &mut File, nds: &NDS, offset: u32, mut path: PathBuf) {
    file.seek(SeekFrom::Start(offset as u64)).unwrap();

    let main_table: FNTDirectoryMainTable = file.read_le().unwrap();

    file.seek(SeekFrom::Start(nds.fnt_offset as u64 + main_table.subtable_offset as u64)).expect("Failed to seek to first subtable");

    loop {
        let table: FNTSubtable = file.read_le().unwrap();

        match &table.data {
            SubtableEntry::FileEntry(name) => {
                let filepath = path.clone().join(PathBuf::from(name));
                println!("File entry: {:#?}", filepath);
            },
            SubtableEntry::SubdirectoryEntry(name, id) => { 
                // println!("Subdir: {} id {:#0X}", name, id);
                let offset = nds.fnt_offset + (*id as u32 & 0xFFF) * 8;
                let previous_position = file.stream_position().unwrap();
                iterate_main_table(file, nds, offset, path.clone().join(PathBuf::from(name)));
                file.seek(SeekFrom::Start(previous_position)).unwrap();
            },
            SubtableEntry::Reserved => {},
            SubtableEntry::End => break,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: mon-rober <path>");
    }

    else {
        let path = PathBuf::from(args.get(1).unwrap());
        let mut file = File::open(path).expect("Failed to open file");
        let nds: NDS = file.read_le().expect("Failed to read file");
        file.seek(SeekFrom::Start(nds.fnt_offset as u64)).expect("Failed to seek to FNT");
        let main_table: FNTDirectoryMainTable =  file.read_le().unwrap();
        println!("first offset: {:0X}", main_table.subtable_offset);

        let total_dirs = main_table.directory_id;
        println!("total dirs: {total_dirs}");

        iterate_main_table(&mut file, &nds, nds.fnt_offset, PathBuf::from("/"));
    }
}
