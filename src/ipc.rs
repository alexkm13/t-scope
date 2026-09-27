use memmap2::MmapMut;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::mbta::{convert_train_state, SharedSnapshot, Snapshot};


#[repr(C)]
pub struct SharedRegion {
    pub reader_holds: AtomicU32,
    pub publication: AtomicU64,
    pub slots: [SharedSnapshot; 3],
}

pub fn mmap_snapshot(snapshot: &Snapshot, mmap: &mut MmapMut) -> Result<(), Box<dyn std::error::Error>> {
    let ptr = mmap.as_mut_ptr() as *mut SharedSnapshot;
    let shared = unsafe { &mut *ptr };
    shared.train_count = snapshot.trains.len() as u32;

    for (i, train) in snapshot.trains.iter().enumerate() {
        shared.trains[i] = convert_train_state(train);
    }
    
    Ok(())
}

pub fn init_shared_region(mmap: &mut MmapMut) -> &mut SharedRegion {
    assert!(mmap.len() >= std::mem::size_of::<SharedRegion>());
    let ptr = mmap.as_mut_ptr() as *mut SharedRegion;
    let region = unsafe { &mut *ptr };
    
    region.reader_holds.store(3, Ordering::Release);
    region.publication.store(3u64, Ordering::Release);
    region.slots = std::array::from_fn(|_| SharedSnapshot::default());
    
    region
}
