//! Task 5 — Merkle Tree Builder (SR-C4, SR-C5)
//!
//! Runs once per day at 01:00 UTC.
//! Builds a Merkle tree over the previous day's result hashes and stores the
//! root in `merkle_roots`. Anyone can use GET /verify/{result_id} to obtain
//! a Merkle inclusion proof.
//!
//! Leaf  = SHA-256(id_hex || payload_hash)
//! Node  = SHA-256(left || right)
//! Odd leaf count → last leaf is duplicated (standard padding).

use chrono::{Duration, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use tracing::{info, warn};

pub async fn run(pool: MySqlPool) {
    info!("[merkle_builder] started — runs daily at 01:00 UTC");

    loop {
        let now = Utc::now();

        // Seconds until next 01:00 UTC
        let next_run = {
            let tomorrow = (now + Duration::days(1))
                .date_naive()
                .and_hms_opt(1, 0, 0)
                .unwrap();
            let diff = tomorrow.and_utc() - now;
            diff.to_std().unwrap_or(std::time::Duration::from_secs(3600))
        };

        tokio::time::sleep(next_run).await;

        let yesterday = (Utc::now() - Duration::days(1)).date_naive();
        info!("[merkle_builder] building tree for {yesterday}");

        if let Err(e) = build_and_store(&pool, yesterday).await {
            tracing::error!("[merkle_builder] error: {e}");
        }
    }
}

async fn build_and_store(pool: &MySqlPool, date: NaiveDate) -> anyhow::Result<()> {
    // Already built for this date?
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM merkle_roots WHERE tree_date = ?"
    )
    .bind(date)
    .fetch_one(pool)
    .await?;

    if exists > 0 {
        warn!("[merkle_builder] tree for {date} already exists, skipping");
        return Ok(());
    }

    // Fetch result ids + payload hashes for the day, ordered deterministically
    let leaves_raw = sqlx::query(
        r#"
        SELECT id, payload_hash
        FROM results
        WHERE DATE(received_at) = ?
        ORDER BY id ASC
        "#
    )
    .bind(date)
    .fetch_all(pool)
    .await?;

    if leaves_raw.is_empty() {
        warn!("[merkle_builder] no results for {date}, skipping");
        return Ok(());
    }

    let leaf_count = leaves_raw.len();
    info!("[merkle_builder] {leaf_count} leaf/leaves for {date}");

    // Build leaf hashes: SHA-256(id_hex || payload_hash)
    let mut leaf_hashes: Vec<[u8; 32]> = leaves_raw
        .iter()
        .map(|row| {
            use sqlx::Row;
            let id: u64 = row.try_get::<u64, _>("id").unwrap_or(0);
            let hash: String = row.try_get("payload_hash").unwrap_or_default();
            let mut h = Sha256::new();
            h.update(format!("{:016x}", id));
            h.update(hash.as_bytes());
            h.finalize().into()
        })
        .collect();

    let root = compute_merkle_root(&mut leaf_hashes);
    let root_hex = hex::encode(root);

    sqlx::query(
        r#"INSERT INTO merkle_roots (tree_date, root_hash, leaf_count, built_at)
           VALUES (?, ?, ?, NOW())"#
    )
    .bind(date)
    .bind(&root_hex)
    .bind(leaf_count as u64)
    .execute(pool)
    .await?;

    info!("[merkle_builder] stored root={root_hex} for {date} ({leaf_count} leaves)");
    Ok(())
}

/// Build a Merkle tree and return the root hash.
/// Pads with a duplicated last leaf if the count is odd.
pub fn compute_merkle_root(leaves: &mut Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current_level = leaves.clone();

    while current_level.len() > 1 {
        if current_level.len() % 2 == 1 {
            let last = *current_level.last().unwrap();
            current_level.push(last);
        }

        let mut next_level: Vec<[u8; 32]> = Vec::new();
        for pair in current_level.chunks(2) {
            let mut h = Sha256::new();
            h.update(pair[0]);
            h.update(pair[1]);
            next_level.push(h.finalize().into());
        }

        current_level = next_level;
    }

    current_level[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_empty_returns_zero() {
        let mut leaves: Vec<[u8; 32]> = vec![];
        assert_eq!(compute_merkle_root(&mut leaves), [0u8; 32]);
    }

    #[test]
    fn merkle_single_leaf_returns_itself() {
        let leaf = [1u8; 32];
        let mut leaves = vec![leaf];
        assert_eq!(compute_merkle_root(&mut leaves), leaf);
    }

    #[test]
    fn merkle_two_leaves_is_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let root1 = compute_merkle_root(&mut vec![a, b]);
        let root2 = compute_merkle_root(&mut vec![a, b]);
        assert_eq!(root1, root2);
    }

    #[test]
    fn merkle_two_leaves_not_zero() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let root = compute_merkle_root(&mut vec![a, b]);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn merkle_odd_leaf_count_pads_correctly() {
        // 3 leaves → pads to 4 → 2 nodes → 1 root
        let mut leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let root = compute_merkle_root(&mut leaves);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn merkle_different_inputs_give_different_roots() {
        let root_a = compute_merkle_root(&mut vec![[1u8; 32], [2u8; 32]]);
        let root_b = compute_merkle_root(&mut vec![[3u8; 32], [4u8; 32]]);
        assert_ne!(root_a, root_b);
    }

    #[test]
    fn merkle_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let root_ab = compute_merkle_root(&mut vec![a, b]);
        let root_ba = compute_merkle_root(&mut vec![b, a]);
        assert_ne!(root_ab, root_ba);
    }
}
