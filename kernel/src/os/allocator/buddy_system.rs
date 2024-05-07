use super::super::stdio::{print_hex_now, println_now};
use super::SimpleAllocator;
use crate::println;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::f32::MIN;
use core::{
    alloc::{GlobalAlloc, Layout},
    cmp,
    ops::Deref,
    ptr::null_mut,
};

const FRAME_MAX_ORDER: u32 = 24;
const MIN_FRAME_SIZE: usize = 16;
const MAX_FRAME_SIZE: usize = MIN_FRAME_SIZE * 2usize.pow(FRAME_MAX_ORDER - 1);
const START_ADDRESS: usize = 0x0000_0000;

static mut FRAMES: Option<BTreeMap<usize, BTreeSet<*mut u8, &SimpleAllocator>, &SimpleAllocator>> =
    None;

pub unsafe fn init(memory_size: usize) {
    FRAMES = Some(BTreeMap::new_in(&SimpleAllocator));

    assert!(MAX_FRAME_SIZE <= memory_size);

    for i in 0..FRAME_MAX_ORDER {
        FRAMES.as_mut().unwrap().insert(
            MIN_FRAME_SIZE * 2usize.pow(i),
            BTreeSet::new_in(&SimpleAllocator),
        );
    }

    let max_frame_num = memory_size / MAX_FRAME_SIZE;

    // print_hex_now(max_frame_num as u32);
    // loop{}

    let max_frame_set = FRAMES.as_mut().unwrap().get_mut(&MAX_FRAME_SIZE).unwrap();

    for i in 0..max_frame_num {
        match FRAMES.as_mut().unwrap().get_mut(&MAX_FRAME_SIZE) {
            Some(set) => set.insert((START_ADDRESS + i * MAX_FRAME_SIZE) as *mut u8),
            None => panic!("Cannot get the key just inserted to map"),
        };
    }
}

// Allocate a block of memory with the given size
unsafe fn alloc_recursive(size: usize) -> Option<*mut u8> {
    // Find the smallest block that can hold the size
    match FRAMES.as_mut().unwrap().range_mut(size..).next() {
        Some((size, frame_set)) => {
            if frame_set.is_empty() {
                match alloc_recursive(size * 2) {
                    Some(ptr) => {
                        frame_set.insert(ptr.add(*size));
                        print_hex_now(ptr as u32);
                        print_hex_now(*size as u32);
                        println_now("======");
                        Some(ptr)
                    }
                    None => None,
                }
            } else {
                let ptr = frame_set.pop_first().unwrap();
                print_hex_now(ptr as u32);
                print_hex_now(*size as u32);
                println_now("======");
                Some(ptr)
            }
        }
        None => None,
    }
}

unsafe fn dealloc_iter(ptr: *mut u8, size: usize) -> bool {
    let mut current_size = FRAMES
        .as_ref()
        .unwrap()
        .range(size..)
        .next()
        .unwrap()
        .0
        .clone();
    let mut current_ptr = ptr;

    while current_size < MAX_FRAME_SIZE {
        print_hex_now(ptr as u32);
        print_hex_now(current_size as u32);
        println_now("======");

        let buddy_ptr = match current_ptr as usize % (current_size * 2) {
            0 => current_ptr.add(current_size),
            rem => {
                assert!(rem == current_size, "Invalid dealloc ptr");
                current_ptr.sub(current_size)
            }
        };

        let buddy_set = FRAMES.as_mut().unwrap().get_mut(&current_size).unwrap();

        buddy_set.remove(&current_ptr);

        if buddy_set.remove(&buddy_ptr) {
            println_now("Buddy found");
            print_hex_now(buddy_ptr as u32);

            current_size *= 2;
            current_ptr = cmp::min(current_ptr, buddy_ptr);

            let buddy_set = FRAMES.as_mut().unwrap().get_mut(&current_size).unwrap();
            assert!(buddy_set.insert(current_ptr), "Cannot insert buddy");
        } else {
            assert!(buddy_set.insert(current_ptr), "Cannot insert current ptr");
            return true;
        }
    }

    true
}

// Reserve a block of memory with the given size
pub unsafe fn reserve(ptr: *mut u8, size: usize) {
    let mut current_size = MAX_FRAME_SIZE;

    while current_size > MIN_FRAME_SIZE {
        let buddy_set = FRAMES.as_mut().unwrap().get_mut(&current_size).unwrap();
        let mut split_frames = Vec::new_in(&SimpleAllocator);
        let mut remove_frames = Vec::new_in(&SimpleAllocator);

        for frame in buddy_set.iter() {
            if (*frame <= ptr && frame.add(current_size) > ptr)
                || (*frame < ptr.add(size) && frame.add(current_size) > ptr.add(size))
            {
                split_frames.push(*frame);
            } else if *frame >= ptr && frame.add(current_size) <= ptr.add(size) {
                remove_frames.push(*frame);
            }
        }

        current_size /= 2;

        for frame in split_frames.iter() {
            assert!(buddy_set.remove(frame));

            let buddy_set = FRAMES.as_mut().unwrap().get_mut(&(current_size)).unwrap();
            assert!(buddy_set.insert(*frame));
            assert!(buddy_set.insert(frame.add(current_size)));
        }

        for frame in remove_frames.iter() {
            assert!(buddy_set.remove(frame));
        }
    }

    let buddy_set = FRAMES.as_mut().unwrap().get_mut(&MIN_FRAME_SIZE).unwrap();

    let first_frame_align = ptr.sub(MIN_FRAME_SIZE - 1).align_offset(MIN_FRAME_SIZE);
    let first_frame_ptr = ptr.sub(MIN_FRAME_SIZE - 1).add(first_frame_align);
    let last_frame_align = ptr
        .add(size)
        .add(MIN_FRAME_SIZE - 1)
        .align_offset(MIN_FRAME_SIZE);
    let last_frame_ptr = ptr.add(size).add(MIN_FRAME_SIZE - 1 + last_frame_align);

    loop {
        match buddy_set.range(first_frame_ptr..last_frame_ptr).next() {
            Some(frame) => {
                print_hex_now(frame.clone() as u32);
                buddy_set.remove(&frame.clone());
            }
            None => {
                println_now("=====");
                break
            }
        }
    }
}

pub struct BuddyAllocator;

unsafe impl GlobalAlloc for BuddyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            println!("Warning: Allocating zero size.");
            return null_mut();
        }

        assert_ne!(FRAMES, None, "BuddyAllocator is not initialized");
        assert!(layout.align() <= MIN_FRAME_SIZE, "Alignment too large");

        let allocate_status = alloc_recursive(layout.size());

        match allocate_status {
            Some(ptr) => {
                println_now("Memory allocated");
                println_now("");
                return ptr;
            }
            None => {
                panic!("Cannot find memory to allocate")
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match dealloc_iter(ptr, layout.size()) {
            true => {
                println_now("Memory freed");
                println_now("");
            }
            false => {
                //print_hex_now(ptr as u32);
                //print_hex_now(layout.size() as u32);
                panic!("Cannot find memory to free")
            }
        }
    }
}
