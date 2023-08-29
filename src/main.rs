use std::env;
use std::fs::File;
use std::path::PathBuf;

use binrw::BinReaderExt;
use binrw::binrw;
use binrw::NullString;

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
    fnt_legnth: u32,

    // FILE ALLOCATION TABLE (FAT)
    fat_offset: u32,
    fat_lenth: u32,

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

#[binrw]
struct FNTDirectoryMainTable {
    subtable_offset: u32,
    first_file_id: u16,

    total_directories: u16,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: mon-rober <path>");
    }

    else {
        let path = PathBuf::from(args.get(1).unwrap());
        let nds: NDS = File::open(path).expect("Failed to open file").read_le().expect("Failed to read file");
        println!("{:#0X?}", nds);
    }

    println!("{:#?}", args);
}
