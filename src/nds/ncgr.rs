// Nintendo Character Graphic Resource

// struct RGCN {
//    char magic[4];
//    u16 byteOrder;
//    u16 version;
//    u32 totalFilesize;
//    u16 rahcOffset;
//    u16 chunkCount;
// };

// struct RAHC {
//    char magic[4];
//    u32 chunkSize;
//    u16 tileDataSizeKilobytes;
//    u16 padding1;
//    u32 colorDepth;
//    u64 padding2;
//    u32 tileDataSizeBytes;
//    u32 tileDataOffset;
//    u8 data[tileDataSizeBytes];
// };

// struct SOPC {
//    char magic[4];
//    u32 sectionSize;
//    u32 padding1;
//    u16 tileSize;
//    u16 tileCount;
// };

use std::io::SeekFrom;

use binrw::binrw;

#[derive(Debug)]
#[binrw]
pub struct NCGR {
    pub header: crate::nds::GenericHeader,
    #[br(seek_before(SeekFrom::Start(header.header_size as u64)))]
    pub rahc: RAHC,
}

#[derive(Debug)]
#[binrw]
pub struct RAHC {
    #[br(count=4)]
    pub magic: Vec<u8>,
    pub chunk_size: u32,
    pub tile_data_size_kb: u16,
    pub tile_dimension: u16,
    pub color_depth: u32,
    pub padding2: u64,
    pub tile_data_size_bytes: u32,
    pub tile_data_offset: u32,
    #[br(count=tile_data_size_bytes)]
    pub data: Vec<u8>
}


#[derive(Debug)]
#[binrw]
struct SOPC {
    #[br(count=4)]
    magic: Vec<u8>,
    chunk_size: u32,
    padding1: u32,
    tile_size: u16,
    tile_count: u16,
}