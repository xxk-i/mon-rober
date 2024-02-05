use std::{fs::File, io::Write, path::PathBuf};

use binrw::binrw;

// http://problemkaputt.de/gbatek-ds-cartridge-nitrorom-and-nitroarc-file-systems.htm

#[derive(Debug)]
#[binrw]
#[br(magic = b"NARC")]
pub struct NARC {
    pub byte_order: u16,
    pub version: u16,
    pub file_size: u32,
    pub chunk_size: u16,
    pub chunk_count: u16,

    pub fat_block: FATBlock,
    pub fnt_block: FNTBlock,
    pub img_block: IMGBlock,
}

#[derive(Debug)]
#[binrw]
#[br(magic=b"BTAF")]
pub struct FATBlock {
    pub chunk_size: u32,
    pub num_files: u16,
    pub reserved: u16,
    
    #[br(count = num_files)]
    pub entries: Vec<crate::nds::FileAllocationTable>,
}

#[derive(Debug)]
#[binrw]
#[br(magic=b"BTNF")]
pub struct FNTBlock {
    pub chunk_size: u32,

    #[br(align_after=4)]
    pub fnt: crate::nds::FNTDirectoryMainTable,
}

#[derive(Debug)]
#[binrw]
#[br(magic=b"GMIF")]
pub struct IMGBlock {
    pub chunk_size: u32,

    #[br(count = chunk_size  - 8)]
    pub data: Vec<u8>,
}

impl NARC {
    // Extracts contents of a NARC archive to given path
    fn extract(&self, path: PathBuf) {
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        // Have to navigate to the start of the FNT inside of the FNTBlock manually since there
        // is no offset saved inside of the NARC header

        println!("FNT contains no names, labeling files manually");
        let mut file_index = 1;
        for entry in &self.fat_block.entries {
            // let mut buffer = vec![0u8; entry.end_address as usize - entry.start_address as usize];

            let buffer = &self.img_block.data[entry.start_address as usize..entry.end_address as usize];

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
}