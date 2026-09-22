use memmap2::MmapOptions;
use std::fs::OpenOptions;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let num: u64 = 42;
    let bytes = num.to_ne_bytes();

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open("shared_data.bin")?;

    file.set_len(8)?;

    let mut mmap = unsafe {
        MmapOptions::new().map_mut(&file)?
    };

    mmap[0..8].copy_from_slice(&bytes);
    println!("Wrote {} to shared_data.bin", num);

    Ok(())
}
