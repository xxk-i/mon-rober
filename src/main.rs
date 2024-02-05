#![allow(arithmetic_overflow)]

use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use binrw::io::Seek;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;

use image::save_buffer;

mod nds;
use nds::NDS;
use nds::FNTDirectoryMainTable;
use nds::FNTSubtable;
use nds::SubtableEntry;
use nds::FileAllocationTable;
use nds::ncgr::NCGR;
use nds::nclr;
use nds::NDSCompressionType;

const ASSET_DIR: &'static str = "assets";


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

fn unpack_rom(mut file: File, path: &PathBuf) {
    let mut filelist = Vec::new();

    let current_dir = std::env::current_dir().expect("Failed to get current directory");

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

/*
fn extract_sprites_from_narc(narc: nds::narc::NARC, path: String, image_tile_width: u32) -> Result<(), Box<dyn Error>> {
    let mut palette_file: Option<nclr::NCLR> = None;

    let current_dir = std::env::current_dir().unwrap();
    
    let mut output_path_base= current_dir.join(ASSET_DIR);
    output_path_base.push(path);

    let mut file_num = 0;

    // these NARC can contain a tree for filenames but for this game, they don't <3
    // skip to FAT and just dump data to generic filenames
    for entry in narc.fat_block.entries {
        let data = &narc.img_block.data[entry.start_address as usize..entry.end_address as usize];

        if data.len() < 4 {
            continue;
        }

        let mut output_path = output_path_base.clone();

        let magic = &data[0..4];

        match magic {
            b"RLCN" => {
                if palette_file.is_some() {
                    continue;
                } else {
                    let mut cursor = Cursor::new(data);
                    palette_file = Some(cursor.read_le().unwrap());
                }
            },

            b"RGCN" => {
                if palette_file.is_none() {
                    println!("Graphics resource found but missing palette; skipping...");
                    continue;
                }

                let cursor = Cursor::new(data);
                let palette = unpack_nclr(palette_file.as_mut().unwrap());

                let graphics_resource = match unpack_ncgr(cursor, palette, image_tile_width, NDSCompressionType::None)  {
                    Some(g) => g,
                    None => break,
                };

                output_path.push(file_num.to_string() + ".png");

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
                    std::fs::create_dir_all(&output_path.parent().unwrap()).expect("Failed to create output path(s)");
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).expect("Failed to save buffer");
                }    
            },

            // compressed sprite (only need to support lz77 right now)
            [0x10, _, _, _] => {
                let cursor = Cursor::new(&data[0..]);
                let mut palette_file: nclr::NCLR = File::open("K:\\Developer\\mon-rober\\7_72.RLCN").unwrap().read_le().unwrap();
                let palette = unpack_nclr(&mut palette_file);

                let graphics_resource = unpack_ncgr(cursor, palette, image_tile_width, NDSCompressionType::LZ77(data.len())).unwrap();

                output_path.push(file_num.to_string() + ".png");

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
                    std::fs::create_dir_all(&output_path.parent().unwrap()).expect("Failed to create output path(s)");
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).expect("Failed to save buffer");
                }    
            }

            _ => println!("Unknown type, skipping ({:?})", magic),
        }

        file_num += 1;
    }

    Ok(())
}
*/

fn extract_sprites_from_narc_with_palette(narc: nds::narc::NARC, path: String, image_tile_width: u32, palette_index: u32) {
    let current_dir = std::env::current_dir().unwrap();

    let mut output_path_base = current_dir.join(ASSET_DIR);
    output_path_base.push(path);

    let palette_allocation_info = &narc.fat_block.entries[palette_index as usize];

    let palette_data = &narc.img_block.data[palette_allocation_info.start_address as usize..palette_allocation_info.end_address as usize];

    let mut cursor = Cursor::new(palette_data);
    let mut palette_file: nclr::NCLR = cursor.read_le().unwrap();
    let palette = palette_file.unpack();

    let mut file_num = 0;
    for entry in narc.fat_block.entries {
        let data = &narc.img_block.data[entry.start_address as usize..entry.end_address as usize];

        if data.len() < 4 {
            continue;
        }

        let mut output_path = output_path_base.clone();

        let magic = &data[0..4];

        match magic {
            b"RGCN" => {
                output_path.push(file_num.to_string() + ".png");

                let ncgr: NCGR = cursor.read_le().unwrap();
                let graphics_resource = ncgr.unpack_trainer_sprite(&palette, image_tile_width).unwrap();
                // let graphics_resource: GraphicsResource = unpack_ncgr(cursor.clone(), palette.clone(), image_tile_width, NDSCompressionType::None).unwrap();

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
                    std::fs::create_dir_all(&output_path.parent().unwrap()).unwrap();
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).unwrap();
                }
            }

            // size is zero, i guess. skip
            [0x10, 0, 0, 0] => {}

            // compressed, LZ77 variant
            [0x10, _, _, _] => {
                let ncgr: NCGR = Cursor::new(nds::decompress_lz77(Cursor::new(&data[0..]) , data.len())).read_le().unwrap();
                let graphics_resource = ncgr.unpack_trainer_sprite(&palette, image_tile_width).unwrap();

                output_path.push(file_num.to_string() + ".png");

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
                    std::fs::create_dir_all(&output_path.parent().unwrap()).expect("Failed to create output path(s)");
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).expect("Failed to save buffer");
                }
            }

            // size is zero again, still guessing. skip
            [0x11, 0, 0, 0] => {}

            // compressed, LZ11 variant
            [0x11, _, _, _] => {
                let ncgr: NCGR = Cursor::new(nds::decompress_lz11(Cursor::new(&data[0..]) , data.len())).read_le().unwrap();
                let graphics_resource = ncgr.unpack_trainer_sprite(&palette, image_tile_width).unwrap();

                output_path.push(file_num.to_string() + ".png");

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0  && graphics_resource.data.len() != 0 {
                    std::fs::create_dir_all(&output_path.parent().unwrap()).expect("Failed to create output path(s)");
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).expect("Failed to save buffer");
                }
            }

            _ => {}
        }

        file_num += 1;
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: mon-rober <ROM>");
        return;
    }

    let path = PathBuf::from(args.get(1).unwrap());
    let file = File::open(&path).unwrap();

    unpack_rom(file, &path);

    let unpack_path = std::env::current_dir().unwrap().join("unpacked");

    // mon icons
    // let mon_icons = unpack_path.join("a/0/0/7");

    // let mon_narc: nds::narc::NARC = File::open(mon_icons).unwrap().read_le().unwrap();

    // extract_sprites_from_narc(mon_narc, String::from("mon-icons"), 4).unwrap();

    // // trainer mugshots
    let mugshots = unpack_path.join("a/2/6/7");
    
    let mugshots_narc: nds::narc::NARC = File::open(mugshots).unwrap().read_le().unwrap();

    extract_sprites_from_narc_with_palette(mugshots_narc, String::from("mugshots"), 16, 72);

    // mon fulls
    // let mon_fulls = unpack_path.join("a/0/0/4");

    // let mon_fulls_narc: nds::narc::NARC = File::open(mon_fulls).unwrap().read_le().unwrap();

    // extract_sprites_from_narc_with_palette(mon_fulls_narc, String::from("mon-fulls"), 8, 58);


    // clean-up unpacked rom dir
    // std::fs::remove_dir_all(unpack_path).unwrap();

    return;
}