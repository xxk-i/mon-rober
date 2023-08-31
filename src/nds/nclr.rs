// https://www.romhacking.net/documents/%5B469%5Dnds_formats.htm#NCLR

use binrw::binrw;

#[derive(Debug)]
#[binrw]
pub struct NCLR {
    pub header: crate::nds::GenericHeader,
    #[br(count=4)]
    pub magic: Vec<u8>,
    pub section_size: u32,
    pub pallete_bit_depth: u32,
    pub padding: u32,
    pub pallete_data_size: u32,
    pub colors_per_pallete: u32,
    #[br(count=pallete_data_size/2)]
    pub data: Vec<u16>
}