// https://www.romhacking.net/documents/%5B469%5Dnds_formats.htm#NCLR

use std::io::SeekFrom;
use binrw::binrw;

#[derive(Debug)]
#[binrw]
pub struct NCLR {
    pub header: crate::nds::GenericHeader,
    #[br(seek_before(SeekFrom::Start(header.header_size as u64)))]
    pub ttlp: TTLP,
}

#[derive(Debug)]
#[binrw]
pub struct TTLP {
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

impl NCLR {
    // Returns a Vector of (R, G, B) for each color in NCLR palette
    pub fn unpack(&self) -> Vec<(u8, u8, u8)> {
        // const r = (bgrInt & 0b11111) * 8;
        // const g = ((bgrInt >>> 5) & 0b11111) * 8;
        // const b = ((bgrInt >>> 10) & 0b11111) * 8;

        // conversion algorithm from orangeglo
        // translated from javascript
        // https://orangeglo.github.io/BGR555/

        let mut converted_colors = Vec::new();

        match self.ttlp.pallete_bit_depth {
            _ => {
                for i in 0..16 {
                    // println!("{:0X}", color);
                    let color = &self.ttlp.data[i];
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

        converted_colors
    }
}