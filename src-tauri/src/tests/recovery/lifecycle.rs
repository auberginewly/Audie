use super::*;

#[test]
fn recovery_transcript_cache_rejects_old_workers_and_released_takes() {
    let slot = Arc::new(parking_lot::Mutex::new(Some(recovery_take(
        DictationMode::Compose,
    ))));
    assert!(cache_transcript(&slot, 1, "首次转写"));
    assert_eq!(
        slot.lock().as_ref().unwrap().transcript.as_deref(),
        Some("首次转写")
    );

    // Resume/new take owns the slot now; the first ASR call returns late.
    slot.lock().as_mut().unwrap().generation = 2;
    assert!(!cache_transcript(&slot, 1, "过期转写"));
    assert_eq!(
        slot.lock().as_ref().unwrap().transcript.as_deref(),
        Some("首次转写")
    );
    assert!(cache_transcript(&slot, 2, "恢复后的转写"));
    clear_last_take(&slot);
    assert!(!cache_transcript(&slot, 2, "清理后才返回"));
    assert!(slot.lock().is_none());
}

#[test]
fn recovery_old_timer_cannot_clear_a_resumed_takes_cache() {
    let slot: LastTakeSlot = Arc::new(parking_lot::Mutex::new(Some(recovery_take(
        DictationMode::Compose,
    ))));
    let generation = Arc::new(AtomicU64::new(1));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let timer_slot = slot.clone();
    let timer_generation = generation.clone();
    let timer = thread::spawn(move || {
        settle_take(&timer_slot, &timer_generation, 1, || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            true
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    // A resume claims its generation under this same lock. It cannot slip
    // between the timer's ownership check and terminal/cache mutation.
    assert!(slot.try_lock().is_none());
    release_tx.send(()).unwrap();
    assert!(timer.join().unwrap());
    {
        let mut cached = slot.lock();
        let next = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut take = recovery_take(DictationMode::Compose);
        take.generation = next;
        *cached = Some(take);
    }
    assert!(!settle_take(&slot, &generation, 1, || panic!(
        "old timer must not change the new terminal state"
    )));
    assert_eq!(slot.lock().as_ref().unwrap().generation, 2);
}
