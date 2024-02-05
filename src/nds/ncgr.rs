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
pub struct GraphicsResource {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

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
    pub n_tiles_y: u16,
    pub n_tiles_x: u16,
    pub color_depth: u32,
    pub padding1: u64,
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

impl NCGR {
    pub fn unpack_trainer_sprite(&self, palette: &Vec<(u8, u8, u8)>, image_tile_width: u32) -> Option<GraphicsResource> {
        let mut colors = Vec::new();
        let mut tmp = Vec::new();


        // index is 4 bits long, so split byte and use each index
        for palette_index in &self.rahc.data {
            let lower_bits = palette_index & 0b00001111;
            let upper_bits = palette_index >> 4;
            colors.push(palette[lower_bits as usize]);
            colors.push(palette[upper_bits as usize]);
            tmp.push(lower_bits);
            tmp.push(upper_bits);
        }

        let colors_per_byte = if self.rahc.color_depth == 3 {
            2
        } else {
            1
        };

        let tile_count = (self.rahc.tile_data_size_bytes / 16u32) / colors_per_byte;

        if self.rahc.n_tiles_x == 0xFFFF {
            return None;
        }

        let width = self.rahc.n_tiles_x as u32 * 8;
        let height = self.rahc.n_tiles_y as u32 * 8;
        
        let mut pixels = vec![vec![0u8; width as usize]; height as usize];
        let mut i = 0;
        for y in 0..(height / 8) {
        for x in 0..(width / 8) {
            for ty in 0..8 {
            for tx in 0..8 {
                let cy = y * 8 + ty;
                let cx = x * 8 + tx;
                pixels[cy as usize][cx as usize] = tmp.get(i).unwrap().clone();
                i += 1;
            }
            }
        }
        }

        let mut buffer= Vec::new();

        for i in pixels {
            for pixel in i {
                let color = palette.get(pixel as usize).unwrap();
                buffer.push(color.0);
                buffer.push(color.1);
                buffer.push(color.2);
            }
        }

        Some(GraphicsResource { width, height, data: buffer })
    }

    pub fn unpack_mon_full_sprite(&self, palette: Vec<(u8, u8, u8)>, image_tile_width: u32) -> Option<GraphicsResource> {
        let mut colors = Vec::new();
        let mut tmp = Vec::new();


        // index is 4 bits long, so split byte and use each index
        for palette_index in &self.rahc.data {
            let lower_bits = palette_index & 0b00001111;
            let upper_bits = palette_index >> 4;
            colors.push(palette[lower_bits as usize]);
            colors.push(palette[upper_bits as usize]);
            tmp.push(lower_bits);
            tmp.push(upper_bits);
        }


        // 512 bytes, 1024 colors
        // each tile is 32 bytes, 64 colors
        // 16 total tiles
        // group each 4 together to build each sprite

        // build 2x2 images

        let colors_per_byte = if self.rahc.color_depth == 3 {
            2
        } else {
            1
        };
        let tile_count = (self.rahc.tile_data_size_bytes / 16u32) / colors_per_byte;


        let width = self.rahc.n_tiles_x as u32 * 8;
        let height = self.rahc.n_tiles_y as u32 * 8;

        if width != 96 {
            return None;
        }

        // untile the pixel data (8x8 tiles)
        let mut pixels = [[0u8; 96]; 96];
        let mut i = 0;
        for y in 0..(height / 8) {
        for x in 0..(width / 8) {
            for ty in 0..8 {
            for tx in 0..8 {
                let cy = y * 8 + ty;
                let cx = x * 8 + tx;
                pixels[cy as usize][cx as usize] = tmp.get(i).unwrap().clone();
                i += 1;
            }
            }
        }
        }

        let mut sorted_pixels = [[0u8; 96]; 96];

        // move groups of 32x8 tiles from one index to another
        fn move_pixels(src: &mut [[u8;96]; 96], dst: &mut [[u8;96]; 96], x: usize, y: usize) {
            let x = x - 1;
            let y = y - 1;

            let row = y / 3;
            let column = y % 3;
            let row2 = x / 3;
            let column2 = x % 3;

            for i in 0..8 {
                for j in 0..32 {
                    dst[row * 8 + i][column * 32 + j] = src[row2 * 8 + i][column2 * 32 + j];
                }
            }
        }

        // this avoids parsing NCER which does god knows to the tiles and replaces them
        // INSTEAD, let's just move the tiles ourselves!! oh good god!
        move_pixels(&mut pixels, &mut sorted_pixels, 1, 1  );
        move_pixels(&mut pixels, &mut sorted_pixels, 2, 2  );
        move_pixels(&mut pixels, &mut sorted_pixels, 3, 4  );
        move_pixels(&mut pixels, &mut sorted_pixels, 4, 5  );
        move_pixels(&mut pixels, &mut sorted_pixels, 5, 7  );
        move_pixels(&mut pixels, &mut sorted_pixels, 6, 8  );
        move_pixels(&mut pixels, &mut sorted_pixels, 7, 10 );
        move_pixels(&mut pixels, &mut sorted_pixels, 8, 11 );
        move_pixels(&mut pixels, &mut sorted_pixels, 9, 13 );
        move_pixels(&mut pixels, &mut sorted_pixels, 10, 14);
        move_pixels(&mut pixels, &mut sorted_pixels, 11, 16);
        move_pixels(&mut pixels, &mut sorted_pixels, 12, 17);
        move_pixels(&mut pixels, &mut sorted_pixels, 13, 19);
        move_pixels(&mut pixels, &mut sorted_pixels, 14, 20);
        move_pixels(&mut pixels, &mut sorted_pixels, 15, 22);
        move_pixels(&mut pixels, &mut sorted_pixels, 16, 23);
        move_pixels(&mut pixels, &mut sorted_pixels, 17, 3 );
        move_pixels(&mut pixels, &mut sorted_pixels, 18, 6 );
        move_pixels(&mut pixels, &mut sorted_pixels, 19, 9 );
        move_pixels(&mut pixels, &mut sorted_pixels, 20, 12);
        move_pixels(&mut pixels, &mut sorted_pixels, 21, 15);
        move_pixels(&mut pixels, &mut sorted_pixels, 22, 18);
        move_pixels(&mut pixels, &mut sorted_pixels, 23, 21);
        move_pixels(&mut pixels, &mut sorted_pixels, 24, 24);
        move_pixels(&mut pixels, &mut sorted_pixels, 25, 25);
        move_pixels(&mut pixels, &mut sorted_pixels, 26, 26);
        move_pixels(&mut pixels, &mut sorted_pixels, 27, 28);
        move_pixels(&mut pixels, &mut sorted_pixels, 28, 29);
        move_pixels(&mut pixels, &mut sorted_pixels, 29, 31);
        move_pixels(&mut pixels, &mut sorted_pixels, 30, 32);
        move_pixels(&mut pixels, &mut sorted_pixels, 31, 35);
        move_pixels(&mut pixels, &mut sorted_pixels, 32, 34);
        move_pixels(&mut pixels, &mut sorted_pixels, 33, 27);
        move_pixels(&mut pixels, &mut sorted_pixels, 34, 30);
        move_pixels(&mut pixels, &mut sorted_pixels, 35, 33);
        move_pixels(&mut pixels, &mut sorted_pixels, 36, 36);

        let mut buffer= Vec::new();

        for i in sorted_pixels {
            for pixel in i {
                let color = palette.get(pixel as usize).unwrap();
                buffer.push(color.0);
                buffer.push(color.1);
                buffer.push(color.2);
            }
        }

        Some(GraphicsResource { width, height, data: buffer })
    }
}