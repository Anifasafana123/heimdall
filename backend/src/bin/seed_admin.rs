// One-off CLI to create an admin user with a properly bcrypt-hashed password.
//
// Usage:
//   cargo run --bin seed_admin -- --username admin --password yourpassword
//
// Requires DATABASE_URL to be set (via .env or environment).

use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = env::args().collect();
    let mut username = String::new();
    let mut password = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--username" => {
                username = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--password" => {
                password = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            _ => i += 1,
        }
    }

    if username.is_empty() || password.is_empty() {
        eprintln!("Usage: seed_admin --username <name> --password <pass>");
        std::process::exit(1);
    }

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("DATABASE_URL is not set");
        std::process::exit(1);
    });

    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;

    let hash = bcrypt::hash(&password, 12)?;

    sqlx::query(
        "INSERT INTO admin_users (username, password_hash) VALUES (?, ?)
         ON DUPLICATE KEY UPDATE password_hash = VALUES(password_hash)",
    )
    .bind(&username)
    .bind(&hash)
    .execute(&pool)
    .await?;

    println!("admin user '{username}' created/updated successfully");
    Ok(())
}
