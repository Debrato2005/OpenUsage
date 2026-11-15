#![cfg(all(target_os = "linux", feature = "native-test"))]
//! Run only through the private D-Bus/keyring harness. All values are synthetic.
use openusage_keys::{BoardCredential, BoardVault, KeyVault, OsVault, VaultError};
use std::path::Path;
use zeroize::Zeroizing;

const DATABASE: [u8; 32] = [0x31; 32];
const SYNTHETIC: &[u8] = b"synthetic-board-device-session-not-a-live-token";
const SERVICE: &str = "app.openusage.desktop.native-test";
const ACCOUNT: &str = "leaderboard-device-session-v1";

fn credential(bytes: &[u8]) -> BoardCredential {
    BoardCredential::from_secret(Zeroizing::new(bytes.to_vec())).unwrap()
}

fn private_environment() {
    let root = std::env::var("OPENUSAGE_PRIVATE_KEYRING_FIXTURE").unwrap();
    let root = Path::new(&root);
    assert!(root.is_absolute() && root.is_dir());
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap();
    assert!(address.starts_with(&format!("unix:path={}/", root.display())));
    for variable in [
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_RUNTIME_DIR",
    ] {
        let path = std::env::var(variable).unwrap();
        let path = Path::new(&path);
        assert!(path.starts_with(root) && path.is_dir());
        assert_eq!(path.canonicalize().unwrap(), path);
    }
}

#[test]
#[ignore = "Requires the private Secret Service harness, never a real user keyring"]
fn private_os_store_keeps_database_and_board_credentials_separate() {
    private_environment();
    let phase = std::env::var("OPENUSAGE_BOARD_VAULT_PHASE").unwrap();
    let expected = credential(SYNTHETIC);
    assert!(!format!("{expected:?}").contains("synthetic-board"));
    assert!(BoardCredential::from_secret(Zeroizing::new(Vec::new())).is_err());
    assert!(BoardCredential::from_secret(Zeroizing::new(vec![1; 4097])).is_err());
    if phase == "unavailable" {
        match BoardVault::new() {
            Err(VaultError::Unavailable) => {}
            Ok(vault) => assert!(matches!(vault.load(), Err(VaultError::Unavailable))),
            _ => panic!("Private stopped keyring must be unavailable"),
        }
        return;
    }
    let database = OsVault::new().unwrap();
    let board = BoardVault::new().unwrap();
    if phase == "seed" {
        assert!(database.load().unwrap().is_none());
        assert!(board.load().unwrap().is_none());
        database.protect(&DATABASE).unwrap();
        board.protect(&expected).unwrap();
        assert_eq!(board.protect(&expected), Err(VaultError::InvalidKey));
        assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
        assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
        return;
    }
    assert_eq!(phase, "reopen");
    assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    assert_eq!(
        board.remove(&credential(b"different synthetic value")),
        Err(VaultError::InvalidKey)
    );
    assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
    database.remove(&DATABASE).unwrap();
    assert!(database.load().unwrap().is_none());
    assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
    database.protect(&DATABASE).unwrap();
    board.remove(&expected).unwrap();
    board.remove(&expected).unwrap();
    assert!(board.load().unwrap().is_none());
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);

    // Plant malformed and duplicate items only in this process's private keyring.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let collection = service.get_default_collection().await.unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let item = collection
            .create_item(
                "Synthetic malformed board item",
                attributes.clone(),
                b"wrong-version",
                false,
                "text/plain",
            )
            .await
            .unwrap();
        // Blocking vault calls run outside this runtime through the helper below.
        item.get_attributes().await.unwrap();
    });
    assert!(matches!(board.load(), Err(VaultError::InvalidKey)));
    assert_eq!(board.remove(&expected), Err(VaultError::InvalidKey));
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = service.search_items(attributes.clone()).await.unwrap();
        assert_eq!(found.unlocked.len(), 1);
        let collection = service.get_default_collection().await.unwrap();
        collection
            .create_item(
                "Synthetic duplicate board item",
                attributes,
                b"another-invalid-value",
                false,
                "text/plain",
            )
            .await
            .unwrap();
    });
    assert!(matches!(board.load(), Err(VaultError::InvalidKey)));
    assert_eq!(board.remove(&expected), Err(VaultError::InvalidKey));
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = service.search_items(attributes).await.unwrap();
        assert_eq!(found.unlocked.len(), 2);
        for item in found.unlocked {
            item.delete().await.unwrap();
        }
    });
    board.protect(&expected).unwrap();
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        service
            .get_default_collection()
            .await
            .unwrap()
            .lock()
            .await
            .unwrap();
    });
    assert!(matches!(board.load(), Err(VaultError::Unavailable)));
    assert_eq!(board.remove(&expected), Err(VaultError::Unavailable));
    assert!(matches!(database.load(), Err(VaultError::Unavailable)));
}
