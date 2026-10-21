//! Task 5 — Merkle Tree Builder (SR-C4, SR-C5)
//!
//! Runs once per day at 01:00 UTC.
//! Builds a Merkle tree over the previous day's result hashes.
//! Stores the root in `merkle_roots` — anyone can use GET /verify/{id}
//! to prove a result is in the archive and was never rewritten.
//!
//! Leaf  = SHA-256(id_hex || payload_hash)
//! Node  = SHA-256(left_child || right_child)
//! Odd count → last leaf duplicated (standard padding).

use chrono::{Duration, NaiveDate, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tracing::{info, warn};

pub async fn run(pool: PgPool) {
    info!("[merkle_builder] started — runs daily at 01:00 UTC");
    loop {
        let now = Utc::now();
        let next = (now + Duration::days(1))
            .date_naive()
            .and_hms_opt(1, 0, 0)
            .unwrap();
        let wait = (next.and_utc() - now)
            .to_std()
            .unwrap_or(std::time::Duration::from_secs(3600));

        tokio::time::sleep(wait).await;

        let yesterday = (Utc::now() - Duration::days(1)).date_naive();
        info!("[merkle_builder] building tree for {yesterday}");

        if let Err(e) = build_and_store(&pool, yesterday).await {
            tracing::error!("[merkle_builder] error: {e}");
        }
    }
}

async fn build_and_store(pool: &PgPool, date: NaiveDate) -> anyhow::Result<()> {
    let exists: bool = sqlx::query_scalar!(
        "SELECT COUNT(*) > 0 FROM merkle_roots WHERE tree_date = $1",
        date
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(false);

    if exists {
        warn!("[merkle_builder] tree for {date} already exists, skipping");
        return Ok(());
    }

    let rows = sqlx::query!(
        r#"
        SELECT id, payload_hash
        FROM results
        WHERE received_at::date = $1
        ORDER BY id ASC
        "#,
        date
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        warn!("[merkle_builder] no results for {date}, skipping");
        return Ok(());
    }

    let leaf_count = rows.len();
    info!("[merkle_builder] {leaf_count} leaf/leaves for {date}");

    let mut leaf_hashes: Vec<[u8; 32]> = rows
        .iter()
        .map(|r| {
            let mut h = Sha256::new();
            h.update(format!("{:016x}", r.id));
            h.update(r.payload_hash.as_bytes());
            h.finalize().into()
        })
        .collect();

    let root = compute_merkle_root(&mut leaf_hashes);
    let root_hex = hex::encode(root);

    sqlx::query!(
        r#"
        INSERT INTO merkle_roots (tree_date, root_hash, leaf_count, built_at)
        VALUES ($1, $2, $3, NOW())
        "#,
        date,
        root_hex,
        leaf_count as i64
    )
    .execute(pool)
    .await?;

    info!("[merkle_builder] root={root_hex} stored for {date} ({leaf_count} leaves)");
    Ok(())
}

/// Build a Merkle tree and return the root hash.
pub fn compute_merkle_root(leaves: &mut Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current = leaves.clone();
    while current.len() > 1 {
        if current.len() % 2 == 1 {
            let last = *current.last().unwrap();
            current.push(last);
        }
        current = current
            .chunks(2)
            .map(|pair| {
                let mut h = Sha256::new();
                h.update(pair[0]);
                h.update(pair[1]);
                h.finalize().into()
            })
            .collect();
    }
    current[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_empty_returns_zero() {
        assert_eq!(compute_merkle_root(&mut vec![]), [0u8; 32]);
    }

    #[test]
    fn merkle_single_leaf_returns_itself() {
        let leaf = [1u8; 32];
        assert_eq!(compute_merkle_root(&mut vec![leaf]), leaf);
    }

    #[test]
    fn merkle_two_leaves_deterministic() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_eq!(
            compute_merkle_root(&mut vec![a, b]),
            compute_merkle_root(&mut vec![a, b])
        );
    }

    #[test]
    fn merkle_two_leaves_not_zero() {
        let root = compute_merkle_root(&mut vec![[1u8; 32], [2u8; 32]]);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn merkle_odd_leaf_count_pads_correctly() {
        let root = compute_merkle_root(&mut vec![[1u8; 32], [2u8; 32], [3u8; 32]]);
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn merkle_different_inputs_different_roots() {
        let r1 = compute_merkle_root(&mut vec![[1u8; 32], [2u8; 32]]);
        let r2 = compute_merkle_root(&mut vec![[3u8; 32], [4u8; 32]]);
        assert_ne!(r1, r2);
    }

    #[test]
    fn merkle_order_matters() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        assert_ne!(
            compute_merkle_root(&mut vec![a, b]),
            compute_merkle_root(&mut vec![b, a])
        );
    }
}
