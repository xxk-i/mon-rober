#![allow(arithmetic_overflow)]

use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;

use binrw::io::Seek;
use binrw::io::SeekFrom;
use binrw::BinReaderExt;

mod nds;
use nds::nclr::NCLR;
use nds::NDS;
use nds::FNTDirectoryMainTable;
use nds::FNTSubtable;
use nds::SubtableEntry;
use nds::FileAllocationTable;
use nds::ncgr::NCGR;

const ASSET_DIR: &'static str = "assets";

// "oriented" == front or back
struct MonOrientedSpriteSet {
    male: NCGR,
    female: Option<NCGR>,
    male_parts: NCGR,
    female_parts: Option<NCGR>,
    // ncer: Vec<u8>,
    // nanr: Vec<u8>,
    // nmcr: Vec<u8>,
    // nmar: Vec<u8>,
    // unknown: Vec<u8>,
}

struct MonSpritesEntry {
    front: MonOrientedSpriteSet,
    back: MonOrientedSpriteSet,
    normal_palette: NCLR,
    shiny_palette: NCLR,
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

fn unpack_rom(mut file: File) {
    let mut filelist = Vec::new();

    let current_dir = std::env::current_dir().expect("Failed to get current directory");

    let nds: NDS = file.read_le().expect("Failed to read file");
    file.seek(SeekFrom::Start(nds.fnt_offset as u64)).expect("Failed to seek to FNT");
    let main_table: FNTDirectoryMainTable =  file.read_le().unwrap();

    // collects all FNT entries
    iterate_main_table(&mut file, nds.fnt_offset, nds.fnt_offset, PathBuf::from("unpacked/"), &mut filelist);

    // Jump to first file ID in FAT... don't really know what the previous entries are
    file.seek(SeekFrom::Start(nds.fat_offset as u64 + main_table.first_file_id as u64 * 8)).expect("Failed to seek to FAT");

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

fn extract_mon_icons(narc: nds::narc::NARC, output_folder: String) {
    let current_dir = std::env::current_dir().unwrap();

    let mut output_path_base = current_dir.join(ASSET_DIR);
    output_path_base.push(output_folder);

    // there are 2 palettes at the top that are supposedly different
    // although they seem to produce the same result
    let palette_nclr: NCLR = narc.get_decompressed_entry(0).read_le().unwrap();
    let palette = palette_nclr.unpack();

    let mut i = 8;
    while i <= 1508 {
        let icon: NCGR = narc.get_decompressed_entry(i).read_le().unwrap();

        if let Some(graphics_resource) = icon.unpack_mon_icon(&palette) {
            graphics_resource.write(output_path_base.join(i.to_string() + ".png"));
        }

        i += 2;
    }
}

fn extract_mon_fulls(narc: nds::narc::NARC, output_folder: String) {
    let current_dir = std::env::current_dir().unwrap();

    let mut output_path_base = current_dir.join(ASSET_DIR);
    output_path_base.push(output_folder);

    // 751 pokemon, 20 files per
    for i in 0..751 {
        let output_path = output_path_base.join(i.to_string());

        let mon_sprites_entry = MonSpritesEntry {
            front: MonOrientedSpriteSet {
                male: narc.get_decompressed_entry(i * 20 + 0 as usize).read_le().unwrap(),
                female: {
                    let mut data = narc.get_decompressed_entry(i * 20 + 1 as usize);
                    if data.get_ref().len() == 0 {
                        None
                    } else {
                        data.read_le().unwrap()
                    }
                },
                male_parts: narc.get_decompressed_entry(i * 20 + 2 as usize).read_le().unwrap(),
                female_parts: { 
                    let mut data = narc.get_decompressed_entry(i * 20 + 3 as usize);
                    if data.get_ref().len() == 0 {
                        None
                    } else {
                        data.read_le().unwrap()
                    }
                }
            },
            back: MonOrientedSpriteSet {
                male: narc.get_decompressed_entry(i * 20 + 9 as usize).read_le().unwrap(),
                female: {
                    let mut data = narc.get_decompressed_entry(i * 20 + 10 as usize);
                    if data.get_ref().len() == 0 {
                        None
                    } else {
                        data.read_le().unwrap()
                    }
                },
                male_parts: narc.get_decompressed_entry(i * 20 + 11 as usize).read_le().unwrap(),
                female_parts: { 
                    let mut data = narc.get_decompressed_entry(i * 20 + 12 as usize);
                    if data.get_ref().len() == 0 {
                        None
                    } else {
                        data.read_le().unwrap()
                    }
                }
            },
            normal_palette: narc.get_decompressed_entry(i * 20 + 18 as usize).read_le().unwrap(),
            shiny_palette: narc.get_decompressed_entry(i * 20 + 19 as usize).read_le().unwrap(),
        };

        let normal_path = output_path.join("normal");
        let shiny_path = output_path.join("shiny");

        // MALE
        if let Some(graphics_resource) = mon_sprites_entry.front.male.unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("male_front.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.front.male_parts.unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("male_front_parts.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.male.unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("male_back.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.male_parts.unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("male_back_parts.png"));
        }

            // SHINY
        if let Some(graphics_resource) = mon_sprites_entry.front.male.unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("male_front.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.front.male_parts.unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("male_front_parts.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.male.unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("male_back.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.male_parts.unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("male_back_parts.png"));
        }
            // SHINY
        // MALE

        if mon_sprites_entry.front.female.is_none() {
            continue;
        }

        // FEMALE
        if let Some(graphics_resource) = mon_sprites_entry.front.female.as_ref().unwrap().unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("female_front.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.front.female_parts.as_ref().unwrap().unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("female_front_parts.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.female.as_ref().unwrap().unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("female_back.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.female_parts.as_ref().unwrap().unpack_mon_full_sprite(mon_sprites_entry.normal_palette.unpack()) {
            graphics_resource.write(normal_path.join("female_back_parts.png"));
        }

            // SHINY
        if let Some(graphics_resource) = mon_sprites_entry.front.female.unwrap().unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("female_front.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.front.female_parts.unwrap().unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("female_front_parts.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.female.unwrap().unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("female_back.png"));
        }

        if let Some(graphics_resource) = mon_sprites_entry.back.female_parts.unwrap().unpack_mon_full_sprite(mon_sprites_entry.shiny_palette.unpack()) {
            graphics_resource.write(shiny_path.join("female_back_parts.png"));
        }
            // SHINY
        // FEMALE
    }
}

fn extract_trainers(narc: nds::narc::NARC, output_folder: String) {
    let current_dir = std::env::current_dir().unwrap();

    let mut output_path_base = current_dir.join(ASSET_DIR);
    output_path_base.push(output_folder);

    let mut palette_offset = 0;

    // everyone before iris
    for i in 0..13 {
        let palette: NCLR = narc.get_decompressed_entry(i + 53).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join(i.to_string() + ".png"));
        }
    }

    // iris has 2 because legs
    for i in 13..15 {
        // shared palette (probably)
        let palette: NCLR = narc.get_decompressed_entry(13 + 53).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join(i.to_string() + ".png"));
        }
    }

