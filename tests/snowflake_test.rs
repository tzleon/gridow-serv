use gridow_web::snowflake::Snowflake;
use std::collections::HashSet;
use std::thread;

#[test]
fn test_snowflake_concurrent_generation() {
    let mut handles = vec![];
    let mut all_ids: Vec<HashSet<i64>> = vec![HashSet::new(); 4];

    for i in 0..4 {
        handles.push(thread::spawn(move || {
            let sf = Snowflake::new(i);
            let mut ids = HashSet::new();
            for _ in 0..5000 {
                let id = sf.generate();
                ids.insert(id);
            }
            (i, ids)
        }));
    }

    for handle in handles {
        let (i, ids) = handle.join().unwrap();
        all_ids[i as usize] = ids;
    }

    let total: HashSet<i64> = all_ids.into_iter().flatten().collect();
    assert_eq!(total.len(), 20000, "concurrent generation should produce 20000 unique IDs");
}

#[test]
fn test_snowflake_multi_worker_no_collision() {
    let sf1 = Snowflake::new(0);
    let sf2 = Snowflake::new(511);
    let sf3 = Snowflake::new(1023);

    let mut ids = HashSet::new();
    for _ in 0..1000 {
        ids.insert(sf1.generate());
        ids.insert(sf2.generate());
        ids.insert(sf3.generate());
    }
    assert_eq!(ids.len(), 3000, "different workers should not collide");
}

#[test]
fn test_snowflake_worker_id_embedded_in_id() {
    let worker_ids = [0, 1, 42, 512, 1023];
    for wid in worker_ids {
        let sf = Snowflake::new(wid);
        let id = sf.generate();
        let extracted = (id >> 12) & ((1 << 10) - 1);
        assert_eq!(extracted, wid, "worker_id {} should be embedded in generated ID", wid);
    }
}

#[test]
fn test_snowflake_ids_always_positive() {
    let sf = Snowflake::new(0);
    for _ in 0..10000 {
        let id = sf.generate();
        assert!(id > 0, "generated ID must be positive");
    }
}

#[test]
fn test_snowflake_large_batch_uniqueness() {
    let sf = Snowflake::new(42);
    let mut ids = HashSet::new();
    for _ in 0..50000 {
        let id = sf.generate();
        assert!(ids.insert(id), "ID collision at count {}", ids.len());
    }
    assert_eq!(ids.len(), 50000);
}

#[test]
#[ignore]
fn test_snowflake_performance_100k() {
    let sf = Snowflake::new(0);
    let start = std::time::Instant::now();
    for _ in 0..100_000 {
        sf.generate();
    }
    let elapsed = start.elapsed();
    assert!(elapsed.as_secs() < 2, "100k IDs took too long: {:?}", elapsed);
}
