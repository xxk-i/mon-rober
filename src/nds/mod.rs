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


// http://problemkaputt.de/gbatek-ds-cartridge-nitrorom-and-nitroarc-file-systems.htm

#[binrw]
#[derive(Debug)]
pub struct NDS {
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
    pub fnt_offset: u32,
    pub fnt_length: u32,

    // FILE ALLOCATION TABLE (FAT)
    pub fat_offset: u32,
    pub fat_length: u32,

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

#[derive(Debug)]
#[binrw]
// #[br(assert(total_directories < 4096, "total_directories is greater than 4096: {}", total_directories))]
pub struct FNTDirectoryMainTable {
    pub subtable_offset: u32,
    pub first_file_id: u16,
    pub directory_id: u16,
}

#[derive(Debug, BinRead)]
pub struct FNTSubtable {
    pub table_type: u8,

    // https://github.com/jam1garner/binrw/issues/73#issuecomment-935758313
    #[br(args(table_type), parse_with = parse_subtable)]
    pub data: SubtableEntry,
}

#[derive(Debug)]
pub enum SubtableEntry {
    FileEntry(String),

    SubdirectoryEntry(String, u16),

    Reserved,

    End
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