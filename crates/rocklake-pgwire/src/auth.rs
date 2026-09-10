//! Multi-principal authentication backed by SCRAM verifier records.

use std::collections::{BTreeMap, BTreeSet};

use rocklake_router::{PrincipalId, PrincipalLimits, PrincipalRecord};

use crate::scram::ScramVerifier;

/// An authenticated principal and its opaque SCRAM verifier.
#[derive(Clone, Debug)]
pub struct AuthPrincipal {
    record: PrincipalRecord,
    verifier: ScramVerifier,
}

impl AuthPrincipal {
    pub fn id(&self) -> &PrincipalId {
        &self.record.id
    }

    pub fn username(&self) -> &str {
        &self.record.username
    }

    pub fn limits(&self) -> &PrincipalLimits {
        &self.record.limits
    }

    pub fn record(&self) -> &PrincipalRecord {
        &self.record
    }

    pub fn verifier(&self) -> &ScramVerifier {
        &self.verifier
    }
}

/// Principal lookup table with a fixed fake verifier for unknown usernames.
#[derive(Clone, Debug)]
pub struct PrincipalStore {
    principals: BTreeMap<String, AuthPrincipal>,
    fake_verifier: ScramVerifier,
}

impl PrincipalStore {
    /// Build a store from registry/config records.
    pub fn from_records(records: &[PrincipalRecord]) -> Result<Self, String> {
        let mut principals = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for record in records {
            if record.username.trim().is_empty() || record.username.len() > 63 {
                return Err(format!("invalid principal username: {}", record.username));
            }
            if !ids.insert(record.id.clone()) {
                return Err(format!("duplicate principal ID: {}", record.id));
            }
            let verifier = ScramVerifier::decode(&record.scram_verifier)
                .ok_or_else(|| format!("invalid SCRAM verifier for principal {}", record.id))?;
            if principals
                .insert(
                    record.username.clone(),
                    AuthPrincipal {
                        record: record.clone(),
                        verifier,
                    },
                )
                .is_some()
            {
                return Err(format!("duplicate principal username: {}", record.username));
            }
        }
        Ok(Self {
            principals,
            fake_verifier: ScramVerifier::fake(),
        })
    }

    /// Find a known principal by PostgreSQL startup username.
    pub fn get(&self, username: &str) -> Option<&AuthPrincipal> {
        self.principals.get(username)
    }

    /// Return the fake verifier used for unknown users.
    pub fn fake_verifier(&self) -> &ScramVerifier {
        &self.fake_verifier
    }

    pub fn len(&self) -> usize {
        self.principals.len()
    }

    pub fn is_empty(&self) -> bool {
        self.principals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_round_trip_and_fake_lookup() {
        let id: PrincipalId = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9".parse().unwrap();
        let verifier =
            ScramVerifier::from_password_with_salt("secret", b"fixed-salt".to_vec(), 4096);
        let record = PrincipalRecord {
            id,
            username: "alice".into(),
            scram_verifier: verifier.encode(),
            role: Default::default(),
            groups: Vec::new(),
            limits: PrincipalLimits::default(),
        };
        let store = PrincipalStore::from_records(&[record]).unwrap();

        assert_eq!(store.len(), 1);
        assert_eq!(store.get("alice").unwrap().verifier(), &verifier);
        assert!(store.get("unknown").is_none());
        assert_ne!(store.fake_verifier(), &verifier);
    }
}
