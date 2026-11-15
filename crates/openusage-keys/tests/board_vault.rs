#![cfg(all(target_os = "linux", feature = "native-test"))]
//! Run only through the private D-Bus/keyring harness. All values are synthetic.
use openusage_keys::{
    BoardCredential, BoardGuard, BoardVault, KeyVault, OsVault, VaultError, private_keyring_fixture,
};
use zeroize::Zeroizing;

const DATABASE: [u8; 32] = [0x31; 32];
const SYNTHETIC: &[u8] = b"synthetic-board-device-session-not-a-live-token";
const REPLACED: &[u8] = b"synthetic-replaced-board-session-not-a-live-token";
const SERVICE: &str = "app.openusage.desktop.native-test";
const ACCOUNT: &str = "leaderboard-device-session-v1";

fn credential(bytes: &[u8]) -> BoardCredential {
    BoardCredential::from_secret(Zeroizing::new(bytes.to_vec())).unwrap()
}

fn selected_item_path() -> String {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = service.search_items(attributes).await.unwrap();
        assert!(found.locked.is_empty());
        assert_eq!(found.unlocked.len(), 1);
        found.unlocked[0].item_path.as_str().to_owned()
    })
}

#[test]
#[ignore = "Requires the private Secret Service harness, never a real user keyring"]
fn private_os_store_keeps_database_and_board_credentials_separate() {
    let root = private_keyring_fixture().unwrap();
    let _guard = BoardGuard::acquire(&root.join("leaderboard")).unwrap();
    let phase = std::env::var("OPENUSAGE_BOARD_VAULT_PHASE").unwrap();
    let original = credential(SYNTHETIC);
    let expected = credential(REPLACED);
    assert!(!format!("{expected:?}").contains("synthetic-"));
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
        assert_eq!(
            board.replace(&original, &expected),
            Err(VaultError::InvalidKey)
        );
        assert!(board.load().unwrap().is_none());
        board.protect(&original).unwrap();
        assert_eq!(board.protect(&original), Err(VaultError::InvalidKey));
        assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
        let original_path = selected_item_path();
        assert_eq!(
            board.replace(&expected, &original),
            Err(VaultError::InvalidKey)
        );
        assert_eq!(board.load().unwrap().unwrap().expose(), SYNTHETIC);
        board.replace(&original, &expected).unwrap();
        assert_eq!(board.load().unwrap().unwrap().expose(), REPLACED);
        assert_eq!(selected_item_path(), original_path);
        assert_eq!(
            board.replace(&original, &expected),
            Err(VaultError::InvalidKey)
        );
        assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
        return;
    }
    assert_eq!(phase, "reopen");
    assert_eq!(board.load().unwrap().unwrap().expose(), REPLACED);
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    assert_eq!(
        board.remove(&credential(b"different synthetic value")),
        Err(VaultError::InvalidKey)
    );
    assert_eq!(board.load().unwrap().unwrap().expose(), REPLACED);
    assert_eq!(
        board.replace(&original, &expected),
        Err(VaultError::InvalidKey)
    );
    database.remove(&DATABASE).unwrap();
    assert!(database.load().unwrap().is_none());
    assert_eq!(board.load().unwrap().unwrap().expose(), REPLACED);
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
    assert_eq!(
        board.replace(&expected, &original),
        Err(VaultError::InvalidKey)
    );
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = service.search_items(attributes.clone()).await.unwrap();
        assert_eq!(found.unlocked.len(), 1);
        assert_eq!(
            found.unlocked[0].get_secret().await.unwrap(),
            b"wrong-version"
        );
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
    assert_eq!(
        board.replace(&expected, &original),
        Err(VaultError::InvalidKey)
    );
    assert_eq!(*database.load().unwrap().unwrap(), DATABASE);
    runtime.block_on(async {
        let service = secret_service::SecretService::connect(secret_service::EncryptionType::Dh)
            .await
            .unwrap();
        let attributes =
            std::collections::HashMap::from([("service", SERVICE), ("username", ACCOUNT)]);
        let found = service.search_items(attributes).await.unwrap();
        assert_eq!(found.unlocked.len(), 2);
        let mut values = Vec::new();
        for item in &found.unlocked {
            values.push(Zeroizing::new(item.get_secret().await.unwrap()));
        }
        assert!(
            values
                .iter()
                .any(|secret| secret.as_slice() == b"wrong-version")
        );
        assert!(
            values
                .iter()
                .any(|secret| secret.as_slice() == b"another-invalid-value")
        );
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
    assert_eq!(
        board.replace(&expected, &original),
        Err(VaultError::Unavailable)
    );
    assert!(matches!(database.load(), Err(VaultError::Unavailable)));
}
