//! Provider-returned descriptive plan for one exact current local association.
use super::*;

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SavedProfilePlan {
    pub profile_id: String,
    pub binding_id: String,
    pub plan: Option<String>,
}

impl Workspace {
    pub fn load_profile_plan(
        &self,
        profile_id: &str,
        binding_id: &str,
    ) -> Result<SavedProfilePlan, CommandError> {
        validate_profile_id(profile_id)?;
        restore::validate_preview_id(binding_id)?;
        let storage = self.storage.as_ref().ok_or(CommandError::WorkspaceLocked)?;
        let binding = storage
            .profile_binding(profile_id)
            .map_err(|_| CommandError::ProviderPlanUnavailable)?
            .ok_or(CommandError::ProviderAccountIdentityUnavailable)?;
        if binding.profile_id != profile_id || binding.binding_id != binding_id {
            return Err(CommandError::ProviderAccountChanged);
        }
        let evidence = storage
            .profile_binding_evidence(&binding)
            .map_err(|_| CommandError::ProviderPlanUnavailable)?;
        Ok(SavedProfilePlan {
            profile_id: profile_id.into(),
            binding_id: binding_id.into(),
            plan: evidence.plan,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyvault::DatabaseKey;
    use std::cell::RefCell;

    #[derive(Default)]
    struct TestVault(RefCell<Option<DatabaseKey>>);
    impl KeyVault for TestVault {
        fn load(&self) -> Result<Option<DatabaseKey>, VaultError> {
            Ok(self.0.borrow().as_ref().map(|key| Zeroizing::new(**key)))
        }
        fn protect(&self, key: &[u8; 32]) -> Result<(), VaultError> {
            *self.0.borrow_mut() = Some(Zeroizing::new(*key));
            Ok(())
        }
    }

    #[test]
    fn profile_plan_isolated_by_current_profile_binding_and_missing_stays_null() {
        let root = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::new(root.path().join("workspace")).unwrap();
        workspace.setup(Ok(&TestVault::default()), None).unwrap();
        workspace.create_profile("Synthetic first").unwrap();
        let profiles = workspace
            .create_profile("Synthetic second")
            .unwrap()
            .profiles;
        let first = &profiles[0].id;
        let second = &profiles[1].id;
        let first_binding = "11111111-1111-4111-8111-111111111111";
        let second_binding = "22222222-2222-4222-8222-222222222222";
        for (id, binding, plan) in [
            (first, first_binding, Some("plus")),
            (second, second_binding, None),
        ] {
            workspace
                .storage
                .as_mut()
                .unwrap()
                .associate_profile_binding(openusage_storage::ProfileBindingInput {
                    profile_id: id.clone(),
                    expected_home_reference: id.clone(),
                    expected_previous_binding_id: None,
                    new_binding_id: binding.into(),
                    evidence: openusage_storage::ProfileIdentityEvidence {
                        email: "synthetic-plan@example.invalid".into(),
                        plan: plan.map(str::to_owned),
                    },
                    observed_at_utc: "2026-10-21T00:00:00.000Z".into(),
                })
                .unwrap();
        }
        assert_eq!(
            workspace
                .load_profile_plan(first, first_binding)
                .unwrap()
                .plan
                .as_deref(),
            Some("plus")
        );
        assert_eq!(
            workspace
                .load_profile_plan(second, second_binding)
                .unwrap()
                .plan,
            None
        );
        assert_eq!(
            workspace.load_profile_plan(first, second_binding),
            Err(CommandError::ProviderAccountChanged)
        );
        let storage = workspace.storage.as_mut().unwrap();
        storage
            .associate_profile_binding(openusage_storage::ProfileBindingInput {
                profile_id: first.clone(),
                expected_home_reference: first.clone(),
                expected_previous_binding_id: Some(first_binding.into()),
                new_binding_id: "33333333-3333-4333-8333-333333333333".into(),
                evidence: openusage_storage::ProfileIdentityEvidence {
                    email: "synthetic-new-plan@example.invalid".into(),
                    plan: Some("pro".into()),
                },
                observed_at_utc: "2026-10-21T01:00:00.000Z".into(),
            })
            .unwrap();
        assert_eq!(
            workspace.load_profile_plan(first, first_binding),
            Err(CommandError::ProviderAccountChanged)
        );
        assert_eq!(
            workspace
                .load_profile_plan(second, second_binding)
                .unwrap()
                .plan,
            None
        );
        assert_eq!(
            workspace
                .load_profile_plan(first, "33333333-3333-4333-8333-333333333333")
                .unwrap()
                .plan
                .as_deref(),
            Some("pro")
        );
        let serialized =
            serde_json::to_value(workspace.load_profile_plan(second, second_binding).unwrap())
                .unwrap();
        assert_eq!(serialized.as_object().unwrap().len(), 3);
        assert!(serialized.get("email").is_none());
    }
}
