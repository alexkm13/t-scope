use memmap2::MmapOptions;
use std::fs::OpenOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = OpenOptions::new()
    .read(true)
    .open("shared_data.bin")?;

    let mmap = unsafe {
        MmapOptions::new().map(&file)?
    };
 
    Ok(())
}
