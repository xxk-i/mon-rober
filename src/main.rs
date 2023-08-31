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
use nds::narc;

fn iterate_main_table(file: &mut File, fnt_offset: u32, subtable_offset: u32, path: PathBuf, filelist: &mut Vec<PathBuf>) {
    file.seek(SeekFrom::Start(subtable_offset as u64)).unwrap();

    let main_table: FNTDirectoryMainTable = file.read_le().unwrap();

    file.seek(SeekFrom::Start(fnt_offset as u64 + main_table.subtable_offset as u64)).expect("Failed to seek to first subtable");

    loop {
        let table: FNTSubtable = file.read_le().unwrap();

        match &table.data {
            SubtableEntry::FileEntry(name) => {
                let filepath = path.clone().join(PathBuf::from(name));
                println!("File entry: {:#?}", filepath);
                filelist.push(filepath);
            },
            SubtableEntry::SubdirectoryEntry(name, id) => { 
                let offset = fnt_offset + (*id as u32 & 0xFFF) * 8;
                let previous_position = file.stream_position().unwrap();
                iterate_main_table(file, fnt_offset, offset, path.clone().join(PathBuf::from(name)), filelist);
                file.seek(SeekFrom::Start(previous_position)).unwrap();
            },
            SubtableEntry::Reserved => {},
            SubtableEntry::End => break,
        }
    }
}

fn iterate_narc_main_table(file: &mut File, fnt_offset: u32, path: PathBuf, filelist: &mut Vec<PathBuf>) {
    file.seek(SeekFrom::Start(fnt_offset as u64)).unwrap();

    let main_table: FNTDirectoryMainTable = file.read_le().unwrap();

    println!("main_table: {:#?}", main_table);

    file.seek(SeekFrom::Start(fnt_offset as u64 + main_table.subtable_offset as u64)).expect("Failed to seek to first subtable");

    loop {
        let table: FNTSubtable = file.read_le().unwrap();

        match &table.data {
            SubtableEntry::FileEntry(name) => {
                let filepath = path.clone().join(PathBuf::from(name));
                println!("File entry: {:#?}", filepath);
                filelist.push(filepath);
            },
            SubtableEntry::SubdirectoryEntry(name, id) => { 
                let previous_position = file.stream_position().unwrap();
                iterate_narc_main_table(file, fnt_offset, path.clone().join(PathBuf::from(name)), filelist);
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

        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        // MAIN ROM
        if path.extension().is_some_and(|extension| extension.eq("nds")) {
            let mut file = File::open(path).expect("Failed to open file");
            let nds: NDS = file.read_le().expect("Failed to read file");
            file.seek(SeekFrom::Start(nds.fnt_offset as u64)).expect("Failed to seek to FNT");
            let main_table: FNTDirectoryMainTable =  file.read_le().unwrap();
            println!("first offset: {:0X}", main_table.subtable_offset);

            let total_dirs = main_table.directory_id;
            println!("total dirs: {total_dirs}");

            // collects all FNT entries
            iterate_main_table(&mut file, nds.fnt_offset, nds.fnt_offset, PathBuf::from("unpacked/"), &mut filelist);

            // Jump to first file ID in FAT... don't really know what the previous entries are
            file.seek(SeekFrom::Start(nds.fat_offset as u64 + main_table.first_file_id as u64 * 8)).expect("Failed to seek to FAT");
            // println!("fat offset: {:#0X}", nds.fat_offset);

            // println!("current_dir: {:?}", current_dir);

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

        // NARC
        // else if path.extension().unwrap().eq("narc") {
        else {
            let mut file = File::open(&path).expect("Failed to open NARC");
            let narc: narc::NARC = file.read_le().expect("Failed to read NARC");

            // Have to navigate to the start of the FNT inside of the FNTBlock manually since there
            // is no offset saved inside of the NARC header

            let mut fnt_offset: u32 = 0;

            fnt_offset += 0x1C; // seek to start of FAT

            fnt_offset += 8 * narc.fat_block.num_files as u32; // seek past size of FAT

            fnt_offset += 8; // seek past FATBlock info

            file.seek(SeekFrom::Start(fnt_offset as u64)).unwrap();

            println!("fnt_offset: {:#0X}", fnt_offset);

            iterate_narc_main_table(&mut file, fnt_offset, path.clone(), &mut filelist);

            println!("Final positon: {:#0X}", file.stream_position().unwrap());

            if filelist.len() == 0 {
                println!("FNT contains no names, labeling files manually");
                let mut file_index = 1;
                for entry in narc.fat_block.entries {
                    // let mut buffer = vec![0u8; entry.end_address as usize - entry.start_address as usize];

                    let buffer = &narc.img_block.data[entry.start_address as usize..entry.end_address as usize];

                    let narc_name = path.file_stem().unwrap().to_str().unwrap().to_owned();

                    let mut final_dir = narc_name.clone();
                    final_dir.push_str("/");

                    let mut output_file_path = current_dir.clone();
                    output_file_path.push("narc_unpacked/");
                    output_file_path.push(&final_dir);

                    std::fs::create_dir_all(&output_file_path).expect("Failed to create output file path");

                    let mut filename = narc_name.clone();
                    filename.push_str("_");
                    filename.push_str(&file_index.to_string());

                    output_file_path.push(filename);

                    println!("output filepath: {:?}", output_file_path);

                    let mut output_file = File::create(output_file_path).expect("Failed to create output file");
                    output_file.write(&buffer).expect("Failed to write data to output file");

                    file_index += 1;
                }
            } else {
                for i in 0..filelist.len() {
                    let end_address = narc.fat_block.entries[i].end_address;
                    let start_address = narc.fat_block.entries[i].start_address;

                    let buffer = &narc.img_block.data[start_address as usize..end_address as usize];

                    let mut output_file_path = current_dir.clone();
                    output_file_path.push(filelist.get(i).unwrap());

                    std::fs::create_dir_all(&output_file_path.parent().unwrap()).expect("Failed to create output file path");

                    let mut output_file = File::create(output_file_path).expect("Failed to create output file");
                    output_file.write(&buffer).expect("Failed to write data to output file");
                }

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
    }
}
