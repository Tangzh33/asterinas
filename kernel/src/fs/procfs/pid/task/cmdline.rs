// SPDX-License-Identifier: MPL-2.0

use crate::{
    fs::{
        procfs::template::{FileOps, ProcFileBuilder},
        utils::Inode,
    },
    prelude::*,
    process::Process,
};

/// Represents the inode at `/proc/[pid]/task/[tid]/cmdline` (and also `/proc/[pid]/cmdline`).
pub struct CmdlineFileOps(Arc<Process>);

impl CmdlineFileOps {
    pub fn new_inode(process_ref: Arc<Process>, parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcFileBuilder::new(Self(process_ref))
            .parent(parent)
            .build()
            .unwrap()
    }
}

impl FileOps for CmdlineFileOps {
    fn data(&self) -> Result<Vec<u8>> {
        let cmdline_output = if self.0.status().is_zombie() {
            // Returns 0 characters for zombie process.
            Vec::new()
        } else {
            self.0
                .init_stack_reader()
                .argv()
                .unwrap_or_else(|_| Vec::new())
        };
        Ok(cmdline_output)
    }
}
