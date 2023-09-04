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
use walkdir::WalkDir;

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

fn unpack_rom(mut file: File, path: PathBuf) {
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

    // for color in &nclr.ttlp.data {
    for i in 0..16 {
        // println!("{:0X}", color);
        let color = &nclr.ttlp.data[i];
        let r: u8 = ((color & 0b11111) * 8).try_into().unwrap();
        let g: u8 = (((color >> 5) & 0b11111) * 8).try_into().unwrap();
        let b: u8 = (((color >> 10) & 0b11111) * 8).try_into().unwrap();
        // println!("{:0X}{:0X}{:0X}", r + (r / 32), g + (g / 32), b + (b / 32));
        converted_colors.push((r,g,b));
    }

    // converted_colors.push((255,0,0));
    // }

    converted_colors
}

fn unpack_narc(mut file: File, path: PathBuf) {
    let mut filelist = Vec::new();

    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    // NARC
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

fn unpack_ncgr(mut cursor: Cursor<&[u8]>, palette: Vec<(u8, u8, u8)>) -> Result<GraphicsResource, Box<dyn Error>> {
    let ncgr: NCGR = cursor.read_le().unwrap();

    let mut colors = Vec::new();

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

    let tile_count = (ncgr.rahc.tile_data_size_bytes / ncgr.rahc.tile_dimension as u32) / colors_per_byte;

    let image_tile_width = 2;

    // this was constructed via black magic
    // it does a bunch of multiplication/addition to get pixel data
    // row by row across tiles based on the given width (image_tile_width)
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

    let mut buffer= Vec::new();

    for pixel in new_pixels {
        buffer.push(pixel.0);
        buffer.push(pixel.1);
        buffer.push(pixel.2);
    }

    // save_buffer(&Path::new("K:/Developer/mon-rober/output2.png"), buffer.as_slice(), 8 * image_tile_width as u32, (tile_count / image_tile_width as u32) * 8 as u32, image::ColorType::Rgb8).expect("Failed to save buffer");
    Ok(GraphicsResource {
        width: 8 * image_tile_width as u32,
        height: (tile_count / image_tile_width as u32) * 8,
        data: buffer,
    })
}

fn extract_sprites_from_narc(mut file: File, path: &Path) -> Result<(), Box<dyn Error>>{
    let narc: narc::NARC = file.read_le()?;

    let mut palette_file: Option<nclr::NCLR> = None;

    let current_dir = std::env::current_dir().unwrap();
    
    let mut output_path_base= PathBuf::new();
    output_path_base.push(current_dir);
    output_path_base.push("output/");
    output_path_base.push(path.clone());

    let mut file_num = 0;

    // these NARC can contain a tree for filenames but for this game, they don't <3
    // skip to FAT and just dump data to generic filenames
    for entry in narc.fat_block.entries {
        let data = &narc.img_block.data[entry.start_address as usize..entry.end_address as usize];

        if data.len() < 4 {
            continue;
        }

        let mut output_path = output_path_base.clone();

        // unwrap or continue
        let Ok(magic) = String::from_utf8(data[0..4].to_vec()) else {
            continue;
        };

        match magic.as_str() {
            "RLCN" => {
                if palette_file.is_some() {
                    continue;
                } else {
                    let mut cursor = Cursor::new(data);
                    palette_file = Some(cursor.read_le().unwrap());
                }
            },

            "RGCN" => {
                if palette_file.is_none() {
                    println!("Graphics resource found but missing palette; skipping...");
                    continue;
                }

                let mut cursor = Cursor::new(data);
                let palette = unpack_nclr(palette_file.as_mut().unwrap());

                let Ok(graphics_resource) = unpack_ncgr(cursor, palette) else {
                    continue;
                };

                output_path.push(file_num.to_string() + ".png");

                println!("Writing sprite file: {:?}", output_path);

                if graphics_resource.height != 0 {
                    std::fs::create_dir_all(&output_path_base).expect("Failed to create output path(s)");
                    save_buffer(&output_path, &graphics_resource.data, graphics_resource.width, graphics_resource.height, image::ColorType::Rgb8).expect("Failed to save buffer");
                }    
            },

            _ => println!("Unknown type, skipping"),
        }

        file_num += 1;
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Usage: mon-rober <path>");
    }

    let path = PathBuf::from(args.get(1).unwrap());

    for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()).filter(|e| e.metadata().unwrap().is_file()) {
        let mut magic = vec![0u8; 4];
        let mut file = File::open(entry.path()).expect("Failed to open file in input directory");
        file.read_exact(magic.as_mut_slice()).expect("Failed to read magic of input file... is file empty?");

        let Ok(magic) = String::from_utf8(magic) else {
            continue
        };

        if magic.as_str().eq("NARC") {
            file.seek(SeekFrom::Start(0u64)).unwrap();
            match extract_sprites_from_narc(file, entry.path()) {
                Ok(()) => {},
                Err(e) => eprintln!("Failure extracting sprite: {:?}, {}", entry.path(), e),
            }
        }
    }
    
    return;

    let mut file = File::open(&path).expect("Failed to open input file");

    let mut magic = vec![0u8; 4];
    file.read_exact(magic.as_mut_slice()).expect("Failed to read magic of input file... is file empty?");
    let magic = String::from_utf8(magic).unwrap();

    file.seek(SeekFrom::Start(0u64)).unwrap();

    match magic.as_str() {
        // this is just a game title not a MAGIC but I don't want to check extension <3
        "POKE" => {
            println!("Unpacking Main ROM");
            unpack_rom(file, path);
        },

        "NARC" =>  {
            println!("Unpacking Nintendo Archive");
            unpack_narc(file, path);
        },
        
        "RGCN" => {
            println!("Unpacking Nitro Character Graphics Resource");
            // unpack_ncgr(file, path);
        },

        "RLCN" => {
            println!("Unpacking Nitro Color Resource");
            // unpack_nclr(file);
        }

        _ => println!("Unrecognized file")
    };
        
}
