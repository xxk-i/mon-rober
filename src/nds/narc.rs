use binrw::{BinRead, binrw};

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