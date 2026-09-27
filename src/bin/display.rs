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

    let region = mmap.as_ptr() as *const SharedRegion;

    let region_slot = loop {
        let curr_pub = unsafe { (*region).publication.load(Ordering::Acquire) };
        let slot = (curr_pub & 0xFFFF_FFFF) as u32;
        unsafe { (*region).reader_holds.store(slot, Ordering::Release) };
        if curr_pub == unsafe { (*region).publication.load(Ordering::Acquire) } && slot != 3 {
            break slot;
        }
    };

    let snapshot = unsafe { &(*region).slots[region_slot as usize] };

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

    unsafe { (*region).reader_holds.store(3, Ordering::Release) };
    Ok(())
}