    palette_offset += 1;

    // guy after iris
    for i in 15..18 {
        let palette: NCLR = narc.get_decompressed_entry(i - palette_offset + 53).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join(i.to_string() + ".png"));
        }
    }

    // gap for medals
    // for i in 18..23 {
    //     let palette: NCLR = narc.get_decompressed_entry(i - palette_offset + 53).read_le().unwrap();
    //     let trainer: NCGR = narc.get_decompressed_entry(i).read_le().unwrap();

    //     if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
    //         graphics_resource.write(output_path_base.join(i.to_string() + ".png"));
    //     }
    // }

    for i in 0..2 {
        let palette: NCLR = narc.get_decompressed_entry(71).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i + 45).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join((i + 45).to_string() + ".png"));
        }
    }

    for i in 0..3 {
        let palette: NCLR = narc.get_decompressed_entry(72).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i + 47).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join((i + 47).to_string() + ".png"));
        }
    }

    for i in 0..3 {
        let palette: NCLR = narc.get_decompressed_entry(74).read_le().unwrap();
        let trainer: NCGR = narc.get_decompressed_entry(i + 50).read_le().unwrap();

        if let Some(graphics_resource) = trainer.unpack_trainer_sprite(&palette.unpack()) {
            graphics_resource.write(output_path_base.join((i + 50).to_string() + ".png"));
        }
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

    // dump rom
    unpack_rom(file);

    let unpack_path = std::env::current_dir().unwrap().join("unpacked");

    // mon icons
    println!("Dumping mon icons...");
    let mon_icons = unpack_path.join("a/0/0/7");
    let mon_narc: nds::narc::NARC = File::open(mon_icons).unwrap().read_le().unwrap();
    extract_mon_icons(mon_narc, String::from("mon_icons"));

    // trainer mugshots
    println!("Dumping trainer mugshots...");
    let mugshots = unpack_path.join("a/2/6/7");
    let mugshots_narc: nds::narc::NARC = File::open(mugshots).unwrap().read_le().unwrap();
    extract_trainers(mugshots_narc, String::from("mugshots"));

    // mon fulls
    println!("Dumping mon fulls...");
    let mon_fulls = unpack_path.join("a/0/0/4");
    let mon_fulls_narc: nds::narc::NARC = File::open(mon_fulls).unwrap().read_le().unwrap();
    extract_mon_fulls(mon_fulls_narc, String::from("mon-fulls"));

    // clean-up unpacked rom dir
    println!("Done! Cleaning up temporary dir...");
    std::fs::remove_dir_all(unpack_path).unwrap();

    return;
}