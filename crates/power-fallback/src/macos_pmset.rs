use crate::PowerFallback;
use anyhow::{anyhow, Result};
use std::process::Command;
use std::{thread, time};

pub struct MacosPmset;

impl PowerFallback for MacosPmset {
    fn blank_and_restore(&self) -> Result<()> {
        let status = Command::new("/usr/bin/pmset").args(["displaysleepnow"]).status()?;
        if !status.success() {
            return Err(anyhow!("pmset displaysleepnow exited with {:?}", status.code()));
        }
        thread::sleep(time::Duration::from_millis(500));
        let status = Command::new("/usr/bin/caffeinate").args(["-u", "-t", "1"]).status()?;
        if !status.success() {
            return Err(anyhow!("caffeinate wake exited with {:?}", status.code()));
        }
        Ok(())
    }
}
