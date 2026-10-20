//! Task 5 — Merkle Tree Builder (SR-C4, SR-C5)
//!
//! Runs once per day at 01:00 UTC.
//! Builds a Merkle tree over the previous day's result hashes and stores the
//! root in `merkle_roots`. Anyone can use GET /verify/{result_id} to obtain
//! a Merkle inclusion proof, proving a result is in the archive and that the
//! archive has not been rewritten.
//!
//! Tree construction:
//!   - Leaf = SHA-256(result_id || payload_hash)  — both fields are hex strings
//!   - Internal node = SHA-256(left_child || right_child)
//!   - If the number of leaves is odd, the last leaf is duplicated (standard padding)

use chrono::{Duration, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use sqlx::MySqlPool;
use tracing::{info, warn};

pub async fn run(pool: MySqlPool) {
    info!("[merkle_builder] started — runs daily at 01:00 UTC");

    loop {
        let now = Utc::now();

        // Calculate seconds until next 01:00 UTC
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
    // Already built for this date? Skip.
    let exists: bool = sqlx::query_scalar!(
        "SELECT COUNT(*) > 0 FROM merkle_roots WHERE tree_date = ?",
        date
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if exists {
        warn!("[merkle_builder] tree for {date} already exists, skipping");
        return Ok(());
    }

    // Fetch all result ids + payload hashes for the day, ordered by id (deterministic)
    let leaves_raw: Vec<(u64, String)> = sqlx::query!(
        r#"
        SELECT id, payload_hash
        FROM results
        WHERE DATE(received_at) = ?
        ORDER BY id ASC
        "#,
        date
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| (r.id as u64, r.payload_hash))
    .collect();

    if leaves_raw.is_empty() {
        warn!("[merkle_builder] no results for {date}, skipping");
        return Ok(());
    }

    let leaf_count = leaves_raw.len();
    info!("[merkle_builder] {leaf_count} leaf/leaves for {date}");

    // Build leaf hashes: SHA-256(id_hex || payload_hash)
    let mut leaf_hashes: Vec<[u8; 32]> = leaves_raw
        .iter()
        .map(|(id, hash)| {
            let mut h = Sha256::new();
            h.update(format!("{:016x}", id));
            h.update(hash.as_bytes());
            h.finalize().into()
        })
        .collect();

    let root = compute_merkle_root(&mut leaf_hashes);
    let root_hex = hex::encode(root);

    // Store the root
    sqlx::query!(
        r#"
        INSERT INTO merkle_roots (tree_date, root_hash, leaf_count, built_at)
        VALUES (?, ?, ?, NOW())
        "#,
        date,
        root_hex,
        leaf_count as u64
    )
    .execute(pool)
    .await?;

    info!("[merkle_builder] stored root={root_hex} for {date} ({leaf_count} leaves)");
    Ok(())
}

/// Build a Merkle tree from a mutable slice of 32-byte leaf hashes.
/// Returns the root hash. Pads with duplicated last leaf if count is odd.
fn compute_merkle_root(leaves: &mut Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current_level = leaves.clone();

    while current_level.len() > 1 {
        // Pad if odd
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
    fn merkle_single_leaf() {
        let leaf = [1u8; 32];
        let mut leaves = vec![leaf];
        assert_eq!(compute_merkle_root(&mut leaves), leaf);
    }

    #[test]
    fn merkle_two_leaves_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let mut leaves = vec![a, b];
        let root1 = compute_merkle_root(&mut leaves);

        let mut leaves2 = vec![a, b];
        let root2 = compute_merkle_root(&mut leaves2);

        assert_eq!(root1, root2);
    }

    #[test]
    fn merkle_odd_leaves_padded() {
        // 3 leaves → pads last leaf → 4 leaves in second level → 2 → 1 root
        let mut leaves = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let root = compute_merkle_root(&mut leaves);
        assert_ne!(root, [0u8; 32]);
    }
}
