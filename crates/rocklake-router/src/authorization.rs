//! Stable principal and catalog-grant policy records.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{CatalogId, CatalogLimits, RouterError};

/// Stable opaque principal identity.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct PrincipalId(String);

impl PrincipalId {
    /// Validate and construct a principal ID from canonical UUID text.
    pub fn new(value: impl Into<String>) -> Result<Self, RouterError> {
        let value = value.into();
        let parsed = uuid::Uuid::parse_str(&value)
            .map_err(|_| RouterError::InvalidConfig(format!("invalid principal id '{value}'")))?;
        Ok(Self(parsed.to_string()))
    }

    /// Return the canonical UUID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PrincipalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for PrincipalId {
    type Err = RouterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Coarse principal role metadata.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalRole {
    #[default]
    User,
    Service,
    Admin,
}

/// Durable principal metadata. The verifier is opaque SCRAM material, never a password.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalRecord {
    pub id: PrincipalId,
    pub username: String,
    pub scram_verifier: String,
    #[serde(default)]
    pub role: PrincipalRole,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub limits: PrincipalLimits,
}

/// Optional per-principal admission limits.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PrincipalLimits {
    pub max_sessions: Option<usize>,
    pub max_requests_per_second: Option<u32>,
}

/// Catalog permission granted to a principal.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Permission {
    Connect,
    Read,
    Write,
    Maintain,
    Admin,
}

/// One principal-to-catalog grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogGrant {
    pub principal_id: PrincipalId,
    pub catalog_id: CatalogId,
    pub permissions: BTreeSet<Permission>,
}

/// In-memory immutable authorization policy built from a registry snapshot or config.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuthorizationPolicy {
    generation: u64,
    principals: Vec<PrincipalRecord>,
    grants: Vec<CatalogGrant>,
}

impl AuthorizationPolicy {
    /// Validate and construct a policy.
    pub fn new(
        generation: u64,
        principals: Vec<PrincipalRecord>,
        grants: Vec<CatalogGrant>,
    ) -> Result<Self, RouterError> {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for principal in &principals {
            if principal.username.trim().is_empty() || principal.username.len() > 63 {
                return Err(RouterError::InvalidConfig(
                    "principal username must be 1-63 bytes".into(),
                ));
            }
            if !ids.insert(principal.id.clone()) || !names.insert(principal.username.clone()) {
                return Err(RouterError::InvalidConfig(
                    "principal IDs and usernames must be unique".into(),
                ));
            }
            validate_principal_limits(&principal.limits)?;
        }
        let mut grant_keys = BTreeSet::new();
        for grant in &grants {
            if !ids.contains(&grant.principal_id) {
                return Err(RouterError::InvalidConfig(format!(
                    "grant references unknown principal {}",
                    grant.principal_id
                )));
            }
            if grant.permissions.is_empty() {
                return Err(RouterError::InvalidConfig(
                    "catalog grants must contain at least one permission".into(),
                ));
            }
            if !grant_keys.insert((grant.principal_id.clone(), grant.catalog_id.clone())) {
                return Err(RouterError::InvalidConfig(
                    "duplicate principal catalog grant".into(),
                ));
            }
        }
        Ok(Self {
            generation,
            principals,
            grants,
        })
    }

    /// Empty policy that denies every catalog.
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn principals(&self) -> &[PrincipalRecord] {
        &self.principals
    }

    pub fn grants(&self) -> &[CatalogGrant] {
        &self.grants
    }

    /// Return whether a principal has a permission for a stable catalog ID.
    pub fn allows(
        &self,
        principal_id: &PrincipalId,
        catalog_id: &CatalogId,
        permission: Permission,
    ) -> bool {
        self.grants.iter().any(|grant| {
            &grant.principal_id == principal_id
                && &grant.catalog_id == catalog_id
                && grant.permissions.contains(&permission)
        })
    }

    /// Return a principal record by stable ID.
    pub fn principal(&self, principal_id: &PrincipalId) -> Option<&PrincipalRecord> {
        self.principals
            .iter()
            .find(|principal| &principal.id == principal_id)
    }

    /// Return optional limits for a principal.
    pub fn principal_limits(&self, principal_id: &PrincipalId) -> Option<&PrincipalLimits> {
        self.principal(principal_id)
            .map(|principal| &principal.limits)
    }
}

fn validate_principal_limits(limits: &PrincipalLimits) -> Result<(), RouterError> {
    if limits.max_sessions == Some(0) || limits.max_requests_per_second == Some(0) {
        return Err(RouterError::InvalidConfig(
            "principal limits must be greater than zero".into(),
        ));
    }
    Ok(())
}

/// Validate that a catalog quota does not contain zero-valued limits.
pub(crate) fn validate_limits(limits: &CatalogLimits) -> Result<(), RouterError> {
    if limits.max_sessions == Some(0)
        || limits.max_active_scans == Some(0)
        || limits.max_queued_requests == Some(0)
        || limits.max_writer_transactions == Some(0)
        || limits.max_admin_jobs == Some(0)
        || limits.max_open_handles == Some(0)
        || limits.max_in_flight_response_bytes == Some(0)
    {
        return Err(RouterError::InvalidConfig(
            "catalog limits must be greater than zero".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_are_scoped_to_principal_catalog_and_permission() {
        let principal_id: PrincipalId = "018f4f4d-6ca1-7f67-9c30-4bf2f4d116a9".parse().unwrap();
        let catalog_id: CatalogId = "018f4f4d-d520-7d91-b9f0-7018b7b50d13".parse().unwrap();
        let other_catalog: CatalogId = "018f4f4d-d521-7d91-b9f0-7018b7b50d13".parse().unwrap();
        let policy = AuthorizationPolicy::new(
            3,
            vec![PrincipalRecord {
                id: principal_id.clone(),
                username: "alice".into(),
                scram_verifier: "opaque".into(),
                role: PrincipalRole::User,
                groups: Vec::new(),
                limits: PrincipalLimits::default(),
            }],
            vec![CatalogGrant {
                principal_id: principal_id.clone(),
                catalog_id: catalog_id.clone(),
                permissions: [Permission::Connect, Permission::Read]
                    .into_iter()
                    .collect(),
            }],
        )
        .unwrap();

        assert!(policy.allows(&principal_id, &catalog_id, Permission::Read));
        assert!(!policy.allows(&principal_id, &catalog_id, Permission::Write));
        assert!(!policy.allows(&principal_id, &other_catalog, Permission::Read));
    }
}
