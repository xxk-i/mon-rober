use std::io::Cursor;
use std::io::SeekFrom;

use binrw::BinRead;
use binrw::BinReaderExt;
use binrw::BinResult;
use binrw::Endian;
use binrw::io::Seek;
use binrw::io::Read;
use binrw::binrw;
use binrw::NullString;
use bitvec::order::Msb0;
use bitvec::view::BitView;

pub mod narc;
pub mod nclr;
pub mod ncgr;

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
pub struct FileAllocationTable {
    pub start_address: u32,
    pub end_address: u32,
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

// merged with info found at http://problemkaputt.de/gbatek-ds-files-2d-video.htm
#[derive(Debug)]
#[binrw]
pub struct GenericHeader {
    #[br(count=4)]
    magic: Vec<u8>,
    // constant: u32, // we swap this out
    byte_order: u16,
    version: u16,
    section_size: u32,
    header_size: u16,
    section_count: u16,
}

#[allow(dead_code)]
pub enum NDSCompressionType {
    LZ77(usize),
    LZ11(usize),
    Huffman,
    RLUncomp,
    None,
}

// tried to use DSDecomp's comment structure but like 20% sure its wrong
// used the original instead after figuring out how to actually read it
// http://problemkaputt.de/gbatek-lz-decompression-functions.htm
pub fn decompress_lz11(mut data: Cursor<&[u8]>, file_size: usize) -> Vec<u8> {
    let mut decompressed_data = Vec::new();
    let mut compressed_data = vec![0u8; file_size];
    data.read(compressed_data.as_mut_slice()).unwrap();

    let magic = &compressed_data[0..4];
    let size: usize = magic[1] as usize + ((magic[2] as usize) << 8) + ((magic[3] as usize) << 16);

    data.seek(SeekFrom::Start(4)).unwrap();

    while data.stream_position().unwrap() != file_size as u64 {
        let flags_byte = data.read_be::<u8>().unwrap();
        let flags = flags_byte.view_bits::<Msb0>();
        for i in 0..8u8 {
            let flag = flags.get(i as usize).unwrap();

            let mut len: usize;
            let mut disp: usize;

            if *flag {
                let reference = data.read_le::<u8>().unwrap();

                // check first 4 bits of reference
                match reference >> 4 {
                    0 => {
                        let len_msb = (reference << 4) as usize;
                        let next = data.read_le::<u8>().unwrap();
                        let len_lsb = (next >> 4) as usize;

                        len = len_msb;
                        len |= len_lsb;
                        len += 0x11;

                        disp = ((next & 0xF) as usize) << 8;
                    },

                    1 => {
                        let len_msb = ((reference & 0xF) as usize) << 12;
                        let len_csb = (data.read_le::<u8>().unwrap() as usize) << 4;
                        let next = data.read_le::<u8>().unwrap();
                        let len_lsb = (next >> 4) as usize;

                        len = len_msb;
                        len |= len_csb;
                        len |= len_lsb;
                        len += 0x111;
                        disp = ((next & 0xF) as usize) << 8;
                    },
                    _ => {
                        len = ((reference >> 4) + 0x1) as usize;
                        disp = ((reference & 0xF) as usize) << 8;
                    }
                }

                let disp_lsb = (data.read_le::<u8>().unwrap()) as usize;
                disp |= disp_lsb;

                let offset = decompressed_data.len() - 1 - disp as usize;
                for i in 0..len as usize {
                    decompressed_data.push(decompressed_data[offset + i]);
                }
                
            } else {
                if data.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(data.read_le::<u8>().unwrap());
                }
            }
        }
    }

    if decompressed_data.len() != size {
        println!("decompressed: {}, expected: {}", decompressed_data.len(), size);
    }
    decompressed_data
}

pub fn decompress_lz77(mut data: Cursor<&[u8]>, file_size: usize) -> Vec<u8> {
    let mut decompressed_data = Vec::new();
    let mut compressed_data = vec![0u8; file_size]; 
    data.read(compressed_data.as_mut_slice()).unwrap();

    let magic = &compressed_data[0..4];
    let size: u32 = magic[1] as u32 + ((magic[2] as u32) << 8) + ((magic[3] as u32) << 16);

    data.seek(SeekFrom::Start(4)).unwrap();

    while data.stream_position().unwrap() != file_size as u64 {
        let flag_byte: u8 = data.read_le().unwrap();

        // all bits are zero, no compression
        if flag_byte == 0 {
            for _ in 0..8 {
                if data.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(data.read_le::<u8>().unwrap());
                }
            }
            continue;
        }

        // poorly adapted from this best by far reference:
        // https://github.com/mtheall/decompress/blob/master/source/lzss.c
        let flags = flag_byte.view_bits::<Msb0>();
        for i in 0..8u8 {
            let flag = flags.get(i as usize).unwrap();
            if *flag {
                let reference: u16 = data.read_le().unwrap();
                let first: u8 = u8::try_from(reference << 8 >> 8).unwrap();
                let second: u8 = u8::try_from(reference >> 8).unwrap();
                let len: u32 = (((first & 0xF0)>>4)+3) as u32;
                let mut disp: u32 = (first & 0x0F) as u32;
                disp = disp << 8 | second as u32;

                let offset = decompressed_data.len() - 1 - disp as usize;

                for i in 0..len as usize {
                    decompressed_data.push(decompressed_data[offset + i]);
                }
            } else {
                if data.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(data.read_le::<u8>().unwrap());
                }
            }
        }
    }

    let padding_size = size as usize - decompressed_data.len();
    for _ in 0..padding_size {
        decompressed_data.push(0u8);
    }

    decompressed_data
}