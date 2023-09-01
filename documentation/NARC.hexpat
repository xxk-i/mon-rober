struct FATEntry {
    u32 startAddress;
    u32 endAddress;
};

struct FATBlock {
    char MAGIC[4];
    u32 chunkSize;
    u16 numFiles;
    u16 reserved;
    FATEntry entires[numFiles];
};

struct FNT {
    u32 subtableOffset;
    u16 firstFileID;
    u16 numberDirs;
};

struct FNTBlock {
    char MAGIC[4];
    u32 size;
    FNT fnt; 
};

struct IMGBlock {
    char MAGIC[4];
    u32 chunkSize;
    u8 data[chunkSize - 8];
};

struct NARC {
    char MAGIC[4];
    u16 byteOrder;
    u16 version;
    u32 fileSize;
    u16 chunkSize;
    u16 numFollowingChunks;
    FATBlock fat;
    FNTBlock fnt;
    IMGBlock img;
};

struct FNTSubtable {
    u8 type_or_length;
    u8 thing;
};

NARC narc @ 0x0;
FNTSubtable f @ 0x54 + narc.fnt.fnt.subtableOffset;
