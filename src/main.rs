#![allow(arithmetic_overflow)]

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use binrw::io::Seek;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;

use bitvec::slice::BitRefIter;
use image::save_buffer;

mod nds;
use nds::NDS;
use nds::FNTDirectoryMainTable;
use nds::FNTSubtable;
use nds::SubtableEntry;
use nds::FileAllocationTable;
use nds::narc;
use nds::ncgr::NCGR;
use nds::nclr;
use nds::NDSCompressionType;

use bitvec::prelude::*;


const ASSET_DIR: &'static str = "assets";

struct GraphicsResource {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

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

fn unpack_nclr(nclr: &mut nclr::NCLR) -> Vec<(u8, u8, u8)> {
    // const r = (bgrInt & 0b11111) * 8;
    // const g = ((bgrInt >>> 5) & 0b11111) * 8;
    // const b = ((bgrInt >>> 10) & 0b11111) * 8;

    // conversion algorithm from orangeglo
    // translated from javascript
    // https://orangeglo.github.io/BGR555/

    let mut converted_colors = Vec::new();

    println!("{:0x}", nclr.ttlp.pallete_bit_depth);

    // for color in &nclr.ttlp.data {

    match nclr.ttlp.pallete_bit_depth {
        _ => {
            for i in 0..16 {
                // println!("{:0X}", color);
                let color = &nclr.ttlp.data[i];
                let r: u8 = ((color & 0b11111) * 8).try_into().unwrap();
                let g: u8 = (((color >> 5) & 0b11111) * 8).try_into().unwrap();
                let b: u8 = (((color >> 10) & 0b11111) * 8).try_into().unwrap();
                // println!("{:0X}{:0X}{:0X}", r + (r / 32), g + (g / 32), b + (b / 32));
                converted_colors.push((r,g,b));
            }
        },
        // 0xA004 => {
        //     for i in 0..16 {
        //         let byte1 = &nclr.ttlp.data[i];
        //         let byte2 = &nclr.ttlp.data[i + 1];
        //         let r: u8 = color 
        //     }
        // },
        // _ => println!("Unsupported palette bit depth: {:0X}", nclr.ttlp.pallete_bit_depth),
    }

    // let mut palette_buffer = Vec::new();

    // for i in 0..16 {
    //     palette_buffer.push(converted_colors[i].0);
    //     palette_buffer.push(converted_colors[i].1);
    //     palette_buffer.push(converted_colors[i].2);
    // }

    // save_buffer(&Path::new("K:/Developer/mon-rober/output2.png"), palette_buffer.as_slice(), 16, 1, image::ColorType::Rgb8).expect("Failed to save buffer");

    converted_colors
}

fn unpack_narc(mut file: File, path: PathBuf) {
    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    // NARC
    let narc: narc::NARC = file.read_le().expect("Failed to read NARC");

    // Have to navigate to the start of the FNT inside of the FNTBlock manually since there
    // is no offset saved inside of the NARC header

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
    
}

fn unpack_ncgr(mut cursor: Cursor<&[u8]>, palette: Vec<(u8, u8, u8)>, image_tile_width: u32, compression: NDSCompressionType) -> Result<GraphicsResource, Box<dyn Error>> {
    let ncgr: NCGR = match compression {
        NDSCompressionType::None => cursor.read_le().unwrap(),
        NDSCompressionType::LZ77(file_size) => {
            Cursor::new(decompress_lz77(cursor, file_size)).read_le().unwrap()
        },
        NDSCompressionType::LZ11(file_size) => {
            Cursor::new(decompress_lz11(cursor, file_size)).read_le().unwrap()
        },
        _ => unimplemented!()
    };

    let mut colors = Vec::new();

    println!("color depth: {}", ncgr.rahc.color_depth);

    // index is 4 bits long, so split byte and use each index
    for palette_index in ncgr.rahc.data {
        // println!("index: {}", palette_index);

        let lower_bits = palette_index & 0b00001111;
        let upper_bits = palette_index >> 4;

        // println!("lower_bits: {}", lower_bits);
        // println!("upper_bits: {}", upper_bits);

        colors.push(palette[lower_bits as usize]);
        colors.push(palette[upper_bits as usize]);
    }

    let mut new_pixels = Vec::new();

    // 512 bytes, 1024 colors
    // each tile is 32 bytes, 64 colors
    // 16 total tiles
    // group each 4 together to build each sprite

    // build 2x2 images

    let colors_per_byte = if ncgr.rahc.color_depth == 3 {
        2
    } else {
        1
    };

    // println!("tilewidthX: {}", ncgr.rahc.nTilesX);
    // println!("tilewidthY: {}", ncgr.rahc.nTilesY);

    // let tile_count = (ncgr.rahc.tile_data_size_bytes / ncgr.rahc.tile_dimension as u32) / colors_per_byte;
    let tile_count = (ncgr.rahc.tile_data_size_bytes / 16u32) / colors_per_byte;

    // this was constructed via black magic
    // it does a bunch of multiplication/addition to get pixel data
    // row by row across tiles based on the given width (image_tile_width)
    if ncgr.rahc.n_tiles_x != 0xFFFF {

    } else {
        for image_index in 0..(tile_count / image_tile_width) {
            for column in 0..8 {
                for tile_index in 0..image_tile_width {
                    for row in 0..8 {
                        let mut color_index = 0;
                        color_index += tile_index * 64;
                        color_index += column * 8;
                        color_index += image_index * 64 * image_tile_width;
                        color_index += row; 
                        let color = colors.get(color_index as usize).clone();
                        match color {
                            Some(c) => new_pixels.push(c),
                            None => new_pixels.push(&(255, 255, 255)),
                        }
                    }
                }
            }
        }
    }

    // for thing in 0..(96 * 96) {
    //     let color = colors.get(thing as usize).clone();
    //     match color {
    //         Some(c) => new_pixels.push(c),
    //         None => new_pixels.push(&(255,255,255)),
    //     }
    // }

    let mut buffer= Vec::new();

    for pixel in new_pixels {
        buffer.push(pixel.0);
        buffer.push(pixel.1);
        buffer.push(pixel.2);
    }


    // println!("tile count: {}\t width: {}", tile_count, image_tile_width);

    // save_buffer(&Path::new("K:/Developer/mon-rober/output2.png"), buffer.as_slice(), 8 * image_tile_width as u32, (tile_count / image_tile_width as u32) * 8 as u32, image::ColorType::Rgb8).expect("Failed to save buffer");
    Ok(GraphicsResource {
        width: 8 * image_tile_width as u32,
        height: (tile_count / image_tile_width as u32) * 8,
        data: buffer,
    })
}

// tried to use DSDecomp's comment structure but like 20% sure its wrong
// used the original instead after figuring out how to actually read it
// http://problemkaputt.de/gbatek-lz-decompression-functions.htm
fn decompress_lz11(mut file: Cursor<&[u8]>, file_size: usize) -> Vec<u8> {
    let mut decompressed_data = Vec::new();
    let mut compressed_data = vec![0u8; file_size];
    file.read(compressed_data.as_mut_slice()).unwrap();

    let magic = &compressed_data[0..4];
    let mut size: usize = magic[1] as usize + ((magic[2] as usize) << 8) + ((magic[3] as usize) << 16);

    file.seek(SeekFrom::Start(4)).unwrap();

    while file.stream_position().unwrap() != file_size as u64 {
        let flags_byte = file.read_be::<u8>().unwrap();
        let flags = flags_byte.view_bits::<Msb0>();
        for i in 0..8u8 {
            let flag = flags.get(i as usize).unwrap();

            let mut len: usize = 0;
            let mut disp: usize = 0;
            let mut disp_msb = 0;

            if *flag {
                let reference = file.read_le::<u8>().unwrap();

                // check first 4 bits of reference
                match reference >> 4 {
                    0 => {
                        let len_msb = (reference << 4) as usize;
                        let next = file.read_le::<u8>().unwrap();
                        let len_lsb = (next >> 4) as usize;

                        len = len_msb;
                        len |= len_lsb;
                        len += 0x11;

                        disp = ((next & 0xF) as usize) << 8;
                    },

                    1 => {
                        let len_msb = ((reference & 0xF) as usize) << 12;
                        let len_csb = (file.read_le::<u8>().unwrap() as usize) << 4;
                        let next = file.read_le::<u8>().unwrap();
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

                let disp_lsb = (file.read_le::<u8>().unwrap()) as usize;
                disp |= disp_lsb;

                let offset = decompressed_data.len() - 1 - disp as usize;
                for i in 0..len as usize {
                    decompressed_data.push(decompressed_data[offset + i]);
                }
                
            } else {
                if file.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(file.read_le::<u8>().unwrap());
                }
            }
        }
    }

    let mut output_path = std::env::current_dir().unwrap();
    output_path.push("decompressed.narc");
    println!("{:?}", output_path);
    let mut output = File::create(output_path).unwrap();
    output.write(&decompressed_data.as_slice()).unwrap();

    println!("decompressed: {}, expected: {}", decompressed_data.len(), size);
    
    // println!("returning data: {}", decompressed_data.len());
    // println!("{:0X?}", decompressed_data);
    decompressed_data
}

fn decompress_lz77(mut file: Cursor<&[u8]>, file_size: usize) -> Vec<u8> {
    let mut decompressed_data = Vec::new();
    let mut compressed_data = vec![0u8; file_size]; 
    file.read(compressed_data.as_mut_slice()).unwrap();

    let magic = &compressed_data[0..4];
    let size: u32 = magic[1] as u32 + ((magic[2] as u32) << 8) + ((magic[3] as u32) << 16);

    file.seek(SeekFrom::Start(4)).unwrap();

    while file.stream_position().unwrap() != file_size as u64 {
        let flag_byte: u8 = file.read_le().unwrap();

        // all bits are zero, no compression
        if flag_byte == 0 {
            for _ in 0..8 {
                if file.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(file.read_le::<u8>().unwrap());
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
                let reference: u16 = file.read_le().unwrap();
                let first: u8 = u8::try_from(reference << 8 >> 8).unwrap();
                let second: u8 = u8::try_from(reference >> 8).unwrap();
                let len: u32 = (((first & 0xF0)>>4)+3) as u32;
                let mut disp: u32 = (first & 0x0F) as u32;
                disp = disp << 8 | second as u32;

                // println!("{}", disp);
                // println!("{}", len);

                let offset = decompressed_data.len() - 1 - disp as usize;

                // println!("offset: {} - {}", offset, offset + len as usize);
                // println!("len: {}", decompressed_data.len());
                // println!("data: {:0X?}", decompressed_data);

                for i in 0..len as usize {
                    decompressed_data.push(decompressed_data[offset + i]);
                }
            } else {
                if file.stream_position().unwrap() != file_size as u64 {
                    decompressed_data.push(file.read_le::<u8>().unwrap());
                }
            }
        }
    }

    let padding_size = size as usize - decompressed_data.len();
    for _ in 0..padding_size {
        decompressed_data.push(0u8);
    }

    // let mut output_path = std::env::current_dir().unwrap();
    // output_path.push("decompressed.narc");
    // println!("{:?}", output_path);
    // let mut output = File::create(output_path).unwrap();
    // output.write(&decompressed_data.as_slice()).unwrap();

    decompressed_data
}

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

                let Ok(graphics_resource) = unpack_ncgr(cursor, palette, image_tile_width, NDSCompressionType::None) else {
                    continue;
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

fn extract_sprites_from_narc_with_palette(narc: nds::narc::NARC, path: String, image_tile_width: u32, palette_index: u32) {
    let current_dir = std::env::current_dir().unwrap();

    let mut output_path_base = current_dir.join(ASSET_DIR);
    output_path_base.push(path);

    let palette_allocation_info = &narc.fat_block.entries[palette_index as usize];

    let palette_data = &narc.img_block.data[palette_allocation_info.start_address as usize..palette_allocation_info.end_address as usize];

    let mut cursor = Cursor::new(palette_data);
    let mut palette_file: nclr::NCLR = cursor.read_le().unwrap();
    let palette = unpack_nclr(&mut palette_file);

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

                let graphics_resource: GraphicsResource = unpack_ncgr(cursor.clone(), palette.clone(), image_tile_width, NDSCompressionType::None).unwrap();

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
                let cursor = Cursor::new(&data[0..]);
                let graphics_resource = unpack_ncgr(cursor.clone(), palette.clone(), image_tile_width, NDSCompressionType::LZ77(data.len())).unwrap();

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
                let cursor = Cursor::new(&data[0..]);
                let graphics_resource = unpack_ncgr(cursor.clone(), palette.clone(), image_tile_width, NDSCompressionType::LZ11(data.len())).unwrap();

                output_path.push(file_num.to_string() + ".png");

                // println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
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
    // let mugshots = unpack_path.join("a/2/6/7");
    
    // let mugshots_narc: nds::narc::NARC = File::open(mugshots).unwrap().read_le().unwrap();

    // extract_sprites_from_narc_with_palette(mugshots_narc, String::from("mugshots"), 16, 72);

    // mon fulls
    let mon_fulls = unpack_path.join("a/0/0/4");

    let mon_fulls_narc: nds::narc::NARC = File::open(mon_fulls).unwrap().read_le().unwrap();

    extract_sprites_from_narc_with_palette(mon_fulls_narc, String::from("mon-fulls"),  12, 58);

    // clean-up unpacked rom dir
    std::fs::remove_dir_all(unpack_path).unwrap();

    return;
}
