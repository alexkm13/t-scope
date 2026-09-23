use memmap2::MmapOptions;
use std::fs::OpenOptions;
use t_scope::mbta::{convert_predicted_times, convert_train_state};

pub fn mmap_snapshot(snapshot: &Snapshot, mmap: &mut MmapMut) -> Result<(), Box<dyn std::error::Error>> {
    let ptr = mmap.as_mut_ptr() as *mut SharedSnapshot;
    let shared = unsafe { &mut *ptr };
    shared.train_count = snapshot.trains.len() as u32;

    for (i, train) in snapshot.trains.iter().enumerate() {
        shared.trains[i] = convert_train_state(train);
    }
    
    Ok(())
}
