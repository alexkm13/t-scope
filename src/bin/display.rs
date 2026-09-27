use memmap2::MmapOptions;
use std::fs::OpenOptions;
use std::sync::atomic::Ordering;
use t_scope::ipc::SharedRegion;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = OpenOptions::new()
    .read(true)
    .write(true)
    .open("shared_data.dat")?;

    let mut mmap = unsafe {
        MmapOptions::new().map_mut(&file)?
    };

    let ptr = mmap.as_ptr() as *mut SharedRegion;
    let region = unsafe { &*ptr };
    
    let region_slot = loop {
        let curr_pub = region.publication.load(Ordering::Acquire);
        let slot = (curr_pub & 0xFFFF_FFFF) as u32;
        region.reader_holds.store(slot, Ordering::Release);
        if curr_pub == (region.publication.load(Ordering::Acquire) as u64) && slot != 3 {
            break slot;
        }
    };
    let snapshot = &region.slots[region_slot as usize];

    for i in 0..snapshot.train_count as usize {
        let train = &snapshot.trains[i];

        println!(
            "train {}: lat={}, long={}, predictions={}",
            i,
            train.lat,
            train.long,
            train.prediction_count
        );
    }
    
    region.reader_holds.store(0, Ordering::Release);
    Ok(())
}
