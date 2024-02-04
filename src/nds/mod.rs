mod nds;

use std::{path::PathBuf, collections::HashMap};

use nds::NDS;

pub struct ROM {
    nds: NDS,
    files: HashMap<PathBuf, nds::FileAllocationTable>,
}