use memmap2::MmapMut;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use crate::mbta::{convert_train_state, SharedSnapshot, Snapshot};


#[repr(C)]
pub struct SharedRegion {
    pub reader_holds: AtomicU32,
    pub publication: AtomicU64,
    pub slots: [SharedSnapshot; 3],
}

pub unsafe fn write_snapshot(snapshot: &Snapshot, dest: *mut SharedSnapshot) {
    assert!(snapshot.trains.len() <= 512);
    unsafe {
        (*dest).train_count = snapshot.trains.len() as u32;
        for (i, train) in snapshot.trains.iter().enumerate() {
            (*dest).trains[i] = convert_train_state(train);
        }
    }
}

pub fn init_shared_region(mmap: &mut MmapMut) {
    assert!(mmap.len() >= std::mem::size_of::<SharedRegion>());

    mmap.fill(0);

    let ptr = mmap.as_mut_ptr() as *mut SharedRegion;
    unsafe {
        (*ptr).reader_holds = AtomicU32::new(3);
        (*ptr).publication = AtomicU64::new(3);
    }
}
