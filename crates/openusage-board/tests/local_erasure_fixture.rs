#![cfg(all(target_os = "linux", feature = "native-test"))]
//! Seed or hold one private enrollment for the native desktop deletion journey.
use openusage_board::{EnrollmentSession, Joined, ServiceToken};
use openusage_keys::{BoardGuard, BoardVault};
use std::io::{Read, Write};
use zeroize::Zeroizing;

#[test]
#[ignore = "Requires the private desktop local-erasure harness"]
fn private_enrollment_fixture() {
    let root = openusage_keys::private_keyring_fixture().unwrap();
    let directory = root.join("leaderboard");
    match std::env::var("OPENUSAGE_ERASURE_PHASE").unwrap().as_str() {
        "active" | "pending" => {
            let mut session =
                EnrollmentSession::open_fixture("https://board.example/", &directory).unwrap();
            let id = session.prepare_join(false).unwrap();
            if std::env::var("OPENUSAGE_ERASURE_PHASE").unwrap() == "active" {
                session
                    .finish_join(Joined {
                        device_id: id,
                        login: "synthetic-user".into(),
                        token: ServiceToken::from_secret(Zeroizing::new("b".repeat(64))).unwrap(),
                    })
                    .unwrap();
                assert_eq!(session.enrollment().unwrap().login(), "synthetic-user");
            }
        }
        "absent" => {
            let _guard = BoardGuard::acquire(&directory).unwrap();
            assert!(BoardVault::new().unwrap().load().unwrap().is_none());
        }
        "hold" => {
            let _guard = BoardGuard::acquire(&directory).unwrap();
            println!("private enrollment guard held");
            std::io::stdout().flush().unwrap();
            let mut byte = [0];
            let _ = std::io::stdin().read(&mut byte).unwrap();
        }
        _ => panic!("Unknown private enrollment phase"),
    }
}
