//! The audio thread never allocates nor frees: a busy song played, graphs swapped in, knobs
//! turned, while every allocation on the thread is counted.

mod common;

use common::*;
use sonevo_engine::Mode;
use sonevo_project::{builtin, Device, Send};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}
static COUNT: AtomicUsize = AtomicUsize::new(0);

fn watched() -> bool {
    WATCHED.try_with(Cell::get).unwrap_or(false)
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if watched() {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if watched() {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn playing_allocates_nothing() {
    // the counting itself counts
    WATCHED.with(|w| w.set(true));
    drop(std::hint::black_box(vec![0u8; 16]));
    WATCHED.with(|w| w.set(false));
    assert_eq!(COUNT.swap(0, Ordering::Relaxed), 2);

    let notes: Vec<_> = (0..16).map(|i| note(i % 4, 1, 48 + (i * 5 % 24) as u8)).collect();
    let (mut song, channel, _) = song(&notes);
    let fx = insert(&mut song, "Fx");
    let other = insert(&mut song, "Other");
    song.channels[0].insert = Some(fx);
    for name in [
        builtin::EQ,
        builtin::COMPRESSOR,
        builtin::DELAY,
        builtin::REVERB,
        builtin::DISTORTION,
        builtin::FILTER,
    ] {
        song.inserts[0].effects.push(Device::builtin(name));
    }
    song.inserts[0].sends.push(Send { to: other, amount: 0.5 });
    song.master.effects.push(Device::builtin(builtin::LIMITER));
    let (mut engine, mut renderer) = started(&song, Mode::Song);
    engine.set_metronome(true);
    engine.play();
    let device = song.inserts[0].effects[0].id;

    let (mut left, mut right) = (vec![0.0f32; 480], vec![0.0f32; 480]);
    for block in 0..2000 {
        if block % 100 == 50 {
            song.inserts[0].effects[2].params.insert("feedback".into(), (block % 7) as f64 / 10.0);
            engine.load(&song, Mode::Song, None);
        }
        if block % 10 == 0 {
            engine.set_param(device, "midGain", (block % 36) as f64 - 18.0);
            engine.note(channel, 72, 0.7, block % 20 == 0);
        }
        WATCHED.with(|w| w.set(true));
        renderer.render(&mut left, &mut right);
        WATCHED.with(|w| w.set(false));
        engine.collect();
    }
    assert_eq!(COUNT.load(Ordering::Relaxed), 0, "the audio thread allocated");
}
