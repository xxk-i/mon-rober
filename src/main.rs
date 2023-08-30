use std::env;
use std::fs::File;
use std::path::PathBuf;

use binrw::BinRead;
use binrw::BinResult;
use binrw::Endian;
use binrw::io::Seek;
use binrw::io::Read;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;
use binrw::binrw;
use binrw::NullString;


// RESOURCES
// https://web.archive.org/web/20060623000027/http://nocash.emubase.de/gbatek.htm
// https://dsibrew.org/wiki/DSi_cartridge_header

#[binrw]
#[derive(Debug)]
struct NDS {
    #[br(align_after = 12)]
    game_title: NullString, 
    gamecode: u32,
    makercode: u16,
    unitcode: u8,
    encrypted_seed_select: u8,
    device_capacity: u8,
    reserved_7: [u8;7],
    game_revision: u16,
    rom_version: u8,
    internal_flags: u8,

    // ARM9
    arm9_rom_offset: u32,
    arm9_entry_address: u32,
    arm9_load_address: u32,
    arm9_size: u32,

    // ARM7
    arm7_rom_offse: u32,
    arm7_entry_address: u32,
    arm7_load_address: u32,
    arm7_size: u32,

    // FILE NAME TABLE (FNT)
    fnt_offset: u32,
    fnt_length: u32,

    // FILE ALLOCATION TABLE (FAT)
    fat_offset: u32,
    fat_length: u32,

    // ARM9 OVERLAY
    arm9_overlay_offset: u32,
    arm9_overlay_length: u32,

    // ARM7 OVERLAY
    arm7_overlay_offset: u32,
    arm7_overlay_length: u32,

    // CARD CONTROL REGISTER SETTINGS (CCRS)
    normal_ccrs: u32,
    secure_ccrs: u32,

    icon_banner_offset: u32,
    secure_area_crc: u16,
    secure_transfer_timeout: u16,
    arm9_autoload: u32,
    arm7_autoload: u32,
    secure_disable: u64,
    ntr_region_rom_size: u32,
    header_size: u32,
    reserved_56: [u8;56],
    nintendo_logo: [u8;156],
    nintendo_logo_crc: u16,
    header_crc: u16,
    debugger_reserved: [u8;32],
}

// http://problemkaputt.de/gbatek-ds-cartridge-nitrorom-and-nitroarc-file-systems.htm

// struct FileNameTable {
//     ...
// }

#[derive(Debug)]
enum SubtableEntry {
    FileEntry(String),

    SubdirectoryEntry(String, u16),

    Reserved,

    End
}

#[derive(Debug)]
#[binrw]
// #[br(assert(total_directories < 4096, "total_directories is greater than 4096: {}", total_directories))]
struct FNTDirectoryMainTable {
    subtable_offset: u32,
    first_file_id: u16,

    directory_id: u16,
}

fn parse_subtable<R: Read + Seek>(reader: &mut R, _ro: Endian, args: (u8,)) -> BinResult<SubtableEntry> {
    let datatype = args.0;

    return match datatype {
        0 => Ok(SubtableEntry::End),

        1..=0x7F => {
            let mut buffer = vec![0; datatype as usize];
            reader.read_exact(buffer.as_mut_slice())?;
            Ok(SubtableEntry::FileEntry(String::from_utf8(buffer.as_slice().clone().to_owned()).expect("Failed to interpret subtable name")))
        },

        0x80 => {
            Ok(SubtableEntry::Reserved)
        },

        0x81..=0xFF => {
            // println!("found subtable entry: {:#08X} size: {}", datatype, datatype - 0x80);
            // println!("at: {:#08X}", reader.stream_position().unwrap());
            let mut buffer = vec![0; (datatype - 0x80) as usize];
            reader.read_exact(&mut buffer).unwrap();
            let mut id = [0u8, 0u8];
            reader.read_exact(&mut id).unwrap();
            Ok(SubtableEntry::SubdirectoryEntry(String::from_utf8(buffer.as_slice().clone().to_owned()).unwrap(), u16::from_le_bytes(id)))
        },
    };
}

#[derive(Debug, BinRead)]
struct FNTSubtable {
    table_type: u8,

    // https://github.com/jam1garner/binrw/issues/73#issuecomment-935758313
    #[br(args(table_type), parse_with = parse_subtable)]
    data: SubtableEntry,
}

// fn iterate_main_table(file: &mut File, nds: &NDS, main_table: &FNTDirectoryMainTable) {
fn iterate_main_table(file: &mut File, nds: &NDS, offset: u32) {
    file.seek(SeekFrom::Start(offset as u64)).unwrap();

    let main_table: FNTDirectoryMainTable = file.read_le().unwrap();

    file.seek(SeekFrom::Start(nds.fnt_offset as u64 + main_table.subtable_offset as u64)).expect("Failed to seek to first subtable");

    loop {
        let table: FNTSubtable = file.read_le().unwrap();

        match &table.data {
            SubtableEntry::FileEntry(name) => println!("File entry: {}", name),
            SubtableEntry::SubdirectoryEntry(name, id) => { 
                println!("Subdir: {} id {:#0X}", name, id);
                let offset = nds.fnt_offset + (*id as u32 & 0xFFF) * 8;
                // println!("offset: {:0X}", offset);
                // file.seek(SeekFrom::Start(offset as u64)).unwrap();
                // let next_main_table: FNTDirectoryMainTable = file.read_le().unwrap();
                let previous_position = file.stream_position().unwrap();
                iterate_main_table(file, nds, offset);
                file.seek(SeekFrom::Start(previous_position)).unwrap();
                // println!("Next main table: {:#0X?}", next_main_table);
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

        iterate_main_table(&mut file, &nds, nds.fnt_offset);

        // let sub_table: FNTSubtable = file.read_le().unwrap();
        // let table2: FNTSubtable = file.read_le().unwrap();

        // // println!("{:#0X?}", nds);
        // println!("first file ID: {:0X}", main_table.first_file_id);
        // println!("total dirs: {}", main_table.total_directories);
        // println!("{:#0X?}", sub_table);
        // println!("{:#0X?}", table2);

        // file.seek(SeekFrom::Start(nds.fnt_offset as u64 + *main_table.entries.get(1).unwrap() as u64)).unwrap();
        // let table: FNTSubtable = file.read_le().unwrap();
        // println!("{:#0X?}", sub_table);
    }

    // println!("{:#?}", args);
}
