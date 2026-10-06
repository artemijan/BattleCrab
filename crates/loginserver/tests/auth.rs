//! M3 acceptance: RequestAuthLogin against SQLite — auto-create, wrong
//! password, temp/permanent bans, double login, failed-attempt IP ban, and
//! the dashboard's `ip_bans` list.

mod common;

use common::{login, start_server, test_config};
use commons::crypt::hash_password;

#[tokio::test]
async fn auto_create_and_login_ok() {
    let server = start_server(test_config()).await;
    let (_client, reply) = login(server.addr, "newuser", "secret").await;

    assert_eq!(reply[0], 0x03, "LoginOk opcode");
    let ok1 = i32::from_le_bytes(reply[1..5].try_into().unwrap());
    let ok2 = i32::from_le_bytes(reply[5..9].try_into().unwrap());
    assert!(ok1 != 0 || ok2 != 0, "session key should be random");

    // Account was created with the Java password hash.
    let (password,): (String,) =
        sqlx::query_as("SELECT password FROM accounts WHERE login = 'newuser'")
            .fetch_one(&server.pool)
            .await
            .unwrap();
    assert_eq!(password, hash_password("secret"));
}

#[tokio::test]
async fn wrong_password_gets_access_failed() {
    let server = start_server(test_config()).await;
    let (_c, reply) = login(server.addr, "bob", "right").await;
    assert_eq!(reply[0], 0x03);

    // Java: bad password → retriveAccountInfo null → REASON_ACCESS_FAILED (0x15).
    let (_c2, reply) = login(server.addr, "bob", "wrong").await;
    assert_eq!(reply[0], 0x01, "LoginFail opcode");
    assert_eq!(reply[1], 0x15, "REASON_ACCESS_FAILED");
}

#[tokio::test]
async fn no_autocreate_rejects_unknown_account() {
    let mut config = test_config();
    config.auto_create_accounts = false;
    let server = start_server(config).await;
    let (_c, reply) = login(server.addr, "ghost", "pw").await;
    assert_eq!(reply[0], 0x01);
    assert_eq!(reply[1], 0x15);
}

#[tokio::test]
async fn banned_account_gets_account_kicked() {
    let server = start_server(test_config()).await;
    sqlx::query("INSERT INTO accounts (login, password, accessLevel) VALUES ('banned', ?, -100)")
        .bind(hash_password("pw"))
        .execute(&server.pool)
        .await
        .unwrap();

    let (_c, reply) = login(server.addr, "banned", "pw").await;
    assert_eq!(reply[0], 0x02, "AccountKicked opcode");
    assert_eq!(
        i32::from_le_bytes(reply[1..5].try_into().unwrap()),
        0x20,
        "REASON_PERMANENTLY_BANNED"
    );
}

#[tokio::test]
async fn temp_ban_via_account_data() {
    let server = start_server(test_config()).await;
    sqlx::query("INSERT INTO accounts (login, password, accessLevel) VALUES ('tempban', ?, 0)")
        .bind(hash_password("pw"))
        .execute(&server.pool)
        .await
        .unwrap();
    // ban_temp in the future → accessLevel reads as -1.
    let future = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64)
        + 3_600_000;
    sqlx::query("INSERT INTO account_data VALUES ('tempban', 'ban_temp', ?)")
        .bind(future.to_string())
        .execute(&server.pool)
        .await
        .unwrap();

    let (_c, reply) = login(server.addr, "tempban", "pw").await;
    assert_eq!(reply[0], 0x02, "AccountKicked opcode");
}

#[tokio::test]
async fn double_login_kicks_first_client() {
    let server = start_server(test_config()).await;

    let (mut first, reply) = login(server.addr, "dual", "pw").await;
    assert_eq!(reply[0], 0x03, "first login succeeds");

    // Second login on the same account: Java kicks both with ACCOUNT_IN_USE.
    let (_second, reply2) = login(server.addr, "dual", "pw").await;
    assert_eq!(reply2[0], 0x01, "LoginFail opcode");
    assert_eq!(reply2[1], 0x07, "REASON_ACCOUNT_IN_USE");

    // First client receives the kick packet.
    let kick = first.recv().await.expect("kick packet");
    assert_eq!(kick[0], 0x01);
    assert_eq!(kick[1], 0x07);
    assert!(
        first.recv().await.is_none(),
        "first connection closed after kick"
    );
}

#[tokio::test]
async fn failed_attempts_ban_ip() {
    let mut config = test_config();
    config.login_try_before_ban = 2;
    let server = start_server(config).await;

    let (_c, reply) = login(server.addr, "victim", "right").await;
    assert_eq!(reply[0], 0x03);

    for _ in 0..2 {
        let (_c, reply) = login(server.addr, "victim", "wrong").await;
        assert_eq!(reply[0], 0x01);
    }

    // IP now banned: next connection gets LoginFail(REASON_NOT_AUTHED) instead
    // of Init, under the static first-packet encryption.
    assert_refused(server.addr).await;

    // On the ban list in the database, so it survives a restart and shows on
    // the dashboard — for `LoginBlockAfterBan` from now.
    let (expires_at, banned_by, reason): (i64, String, String) =
        sqlx::query_as("SELECT expires_at, banned_by, reason FROM ip_bans WHERE ip = '127.0.0.1'")
            .fetch_one(&server.pool)
            .await
            .expect("the ban is stored");
    assert_eq!(banned_by, "login_server");
    assert_eq!(reason, "automatic: 2 wrong passwords");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let block = i64::from(test_config().login_block_after_ban) * 1000;
    assert!(
        expires_at > now && expires_at <= now + block,
        "{expires_at}"
    );
}

async fn assert_refused(addr: std::net::SocketAddr) {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut read, _write) = stream.into_split();
    let mut first = commons::network::read_frame(&mut read, 8192)
        .await
        .unwrap()
        .expect("no packet");
    commons::crypt::NewCrypt::new(&common::STATIC_BLOWFISH_KEY).decrypt(&mut first);
    common::dec_xor_pass(&mut first);
    assert_eq!(first[0], 0x01, "LoginFail opcode");
    assert_eq!(first[1], 0x06, "REASON_NOT_AUTHED");
}

/// The dashboard writes `ip_bans`; the login server reads it per connection,
/// so a row takes effect on the next connect with no reload.
#[tokio::test]
async fn an_ip_bans_row_refuses_the_address_until_it_expires() {
    let server = start_server(test_config()).await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    // Expired: no effect.
    sqlx::query("INSERT INTO ip_bans (ip, expires_at) VALUES ('127.0.0.0', ?)")
        .bind(now - 1000)
        .execute(&server.pool)
        .await
        .unwrap();
    let (_c, reply) = login(server.addr, "subnet", "pw").await;
    assert_eq!(reply[0], 0x03, "an expired ban lets the client in");

    // In force, on the subnet: refused.
    sqlx::query("UPDATE ip_bans SET expires_at = ?")
        .bind(now + 60_000)
        .execute(&server.pool)
        .await
        .unwrap();
    assert_refused(server.addr).await;

    // Permanent works the same way.
    sqlx::query("UPDATE ip_bans SET expires_at = 0")
        .execute(&server.pool)
        .await
        .unwrap();
    assert_refused(server.addr).await;
}
