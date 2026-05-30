//! 雪花算法 ID 生成器
//!
//! 基于 Twitter Snowflake 算法实现，生成全局唯一的 64 位整数 ID。
//!
//! # ID 结构 (64 bits)
//! * 1  bit  — 保留位，始终为 0
//! * 41 bits — 时间戳（毫秒，自定义纪元 2024-01-01）
//! * 10 bits — Worker ID（0~1023，通过 gridow.conf 配置）
//! * 12 bits — 序列号（0~4095，同毫秒内递增）
//!
//! # 时钟回拨处理
//! 若检测到时钟回拨则自旋等待时钟追上后继续生成。

use std::sync::atomic::{AtomicI64, Ordering};

/// 自定义纪元：2024-01-01 00:00:00 UTC（毫秒）
const EPOCH: i64 = 1704067200000;

const SEQUENCE_BITS: i64 = 12;
const WORKER_ID_SHIFT: i64 = SEQUENCE_BITS;
const TIMESTAMP_SHIFT: i64 = 10 + SEQUENCE_BITS;
const SEQUENCE_MASK: i64 = (1 << SEQUENCE_BITS) - 1;

/// 雪花算法 ID 生成器
pub struct Snowflake {
    worker_id: i64,
    last_timestamp: AtomicI64,
    sequence: AtomicI64,
}

impl Snowflake {
    /// 创建雪花算法生成器
    pub fn new(worker_id: i64) -> Self {
        assert!(worker_id >= 0 && worker_id <= 1023, "worker_id must be 0~1023");
        tracing::info!("Snowflake initialized: worker_id={}", worker_id);
        Self {
            worker_id,
            last_timestamp: AtomicI64::new(0),
            sequence: AtomicI64::new(0),
        }
    }

    /// 生成全局唯一的 64 位 ID
    pub fn generate(&self) -> i64 {
        loop {
            let mut timestamp = current_millis();
            let last_ts = self.last_timestamp.load(Ordering::Acquire);

            if timestamp < last_ts {
                let drift = last_ts - timestamp;
                if drift > 100 {
                    tracing::warn!(
                        "Snowflake clock drift: {}ms behind, waiting...",
                        drift
                    );
                }
                while timestamp < last_ts {
                    std::thread::yield_now();
                    timestamp = current_millis();
                }
            }

            if timestamp == last_ts {
                let next_seq = (self.sequence.load(Ordering::Relaxed) + 1) & SEQUENCE_MASK;
                if next_seq == 0 {
                    while timestamp <= last_ts {
                        std::thread::yield_now();
                        timestamp = current_millis();
                    }
                    self.sequence.store(0, Ordering::Relaxed);
                } else {
                    // CAS 避免同一毫秒内重复
                    if self.last_timestamp.compare_exchange(
                        last_ts,
                        timestamp,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    ).is_ok() {
                        self.sequence.store(next_seq, Ordering::Relaxed);
                        return ((timestamp - EPOCH) << TIMESTAMP_SHIFT)
                            | (self.worker_id << WORKER_ID_SHIFT)
                            | next_seq;
                    }
                    continue;
                }
            }

            self.sequence.store(0, Ordering::Relaxed);
            if self.last_timestamp.compare_exchange(
                last_ts,
                timestamp,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ).is_ok() {
                return ((timestamp - EPOCH) << TIMESTAMP_SHIFT)
                    | (self.worker_id << WORKER_ID_SHIFT);
            }
        }
    }
}

fn current_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_snowflake_valid_worker_id() {
        let sf = Snowflake::new(0);
        assert_eq!(sf.worker_id, 0);

        let sf = Snowflake::new(1023);
        assert_eq!(sf.worker_id, 1023);
    }

    #[test]
    #[should_panic(expected = "worker_id must be 0~1023")]
    fn test_new_snowflake_invalid_worker_id_negative() {
        Snowflake::new(-1);
    }

    #[test]
    #[should_panic(expected = "worker_id must be 0~1023")]
    fn test_new_snowflake_invalid_worker_id_overflow() {
        Snowflake::new(1024);
    }

    #[test]
    fn test_generate_id_format() {
        let sf = Snowflake::new(42);
        let id = sf.generate();

        // ID 必须为正数（符号位为 0）
        assert!(id > 0, "Generated ID should be positive");

        // 提取 worker_id（位于 bit 12~21）
        let extracted_worker = (id >> SEQUENCE_BITS) & ((1 << 10) - 1);
        assert_eq!(extracted_worker, 42, "Worker ID should be embedded correctly");
    }

    #[test]
    fn test_generate_unique_ids() {
        let sf = Snowflake::new(1);
        let mut ids = std::collections::HashSet::new();

        for _ in 0..1000 {
            let id = sf.generate();
            assert!(ids.insert(id), "ID {} should be unique", id);
        }

        assert_eq!(ids.len(), 1000);
    }

    #[test]
    fn test_generate_monotonic_increasing() {
        let sf = Snowflake::new(7);
        let mut prev = sf.generate();
        for _ in 0..100 {
            let curr = sf.generate();
            assert!(
                curr > prev,
                "IDs should be monotonically increasing: curr={} <= prev={}",
                curr, prev
            );
            prev = curr;
        }
    }

    #[test]
    fn test_different_worker_ids_produce_different_ids() {
        let sf1 = Snowflake::new(1);
        let sf2 = Snowflake::new(2);

        let id1 = sf1.generate();
        let id2 = sf2.generate();

        assert_ne!(id1, id2, "Different workers should produce different IDs");

        // 提取 worker_id
        let w1 = (id1 >> SEQUENCE_BITS) & ((1 << 10) - 1);
        let w2 = (id2 >> SEQUENCE_BITS) & ((1 << 10) - 1);
        assert_eq!(w1, 1);
        assert_eq!(w2, 2);
    }

    #[test]
    fn test_generate_100k_ids_fast() {
        let sf = Snowflake::new(0);
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            sf.generate();
        }
        let elapsed = start.elapsed();
        // 生成 10 万 ID 应在 1 秒内完成
        assert!(
            elapsed.as_secs() < 1,
            "Generating 100k IDs took too long: {:?}",
            elapsed
        );
    }
}
