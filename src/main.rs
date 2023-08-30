use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use binrw::io::Seek;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;

mod nds;
use nds::NDS;
use nds::FNTDirectoryMainTable;
use nds::FNTSubtable;
use nds::SubtableEntry;
use nds::FileAllocationTable;

fn iterate_main_table(file: &mut File, nds: &NDS, offset: u32, mut path: PathBuf, filelist: &mut Vec<PathBuf>) {
    file.seek(SeekFrom::Start(offset as u64)).unwrap();

    let main_table: FNTDirectoryMainTable = file.read_le().unwrap();

    file.seek(SeekFrom::Start(nds.fnt_offset as u64 + main_table.subtable_offset as u64)).expect("Failed to seek to first subtable");

    loop {
        let table: FNTSubtable = file.read_le().unwrap();

        match &table.data {
            SubtableEntry::FileEntry(name) => {
                let filepath = path.clone().join(PathBuf::from(name));
                println!("File entry: {:#?}", filepath);
                filelist.push(filepath);
            },
            SubtableEntry::SubdirectoryEntry(name, id) => { 
                // println!("Subdir: {} id {:#0X}", name, id);
                let offset = nds.fnt_offset + (*id as u32 & 0xFFF) * 8;
                let previous_position = file.stream_position().unwrap();
                iterate_main_table(file, nds, offset, path.clone().join(PathBuf::from(name)), filelist);
                file.seek(SeekFrom::Start(previous_position)).unwrap();
            },
            SubtableEntry::Reserved => {},
            SubtableEntry::End => break,
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut filelist = Vec::new();

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

        iterate_main_table(&mut file, &nds, nds.fnt_offset, PathBuf::from("unpacked/"), &mut filelist);

        // Jump to first file ID in FAT... don't really know what the previous entries are
        file.seek(SeekFrom::Start(nds.fat_offset as u64 + main_table.first_file_id as u64 * 8)).expect("Failed to seek to FAT");
        println!("fat offset: {:#0X}", nds.fat_offset);

        let current_dir = std::env::current_dir().expect("Failed to get current directory");
        println!("current_dir: {:?}", current_dir);

        for path in filelist.iter() {
            let fat_entry: FileAllocationTable = file.read_le().unwrap();
            let stored_position = file.stream_position().unwrap();

            let mut buffer = vec![0u8; fat_entry.end_address as usize - fat_entry.start_address as usize];

            file.seek(SeekFrom::Start(fat_entry.start_address as u64)).expect("Failed to seek to file start address");
            file.read_exact(buffer.as_mut_slice()).expect("Failed to read file data into buffer");
            
            let mut output_file_path = current_dir.clone();
            output_file_path.push(path);

            std::fs::create_dir_all(&output_file_path.parent().unwrap()).expect("Failed to create output file path");

            let mut output_file = File::create(output_file_path).expect("Failed to create output file");
            output_file.write(&buffer).expect("Failed to write data to output file");

            file.seek(SeekFrom::Start(stored_position)).unwrap();
        }
    }
}
