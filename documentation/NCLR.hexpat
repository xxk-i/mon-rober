struct TTLP {
   char magic[4];
   u32 chunkSize;
   u32 colorDepth;
   u32 padding1;
   u32 paletteDataSize;
   u32 offset;
   u8 data[paletteDataSize];
};

struct RLCN {
   char magic[4];
   u16 byteOrder;
   u16 version;
   u32 totalFilesize;
   u16 rahcOffset;
   u16 chunkCount;
   TTLP ttlp;
};



RLCN rlcn @ 0x0;