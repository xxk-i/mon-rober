struct RGCN {
   char magic[4];
   u16 byteOrder;
   u16 version;
   u32 totalFilesize;
   u16 rahcOffset;
   u16 chunkCount;
};

struct RAHC {
   char magic[4];
   u32 chunkSize;
   u16 tileDataSizeKilobytes;
   u16 padding1;
   u32 colorDepth;
   u64 padding2;
   u32 tileDataSizeBytes;
   u32 tileDataOffset;
   u8 data[tileDataSizeBytes];
};

struct SOPC {
   char magic[4];
   u32 sectionSize;
   u32 padding1;
   u16 tileSize;
   u16 tileCount;
};

RGCN rgcn @ 0x0;
RAHC rahc @ rgcn.rahcOffset;
SOPC sopc @ rgcn.rahcOffset + rahc.chunkSize;