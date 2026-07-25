//! Schema-derived types from pinned OpenAPI. DO NOT EDIT BY HAND.

use serde::{Deserialize, Serialize};
use crate::openai_platform::transport::SseEvent;

/// Typed params for `POST /organization/certificates/activate` (`activateOrganizationCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivateOrganizationCertificatesParams {
    pub body: ToggleCertificatesRequest,
}

/// JSON result for `activateOrganizationCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivateOrganizationCertificatesResult {
    #[serde(flatten)]
    pub body: OrganizationCertificateActivationResponse,
}

/// Typed params for `POST /organization/projects/{project_id}/certificates/activate` (`activateProjectCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivateProjectCertificatesParams {
    pub project_id: String,
    pub body: ToggleCertificatesRequest,
}

/// JSON result for `activateProjectCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivateProjectCertificatesResult {
    #[serde(flatten)]
    pub body: OrganizationProjectCertificateActivationResponse,
}

/// Typed params for `POST /organization/groups/{group_id}/users` (`add-group-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AddGroupUserParams {
    pub group_id: String,
    pub body: CreateGroupUserBody,
}

/// JSON result for `add-group-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AddGroupUserResult {
    #[serde(flatten)]
    pub body: GroupUserAssignment,
}

/// Typed params for `POST /organization/projects/{project_id}/groups` (`add-project-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AddProjectGroupParams {
    pub project_id: String,
    pub body: InviteProjectGroupBody,
}

/// JSON result for `add-project-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AddProjectGroupResult {
    #[serde(flatten)]
    pub body: ProjectGroup,
}

/// Generated object `AdminApiKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKey {
    pub object: AdminApiKeyObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub redacted_value: String,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    pub owner: AdminApiKeyOwner,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AdminApiKeyCreateResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeyCreateResponse {
    pub object: AdminApiKeyCreateResponseObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub redacted_value: String,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    pub owner: AdminApiKeyCreateResponseOwner,
    pub value: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AdminApiKeyCreateResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminApiKeyCreateResponseObjectEnum {
    #[serde(rename = "organization.admin_api_key")]
    OrganizationAdminApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AdminApiKeyCreateResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AdminApiKeyCreateResponseOwner`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeyCreateResponseOwner {
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AdminApiKeyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminApiKeyObjectEnum {
    #[serde(rename = "organization.admin_api_key")]
    OrganizationAdminApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AdminApiKeyObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AdminApiKeyOwner`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeyOwner {
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AdminApiKeysCreateBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysCreateBody {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_seconds: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /organization/admin_api_keys` (`admin-api-keys-create`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysCreateParams {
    pub body: AdminApiKeysCreateBody,
}

/// JSON result for `admin-api-keys-create`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysCreateResult {
    #[serde(flatten)]
    pub body: AdminApiKeyCreateResponse,
}

/// Typed params for `DELETE /organization/admin_api_keys/{key_id}` (`admin-api-keys-delete`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysDeleteParams {
    pub key_id: String,
}

/// JSON result for `admin-api-keys-delete`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysDeleteResult {
    #[serde(flatten)]
    pub body: AdminApiKeysDeleteResultBody,
}

/// Generated object `AdminApiKeysDeleteResultBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysDeleteResultBody {
    pub id: String,
    pub object: AdminApiKeysDeleteResultBodyObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AdminApiKeysDeleteResultBodyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminApiKeysDeleteResultBodyObjectEnum {
    #[serde(rename = "organization.admin_api_key.deleted")]
    OrganizationAdminApiKeyDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AdminApiKeysDeleteResultBodyObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/admin_api_keys/{key_id}` (`admin-api-keys-get`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysGetParams {
    pub key_id: String,
}

/// JSON result for `admin-api-keys-get`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysGetResult {
    #[serde(flatten)]
    pub body: AdminApiKey,
}

/// Typed params for `GET /organization/admin_api_keys` (`admin-api-keys-list`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<AdminApiKeysListParamsOrderEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// Generated string enum `AdminApiKeysListParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminApiKeysListParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AdminApiKeysListParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `admin-api-keys-list`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AdminApiKeysListResult {
    #[serde(flatten)]
    pub body: ApiKeyList,
}

/// Generated object `ApiKeyList`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ApiKeyList {
    pub object: ApiKeyListObjectEnum,
    pub data: Vec<AdminApiKey>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ApiKeyListObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiKeyListObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ApiKeyListObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/projects/{project_id}/archive` (`archive-project`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArchiveProjectParams {
    pub project_id: String,
}

/// JSON result for `archive-project`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ArchiveProjectResult {
    #[serde(flatten)]
    pub body: Project,
}

/// Typed params for `POST /organization/groups/{group_id}/roles` (`assign-group-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignGroupRoleParams {
    pub group_id: String,
    pub body: PublicAssignOrganizationGroupRoleBody,
}

/// JSON result for `assign-group-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignGroupRoleResult {
    #[serde(flatten)]
    pub body: GroupRoleAssignment,
}

/// Typed params for `POST /organization/users/{user_id}/roles` (`assign-user-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignUserRoleParams {
    pub user_id: String,
    pub body: PublicAssignOrganizationGroupRoleBody,
}

/// JSON result for `assign-user-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignUserRoleResult {
    #[serde(flatten)]
    pub body: UserRoleAssignment,
}

/// Generated object `AssignedRoleDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignedRoleDetails {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    pub resource_type: String,
    pub predefined_role: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by_user_obj: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_sources: Option<Vec<AssignedRoleDetailsAssignmentSourcesItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AssignedRoleDetailsAssignmentSourcesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssignedRoleDetailsAssignmentSourcesItem {
    pub principal_id: String,
    pub principal_type: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLog`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: AuditLogEventType,
    pub effective_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<AuditLogProject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<AuditLogActor>,
    #[serde(rename = "api_key.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_created: Option<AuditLogApiKeyCreated>,
    #[serde(rename = "api_key.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_updated: Option<AuditLogApiKeyUpdated>,
    #[serde(rename = "api_key.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_deleted: Option<AuditLogApiKeyDeleted>,
    #[serde(rename = "checkpoint.permission.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_permission_created: Option<AuditLogCheckpointPermissionCreated>,
    #[serde(rename = "checkpoint.permission.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_permission_deleted: Option<AuditLogCheckpointPermissionDeleted>,
    #[serde(rename = "external_key.registered")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_registered: Option<AuditLogExternalKeyRegistered>,
    #[serde(rename = "external_key.removed")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_removed: Option<AuditLogExternalKeyRemoved>,
    #[serde(rename = "group.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_created: Option<AuditLogGroupCreated>,
    #[serde(rename = "group.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_updated: Option<AuditLogGroupUpdated>,
    #[serde(rename = "group.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_deleted: Option<AuditLogGroupDeleted>,
    #[serde(rename = "scim.enabled")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scim_enabled: Option<AuditLogScimEnabled>,
    #[serde(rename = "scim.disabled")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scim_disabled: Option<AuditLogScimDisabled>,
    #[serde(rename = "invite.sent")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_sent: Option<AuditLogInviteSent>,
    #[serde(rename = "invite.accepted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_accepted: Option<AuditLogInviteAccepted>,
    #[serde(rename = "invite.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_deleted: Option<AuditLogInviteDeleted>,
    #[serde(rename = "ip_allowlist.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_allowlist_created: Option<AuditLogIpAllowlistCreated>,
    #[serde(rename = "ip_allowlist.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_allowlist_updated: Option<AuditLogIpAllowlistUpdated>,
    #[serde(rename = "ip_allowlist.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_allowlist_deleted: Option<AuditLogIpAllowlistDeleted>,
    #[serde(rename = "ip_allowlist.config.activated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_allowlist_config_activated: Option<AuditLogIpAllowlistConfigActivated>,
    #[serde(rename = "ip_allowlist.config.deactivated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_allowlist_config_deactivated: Option<AuditLogIpAllowlistConfigDeactivated>,
    #[serde(rename = "login.succeeded")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_succeeded: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "login.failed")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_failed: Option<AuditLogLoginFailed>,
    #[serde(rename = "logout.succeeded")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logout_succeeded: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(rename = "logout.failed")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logout_failed: Option<AuditLogLogoutFailed>,
    #[serde(rename = "organization.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_updated: Option<AuditLogOrganizationUpdated>,
    #[serde(rename = "project.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_created: Option<AuditLogProjectCreated>,
    #[serde(rename = "project.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_updated: Option<AuditLogProjectUpdated>,
    #[serde(rename = "project.archived")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_archived: Option<AuditLogProjectArchived>,
    #[serde(rename = "project.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_deleted: Option<AuditLogProjectDeleted>,
    #[serde(rename = "rate_limit.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_updated: Option<AuditLogRateLimitUpdated>,
    #[serde(rename = "rate_limit.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_deleted: Option<AuditLogRateLimitDeleted>,
    #[serde(rename = "role.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_created: Option<AuditLogRoleCreated>,
    #[serde(rename = "role.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_updated: Option<AuditLogRoleUpdated>,
    #[serde(rename = "role.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_deleted: Option<AuditLogRoleDeleted>,
    #[serde(rename = "role.assignment.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_assignment_created: Option<AuditLogRoleAssignmentCreated>,
    #[serde(rename = "role.assignment.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_assignment_deleted: Option<AuditLogRoleAssignmentDeleted>,
    #[serde(rename = "role.bound_to_resource")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_bound_to_resource: Option<AuditLogRoleBoundToResource>,
    #[serde(rename = "role.unbound_from_resource")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_unbound_from_resource: Option<AuditLogRoleUnboundFromResource>,
    #[serde(rename = "service_account.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_created: Option<AuditLogServiceAccountCreated>,
    #[serde(rename = "service_account.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_updated: Option<AuditLogServiceAccountUpdated>,
    #[serde(rename = "service_account.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_deleted: Option<AuditLogServiceAccountDeleted>,
    #[serde(rename = "workload_identity_provider.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_created: Option<AuditLogWorkloadIdentityProviderCreated>,
    #[serde(rename = "workload_identity_provider.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_updated: Option<AuditLogWorkloadIdentityProviderUpdated>,
    #[serde(rename = "workload_identity_provider.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_deleted: Option<AuditLogWorkloadIdentityProviderDeleted>,
    #[serde(rename = "workload_identity_provider_mapping.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_mapping_created: Option<AuditLogWorkloadIdentityProviderMappingCreated>,
    #[serde(rename = "workload_identity_provider_mapping.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_mapping_updated: Option<AuditLogWorkloadIdentityProviderMappingUpdated>,
    #[serde(rename = "workload_identity_provider_mapping.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_identity_provider_mapping_deleted: Option<AuditLogWorkloadIdentityProviderMappingDeleted>,
    #[serde(rename = "user.added")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_added: Option<AuditLogUserAdded>,
    #[serde(rename = "user.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_updated: Option<AuditLogUserUpdated>,
    #[serde(rename = "user.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_deleted: Option<AuditLogUserDeleted>,
    #[serde(rename = "certificate.created")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_created: Option<AuditLogCertificateCreated>,
    #[serde(rename = "certificate.updated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_updated: Option<AuditLogCertificateUpdated>,
    #[serde(rename = "certificate.deleted")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_deleted: Option<AuditLogCertificateDeleted>,
    #[serde(rename = "certificates.activated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates_activated: Option<AuditLogCertificatesActivated>,
    #[serde(rename = "certificates.deactivated")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates_deactivated: Option<AuditLogCertificatesDeactivated>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogActor`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogActor {
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<AuditLogActorTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<AuditLogActorSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<AuditLogActorApiKey>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogActorApiKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogActorApiKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<AuditLogActorApiKeyTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<AuditLogActorUser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account: Option<AuditLogActorServiceAccount>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AuditLogActorApiKeyTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogActorApiKeyTypeEnum {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "service_account")]
    ServiceAccount,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AuditLogActorApiKeyTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AuditLogActorServiceAccount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogActorServiceAccount {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogActorSession`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogActorSession {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<AuditLogActorUser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AuditLogActorTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogActorTypeEnum {
    #[serde(rename = "session")]
    Session,
    #[serde(rename = "api_key")]
    ApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AuditLogActorTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AuditLogActorUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogActorUser {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogApiKeyCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogApiKeyCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogApiKeyCreatedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogApiKeyCreatedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogApiKeyCreatedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogApiKeyDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogApiKeyDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogApiKeyUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogApiKeyUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogApiKeyUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogApiKeyUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogApiKeyUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificateCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificateCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificateDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificateDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificateUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificateUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificatesActivated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificatesActivated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates: Option<Vec<AuditLogCertificatesActivatedCertificatesItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificatesActivatedCertificatesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificatesActivatedCertificatesItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificatesDeactivated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificatesDeactivated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificates: Option<Vec<AuditLogCertificatesDeactivatedCertificatesItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCertificatesDeactivatedCertificatesItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCertificatesDeactivatedCertificatesItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCheckpointPermissionCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCheckpointPermissionCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogCheckpointPermissionCreatedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCheckpointPermissionCreatedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCheckpointPermissionCreatedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fine_tuned_model_checkpoint: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogCheckpointPermissionDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogCheckpointPermissionDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AuditLogEventType`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogEventType {
    #[serde(rename = "api_key.created")]
    ApiKeyCreated,
    #[serde(rename = "api_key.updated")]
    ApiKeyUpdated,
    #[serde(rename = "api_key.deleted")]
    ApiKeyDeleted,
    #[serde(rename = "certificate.created")]
    CertificateCreated,
    #[serde(rename = "certificate.updated")]
    CertificateUpdated,
    #[serde(rename = "certificate.deleted")]
    CertificateDeleted,
    #[serde(rename = "certificates.activated")]
    CertificatesActivated,
    #[serde(rename = "certificates.deactivated")]
    CertificatesDeactivated,
    #[serde(rename = "checkpoint.permission.created")]
    CheckpointPermissionCreated,
    #[serde(rename = "checkpoint.permission.deleted")]
    CheckpointPermissionDeleted,
    #[serde(rename = "external_key.registered")]
    ExternalKeyRegistered,
    #[serde(rename = "external_key.removed")]
    ExternalKeyRemoved,
    #[serde(rename = "group.created")]
    GroupCreated,
    #[serde(rename = "group.updated")]
    GroupUpdated,
    #[serde(rename = "group.deleted")]
    GroupDeleted,
    #[serde(rename = "invite.sent")]
    InviteSent,
    #[serde(rename = "invite.accepted")]
    InviteAccepted,
    #[serde(rename = "invite.deleted")]
    InviteDeleted,
    #[serde(rename = "ip_allowlist.created")]
    IpAllowlistCreated,
    #[serde(rename = "ip_allowlist.updated")]
    IpAllowlistUpdated,
    #[serde(rename = "ip_allowlist.deleted")]
    IpAllowlistDeleted,
    #[serde(rename = "ip_allowlist.config.activated")]
    IpAllowlistConfigActivated,
    #[serde(rename = "ip_allowlist.config.deactivated")]
    IpAllowlistConfigDeactivated,
    #[serde(rename = "login.succeeded")]
    LoginSucceeded,
    #[serde(rename = "login.failed")]
    LoginFailed,
    #[serde(rename = "logout.succeeded")]
    LogoutSucceeded,
    #[serde(rename = "logout.failed")]
    LogoutFailed,
    #[serde(rename = "organization.updated")]
    OrganizationUpdated,
    #[serde(rename = "project.created")]
    ProjectCreated,
    #[serde(rename = "project.updated")]
    ProjectUpdated,
    #[serde(rename = "project.archived")]
    ProjectArchived,
    #[serde(rename = "project.deleted")]
    ProjectDeleted,
    #[serde(rename = "rate_limit.updated")]
    RateLimitUpdated,
    #[serde(rename = "rate_limit.deleted")]
    RateLimitDeleted,
    #[serde(rename = "resource.deleted")]
    ResourceDeleted,
    #[serde(rename = "tunnel.created")]
    TunnelCreated,
    #[serde(rename = "tunnel.updated")]
    TunnelUpdated,
    #[serde(rename = "tunnel.deleted")]
    TunnelDeleted,
    #[serde(rename = "workload_identity_provider.created")]
    WorkloadIdentityProviderCreated,
    #[serde(rename = "workload_identity_provider.updated")]
    WorkloadIdentityProviderUpdated,
    #[serde(rename = "workload_identity_provider.deleted")]
    WorkloadIdentityProviderDeleted,
    #[serde(rename = "workload_identity_provider_mapping.created")]
    WorkloadIdentityProviderMappingCreated,
    #[serde(rename = "workload_identity_provider_mapping.updated")]
    WorkloadIdentityProviderMappingUpdated,
    #[serde(rename = "workload_identity_provider_mapping.deleted")]
    WorkloadIdentityProviderMappingDeleted,
    #[serde(rename = "role.created")]
    RoleCreated,
    #[serde(rename = "role.updated")]
    RoleUpdated,
    #[serde(rename = "role.deleted")]
    RoleDeleted,
    #[serde(rename = "role.assignment.created")]
    RoleAssignmentCreated,
    #[serde(rename = "role.assignment.deleted")]
    RoleAssignmentDeleted,
    #[serde(rename = "role.bound_to_resource")]
    RoleBoundToResource,
    #[serde(rename = "role.unbound_from_resource")]
    RoleUnboundFromResource,
    #[serde(rename = "scim.enabled")]
    ScimEnabled,
    #[serde(rename = "scim.disabled")]
    ScimDisabled,
    #[serde(rename = "service_account.created")]
    ServiceAccountCreated,
    #[serde(rename = "service_account.updated")]
    ServiceAccountUpdated,
    #[serde(rename = "service_account.deleted")]
    ServiceAccountDeleted,
    #[serde(rename = "user.added")]
    UserAdded,
    #[serde(rename = "user.updated")]
    UserUpdated,
    #[serde(rename = "user.deleted")]
    UserDeleted,
    #[serde(rename = "tenant.metadata.updated")]
    TenantMetadataUpdated,
    #[serde(rename = "tenant.microsoft_entra_mapping.upserted")]
    TenantMicrosoftEntraMappingUpserted,
    #[serde(rename = "tenant.microsoft_entra_mapping.deleted")]
    TenantMicrosoftEntraMappingDeleted,
    #[serde(rename = "tenant.workload_identity.provider.created")]
    TenantWorkloadIdentityProviderCreated,
    #[serde(rename = "tenant.workload_identity.provider.updated")]
    TenantWorkloadIdentityProviderUpdated,
    #[serde(rename = "tenant.workload_identity.provider.archived")]
    TenantWorkloadIdentityProviderArchived,
    #[serde(rename = "tenant.workload_identity.mapping.created")]
    TenantWorkloadIdentityMappingCreated,
    #[serde(rename = "tenant.workload_identity.mapping.updated")]
    TenantWorkloadIdentityMappingUpdated,
    #[serde(rename = "tenant.workload_identity.mapping.archived")]
    TenantWorkloadIdentityMappingArchived,
    #[serde(rename = "tenant.workload_identity.binding.created")]
    TenantWorkloadIdentityBindingCreated,
    #[serde(rename = "tenant.workload_identity.principal.provisioned")]
    TenantWorkloadIdentityPrincipalProvisioned,
    #[serde(rename = "tenant.admin_api_key.created")]
    TenantAdminApiKeyCreated,
    #[serde(rename = "tenant.admin_api_key.updated")]
    TenantAdminApiKeyUpdated,
    #[serde(rename = "tenant.admin_api_key.deleted")]
    TenantAdminApiKeyDeleted,
    #[serde(rename = "tenant.project_api_key.created")]
    TenantProjectApiKeyCreated,
    #[serde(rename = "tenant.chatgpt_access_token.revoked")]
    TenantChatgptAccessTokenRevoked,
    #[serde(rename = "tenant.migration.completed")]
    TenantMigrationCompleted,
    #[serde(rename = "tenant.sso.migrated")]
    TenantSsoMigrated,
    #[serde(rename = "tenant.domains.migrated")]
    TenantDomainsMigrated,
    #[serde(rename = "tenant.sso_connection.created")]
    TenantSsoConnectionCreated,
    #[serde(rename = "tenant.sso_connection.updated")]
    TenantSsoConnectionUpdated,
    #[serde(rename = "tenant.sso_connection.deleted")]
    TenantSsoConnectionDeleted,
    #[serde(rename = "tenant.sso_connection.setup.started")]
    TenantSsoConnectionSetupStarted,
    #[serde(rename = "tenant.policy.created")]
    TenantPolicyCreated,
    #[serde(rename = "tenant.policy.updated")]
    TenantPolicyUpdated,
    #[serde(rename = "tenant.policy.deleted")]
    TenantPolicyDeleted,
    #[serde(rename = "tenant.policy.attached")]
    TenantPolicyAttached,
    #[serde(rename = "tenant.policy.detached")]
    TenantPolicyDetached,
    #[serde(rename = "tenant.principal_authentication_policy.resolved")]
    TenantPrincipalAuthenticationPolicyResolved,
    #[serde(rename = "tenant.scim.setup.started")]
    TenantScimSetupStarted,
    #[serde(rename = "tenant.scim.deletion.requested")]
    TenantScimDeletionRequested,
    #[serde(rename = "tenant.scim.directory.created")]
    TenantScimDirectoryCreated,
    #[serde(rename = "tenant.product_access_policy.updated")]
    TenantProductAccessPolicyUpdated,
    #[serde(rename = "tenant.resource_share_grant.created")]
    TenantResourceShareGrantCreated,
    #[serde(rename = "tenant.resource_share_grant.updated")]
    TenantResourceShareGrantUpdated,
    #[serde(rename = "tenant.resource_share_grant.accepted")]
    TenantResourceShareGrantAccepted,
    #[serde(rename = "tenant.resource_share_grant.declined")]
    TenantResourceShareGrantDeclined,
    #[serde(rename = "tenant.resource_share_grant.revoked")]
    TenantResourceShareGrantRevoked,
    #[serde(rename = "tenant.resource_share_grant.deleted")]
    TenantResourceShareGrantDeleted,
    #[serde(rename = "tenant.service_account.updated")]
    TenantServiceAccountUpdated,
    #[serde(rename = "tenant.service_account.deleted")]
    TenantServiceAccountDeleted,
    #[serde(rename = "tenant.service_account.token.revoked")]
    TenantServiceAccountTokenRevoked,
    #[serde(rename = "tenant.billing.overage_limit.updated")]
    TenantBillingOverageLimitUpdated,
    #[serde(rename = "tenant.billing.alerts.updated")]
    TenantBillingAlertsUpdated,
    #[serde(rename = "tenant.billing.info.updated")]
    TenantBillingInfoUpdated,
    #[serde(rename = "tenant.usage_limit.workspace.updated")]
    TenantUsageLimitWorkspaceUpdated,
    #[serde(rename = "tenant.usage_limit.group.updated")]
    TenantUsageLimitGroupUpdated,
    #[serde(rename = "tenant.usage_limit.user.updated")]
    TenantUsageLimitUserUpdated,
    #[serde(rename = "tenant.usage_limit.increase_request.updated")]
    TenantUsageLimitIncreaseRequestUpdated,
    #[serde(rename = "tenant.usage_limit.increase_request.resolved")]
    TenantUsageLimitIncreaseRequestResolved,
    #[serde(rename = "tenant.group.created")]
    TenantGroupCreated,
    #[serde(rename = "tenant.group.updated")]
    TenantGroupUpdated,
    #[serde(rename = "tenant.group.deleted")]
    TenantGroupDeleted,
    #[serde(rename = "tenant.group.member.added")]
    TenantGroupMemberAdded,
    #[serde(rename = "tenant.group.member.removed")]
    TenantGroupMemberRemoved,
    #[serde(rename = "tenant.migration_rollout.status.updated")]
    TenantMigrationRolloutStatusUpdated,
    #[serde(rename = "tenant.migration_rollout.tier.updated")]
    TenantMigrationRolloutTierUpdated,
    #[serde(rename = "tenant.role.metadata.updated")]
    TenantRoleMetadataUpdated,
    #[serde(rename = "tenant.custom_role.created")]
    TenantCustomRoleCreated,
    #[serde(rename = "tenant.custom_role.updated")]
    TenantCustomRoleUpdated,
    #[serde(rename = "tenant.custom_role.deleted")]
    TenantCustomRoleDeleted,
    #[serde(rename = "tenant.role_assignment.created")]
    TenantRoleAssignmentCreated,
    #[serde(rename = "tenant.role_assignment.deleted")]
    TenantRoleAssignmentDeleted,
    #[serde(rename = "tenant.resource_role_assignment.created")]
    TenantResourceRoleAssignmentCreated,
    #[serde(rename = "tenant.resource_role_assignment.deleted")]
    TenantResourceRoleAssignmentDeleted,
    #[serde(rename = "tenant.resource_access.updated")]
    TenantResourceAccessUpdated,
    #[serde(rename = "tenant.resource_access.deleted")]
    TenantResourceAccessDeleted,
    #[serde(rename = "tenant.session_policy.created")]
    TenantSessionPolicyCreated,
    #[serde(rename = "tenant.session_policy.updated")]
    TenantSessionPolicyUpdated,
    #[serde(rename = "tenant.session_policy.deleted")]
    TenantSessionPolicyDeleted,
    #[serde(rename = "tenant.session_revocation.started")]
    TenantSessionRevocationStarted,
    #[serde(rename = "tenant.third_party_app_policy.updated")]
    TenantThirdPartyAppPolicyUpdated,
    #[serde(rename = "tenant.user.added")]
    TenantUserAdded,
    #[serde(rename = "tenant.user.updated")]
    TenantUserUpdated,
    #[serde(rename = "tenant.user.removed")]
    TenantUserRemoved,
    #[serde(rename = "tenant.user.looked_up")]
    TenantUserLookedUp,
    #[serde(rename = "tenant.user.invited")]
    TenantUserInvited,
    #[serde(rename = "tenant.membership.revoked")]
    TenantMembershipRevoked,
    #[serde(rename = "tenant.api_organization_invite.upserted")]
    TenantApiOrganizationInviteUpserted,
    #[serde(rename = "tenant.api_organization_invite.deleted")]
    TenantApiOrganizationInviteDeleted,
    #[serde(rename = "tenant.chatgpt_workspace_invite.upserted")]
    TenantChatgptWorkspaceInviteUpserted,
    #[serde(rename = "tenant.membership.accepted")]
    TenantMembershipAccepted,
    #[serde(rename = "tenant.membership.declined")]
    TenantMembershipDeclined,
    #[serde(rename = "tenant.workspace_invite_email_settings.updated")]
    TenantWorkspaceInviteEmailSettingsUpdated,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AuditLogEventType { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AuditLogExternalKeyRegistered`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogExternalKeyRegistered {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogExternalKeyRemoved`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogExternalKeyRemoved {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogGroupCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogGroupCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogGroupCreatedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogGroupCreatedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogGroupCreatedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogGroupDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogGroupDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogGroupUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogGroupUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogGroupUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogGroupUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogGroupUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogInviteAccepted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogInviteAccepted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogInviteDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogInviteDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogInviteSent`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogInviteSent {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogInviteSentData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogInviteSentData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogInviteSentData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistConfigActivated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistConfigActivated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs: Option<Vec<AuditLogIpAllowlistConfigActivatedConfigsItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistConfigActivatedConfigsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistConfigActivatedConfigsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistConfigDeactivated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistConfigDeactivated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configs: Option<Vec<AuditLogIpAllowlistConfigDeactivatedConfigsItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistConfigDeactivatedConfigsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistConfigDeactivatedConfigsItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogIpAllowlistUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogIpAllowlistUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ips: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogLoginFailed`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogLoginFailed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogLogoutFailed`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogLogoutFailed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogOrganizationUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogOrganizationUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogOrganizationUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogOrganizationUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogOrganizationUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads_ui_visibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_dashboard_visibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_call_logging: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_call_logging_project_ids: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProject`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProject {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectArchived`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectArchived {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogProjectCreatedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectCreatedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectCreatedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogProjectUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogProjectUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogProjectUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRateLimitDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRateLimitDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRateLimitUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRateLimitUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogRateLimitUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRateLimitUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRateLimitUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_audio_megabytes_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_day: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_1_day_max_input_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleAssignmentCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleAssignmentCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleAssignmentDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleAssignmentDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleBoundToResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleBoundToResource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<AuditLogRoleBoundToResourceSourceEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AuditLogRoleBoundToResourceSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogRoleBoundToResourceSourceEnum {
    #[serde(rename = "role_toggle")]
    RoleToggle,
    #[serde(rename = "role_connector_update")]
    RoleConnectorUpdate,
    #[serde(rename = "role_delete")]
    RoleDelete,
    #[serde(rename = "workspace_permissions")]
    WorkspacePermissions,
    #[serde(rename = "connector_publish")]
    ConnectorPublish,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AuditLogRoleBoundToResourceSourceEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AuditLogRoleCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleUnboundFromResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleUnboundFromResource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connector_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<AuditLogRoleUnboundFromResourceSourceEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `AuditLogRoleUnboundFromResourceSourceEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogRoleUnboundFromResourceSourceEnum {
    #[serde(rename = "role_toggle")]
    RoleToggle,
    #[serde(rename = "role_connector_update")]
    RoleConnectorUpdate,
    #[serde(rename = "role_delete")]
    RoleDelete,
    #[serde(rename = "workspace_permissions")]
    WorkspacePermissions,
    #[serde(rename = "connector_publish")]
    ConnectorPublish,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for AuditLogRoleUnboundFromResourceSourceEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `AuditLogRoleUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogRoleUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogRoleUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRoleUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions_added: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions_removed: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogScimDisabled`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogScimDisabled {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogScimEnabled`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogScimEnabled {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogServiceAccountCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogServiceAccountCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogServiceAccountCreatedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogServiceAccountCreatedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogServiceAccountCreatedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogServiceAccountDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogServiceAccountDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogServiceAccountUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogServiceAccountUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogServiceAccountUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogServiceAccountUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogServiceAccountUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogUserAdded`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogUserAdded {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<AuditLogUserAddedData>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogUserAddedData`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogUserAddedData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogUserDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogUserDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogUserUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogUserUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<AuditLogUserUpdatedChangesRequested>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogUserUpdatedChangesRequested`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogUserUpdatedChangesRequested {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderMappingCreated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderMappingCreated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderMappingDeleted`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderMappingDeleted {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderMappingUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderMappingUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `AuditLogWorkloadIdentityProviderUpdated`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AuditLogWorkloadIdentityProviderUpdated {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub changes_requested: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `Certificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    pub object: CertificateObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: i64,
    pub certificate_details: CertificateCertificateDetails,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CertificateCertificateDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CertificateCertificateDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CertificateObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CertificateObjectEnum {
    #[serde(rename = "certificate")]
    Certificate,
    #[serde(rename = "organization.certificate")]
    OrganizationCertificate,
    #[serde(rename = "organization.project.certificate")]
    OrganizationProjectCertificate,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for CertificateObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `CostsResult`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostsResult {
    pub object: CostsResultObjectEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<CostsResultAmount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_item: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `CostsResultAmount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CostsResultAmount {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CostsResultObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostsResultObjectEnum {
    #[serde(rename = "organization.costs.result")]
    OrganizationCostsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for CostsResultObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `CreateGroupBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /organization/groups` (`create-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupParams {
    pub body: CreateGroupBody,
}

/// JSON result for `create-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupResult {
    #[serde(flatten)]
    pub body: GroupResponse,
}

/// Generated object `CreateGroupUserBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateGroupUserBody {
    pub user_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /organization/spend_alerts` (`create-organization-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateOrganizationSpendAlertParams {
    pub body: CreateSpendAlertBody,
}

/// JSON result for `create-organization-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateOrganizationSpendAlertResult {
    #[serde(flatten)]
    pub body: OrganizationSpendAlert,
}

/// Typed params for `POST /organization/projects` (`create-project`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectParams {
    pub body: ProjectCreateRequest,
}

/// JSON result for `create-project`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectResult {
    #[serde(flatten)]
    pub body: Project,
}

/// Generated object `CreateProjectServiceAccountApiKeyBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectServiceAccountApiKeyBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<String>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /organization/projects/{project_id}/service_accounts` (`create-project-service-account`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectServiceAccountParams {
    pub project_id: String,
    pub body: ProjectServiceAccountCreateRequest,
}

/// JSON result for `create-project-service-account`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectServiceAccountResult {
    #[serde(flatten)]
    pub body: ProjectServiceAccountCreateResponse,
}

/// Typed params for `POST /organization/projects/{project_id}/spend_alerts` (`create-project-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectSpendAlertParams {
    pub project_id: String,
    pub body: CreateSpendAlertBody,
}

/// JSON result for `create-project-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectSpendAlertResult {
    #[serde(flatten)]
    pub body: ProjectSpendAlert,
}

/// Typed params for `POST /organization/projects/{project_id}/users` (`create-project-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectUserParams {
    pub project_id: String,
    pub body: ProjectUserCreateRequest,
}

/// JSON result for `create-project-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateProjectUserResult {
    #[serde(flatten)]
    pub body: ProjectUser,
}

/// Typed params for `POST /organization/roles` (`create-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRoleParams {
    pub body: PublicCreateOrganizationRoleBody,
}

/// JSON result for `create-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateRoleResult {
    #[serde(flatten)]
    pub body: Role,
}

/// Generated object `CreateSpendAlertBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateSpendAlertBody {
    pub threshold_amount: i64,
    pub currency: CreateSpendAlertBodyCurrencyEnum,
    pub interval: CreateSpendAlertBodyIntervalEnum,
    pub notification_channel: SpendAlertNotificationChannel,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `CreateSpendAlertBodyCurrencyEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateSpendAlertBodyCurrencyEnum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for CreateSpendAlertBodyCurrencyEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `CreateSpendAlertBodyIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreateSpendAlertBodyIntervalEnum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for CreateSpendAlertBodyIntervalEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys` (`CreateanAPIkeyforaserviceaccount`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateanAPIkeyforaserviceaccountParams {
    pub project_id: String,
    pub service_account_id: String,
    pub body: CreateProjectServiceAccountApiKeyBody,
}

/// JSON result for `CreateanAPIkeyforaserviceaccount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateanAPIkeyforaserviceaccountResult {
    #[serde(flatten)]
    pub body: ServiceAccountApiKeyBody,
}

/// Typed params for `POST /organization/certificates/deactivate` (`deactivateOrganizationCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeactivateOrganizationCertificatesParams {
    pub body: ToggleCertificatesRequest,
}

/// JSON result for `deactivateOrganizationCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeactivateOrganizationCertificatesResult {
    #[serde(flatten)]
    pub body: OrganizationCertificateDeactivationResponse,
}

/// Typed params for `POST /organization/projects/{project_id}/certificates/deactivate` (`deactivateProjectCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeactivateProjectCertificatesParams {
    pub project_id: String,
    pub body: ToggleCertificatesRequest,
}

/// JSON result for `deactivateProjectCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeactivateProjectCertificatesResult {
    #[serde(flatten)]
    pub body: OrganizationProjectCertificateDeactivationResponse,
}

/// Typed params for `DELETE /organization/certificates/{certificate_id}` (`deleteCertificate`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteCertificateParams {
    pub certificate_id: String,
}

/// Generated object `DeleteCertificateResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteCertificateResponse {
    pub object: DeleteCertificateResponseObjectEnum,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `DeleteCertificateResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteCertificateResponseObjectEnum {
    #[serde(rename = "certificate.deleted")]
    CertificateDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for DeleteCertificateResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `deleteCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteCertificateResult {
    #[serde(flatten)]
    pub body: DeleteCertificateResponse,
}

/// Typed params for `DELETE /organization/groups/{group_id}` (`delete-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteGroupParams {
    pub group_id: String,
}

/// JSON result for `delete-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteGroupResult {
    #[serde(flatten)]
    pub body: GroupDeletedResource,
}

/// Typed params for `DELETE /organization/invites/{invite_id}` (`delete-invite`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteInviteParams {
    pub invite_id: String,
}

/// JSON result for `delete-invite`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteInviteResult {
    #[serde(flatten)]
    pub body: InviteDeleteResponse,
}

/// Typed params for `DELETE /organization/spend_alerts/{alert_id}` (`delete-organization-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteOrganizationSpendAlertParams {
    pub alert_id: String,
}

/// JSON result for `delete-organization-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteOrganizationSpendAlertResult {
    #[serde(flatten)]
    pub body: OrganizationSpendAlertDeletedResource,
}

/// Typed params for `DELETE /organization/projects/{project_id}/api_keys/{api_key_id}` (`delete-project-api-key`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectApiKeyParams {
    pub project_id: String,
    pub api_key_id: String,
}

/// JSON result for `delete-project-api-key`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectApiKeyResult {
    #[serde(flatten)]
    pub body: ProjectApiKeyDeleteResponse,
}

/// Typed params for `DELETE /organization/projects/{project_id}/model_permissions` (`delete-project-model-permissions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectModelPermissionsParams {
    pub project_id: String,
}

/// JSON result for `delete-project-model-permissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectModelPermissionsResult {
    #[serde(flatten)]
    pub body: ProjectModelPermissionsDeleteResponse,
}

/// Typed params for `DELETE /organization/projects/{project_id}/service_accounts/{service_account_id}` (`delete-project-service-account`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectServiceAccountParams {
    pub project_id: String,
    pub service_account_id: String,
}

/// JSON result for `delete-project-service-account`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectServiceAccountResult {
    #[serde(flatten)]
    pub body: ProjectServiceAccountDeleteResponse,
}

/// Typed params for `DELETE /organization/projects/{project_id}/spend_alerts/{alert_id}` (`delete-project-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectSpendAlertParams {
    pub project_id: String,
    pub alert_id: String,
}

/// JSON result for `delete-project-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectSpendAlertResult {
    #[serde(flatten)]
    pub body: ProjectSpendAlertDeletedResource,
}

/// Typed params for `DELETE /organization/projects/{project_id}/users/{user_id}` (`delete-project-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectUserParams {
    pub project_id: String,
    pub user_id: String,
}

/// JSON result for `delete-project-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteProjectUserResult {
    #[serde(flatten)]
    pub body: ProjectUserDeleteResponse,
}

/// Typed params for `DELETE /organization/roles/{role_id}` (`delete-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteRoleParams {
    pub role_id: String,
}

/// JSON result for `delete-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteRoleResult {
    #[serde(flatten)]
    pub body: RoleDeletedResource,
}

/// Typed params for `DELETE /organization/users/{user_id}` (`delete-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteUserParams {
    pub user_id: String,
}

/// JSON result for `delete-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteUserResult {
    #[serde(flatten)]
    pub body: UserDeleteResponse,
}

/// Generated object `DeletedRoleAssignmentResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeletedRoleAssignmentResource {
    pub object: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `DELETE /organization/spend_limit` (`Deleteorganizationspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteorganizationspendlimitParams {
}

/// JSON result for `Deleteorganizationspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteorganizationspendlimitResult {
    #[serde(flatten)]
    pub body: OrganizationSpendLimitDeletedResource,
}

/// Typed params for `DELETE /organization/projects/{project_id}/spend_limit` (`Deleteprojectspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteprojectspendlimitParams {
    pub project_id: String,
}

/// JSON result for `Deleteprojectspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeleteprojectspendlimitResult {
    #[serde(flatten)]
    pub body: ProjectSpendLimitDeletedResource,
}

/// Typed params for `GET /organization/certificates/{certificate_id}` (`getCertificate`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCertificateParams {
    pub certificate_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<GetCertificateParamsIncludeItemEnum>>,
}

/// Generated string enum `GetCertificateParamsIncludeItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GetCertificateParamsIncludeItemEnum {
    #[serde(rename = "content")]
    Content,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GetCertificateParamsIncludeItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `getCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetCertificateResult {
    #[serde(flatten)]
    pub body: Certificate,
}

/// Typed params for `GET /organization/spend_limit` (`Getorganizationspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetorganizationspendlimitParams {
}

/// JSON result for `Getorganizationspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetorganizationspendlimitResult {
    #[serde(flatten)]
    pub body: OrganizationSpendLimitResource,
}

/// Typed params for `GET /organization/projects/{project_id}/spend_limit` (`Getprojectspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetprojectspendlimitParams {
    pub project_id: String,
}

/// JSON result for `Getprojectspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GetprojectspendlimitResult {
    #[serde(flatten)]
    pub body: ProjectSpendLimitResource,
}

/// Generated object `Group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub object: GroupObjectEnum,
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub scim_managed: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GroupDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupDeletedResource {
    pub object: GroupDeletedResourceObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupDeletedResourceObjectEnum {
    #[serde(rename = "group.deleted")]
    GroupDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupListResource {
    pub object: GroupListResourceObjectEnum,
    pub data: Vec<GroupResponse>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupMemberUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupMemberUser {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_service_account: Option<bool>,
    pub user_type: GroupMemberUserUserTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupMemberUserUserTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupMemberUserUserTypeEnum {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "tenant_user")]
    TenantUser,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupMemberUserUserTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `GroupObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupObjectEnum {
    #[serde(rename = "group")]
    Group,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupResourceWithSuccess`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupResourceWithSuccess {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub is_scim_managed: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GroupResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupResponse {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub is_scim_managed: bool,
    pub group_type: GroupResponseGroupTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupResponseGroupTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupResponseGroupTypeEnum {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "tenant_group")]
    TenantGroup,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupResponseGroupTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupRoleAssignment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupRoleAssignment {
    pub object: GroupRoleAssignmentObjectEnum,
    pub group: Group,
    pub role: Role,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupRoleAssignmentObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupRoleAssignmentObjectEnum {
    #[serde(rename = "group.role")]
    GroupRole,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupRoleAssignmentObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupUser {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `GroupUserAssignment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupUserAssignment {
    pub object: GroupUserAssignmentObjectEnum,
    pub user_id: String,
    pub group_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupUserAssignmentObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupUserAssignmentObjectEnum {
    #[serde(rename = "group.user")]
    GroupUser,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupUserAssignmentObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `GroupUserDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GroupUserDeletedResource {
    pub object: GroupUserDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `GroupUserDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupUserDeletedResourceObjectEnum {
    #[serde(rename = "group.user.deleted")]
    GroupUserDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for GroupUserDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `HostedToolPermission`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostedToolPermission {
    pub enabled: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `HostedToolPermissionUpdate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostedToolPermissionUpdate {
    pub enabled: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `Invite`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Invite {
    pub object: InviteObjectEnum,
    pub id: String,
    pub email: String,
    pub role: InviteRoleEnum,
    pub status: InviteStatusEnum,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at: Option<i64>,
    pub projects: Vec<InviteProjectsItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `InviteDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteDeleteResponse {
    pub object: InviteDeleteResponseObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `InviteDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteDeleteResponseObjectEnum {
    #[serde(rename = "organization.invite.deleted")]
    OrganizationInviteDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `InviteListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteListResponse {
    pub object: InviteListResponseObjectEnum,
    pub data: Vec<Invite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `InviteListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `InviteObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteObjectEnum {
    #[serde(rename = "organization.invite")]
    OrganizationInvite,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `InviteProjectGroupBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteProjectGroupBody {
    pub group_id: String,
    pub role: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `InviteProjectsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteProjectsItem {
    pub id: String,
    pub role: InviteProjectsItemRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `InviteProjectsItemRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteProjectsItemRoleEnum {
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "owner")]
    Owner,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteProjectsItemRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `InviteRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteRequest {
    pub email: String,
    pub role: InviteRequestRoleEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<InviteRequestProjectsItem>>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `InviteRequestProjectsItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteRequestProjectsItem {
    pub id: String,
    pub role: InviteRequestProjectsItemRoleEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `InviteRequestProjectsItemRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteRequestProjectsItemRoleEnum {
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "owner")]
    Owner,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteRequestProjectsItemRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `InviteRequestRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteRequestRoleEnum {
    #[serde(rename = "reader")]
    Reader,
    #[serde(rename = "owner")]
    Owner,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteRequestRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `InviteRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteRoleEnum {
    #[serde(rename = "owner")]
    Owner,
    #[serde(rename = "reader")]
    Reader,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `InviteStatusEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InviteStatusEnum {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "pending")]
    Pending,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for InviteStatusEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/invites` (`inviteUser`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteUserParams {
    pub body: InviteRequest,
}

/// JSON result for `inviteUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InviteUserResult {
    #[serde(flatten)]
    pub body: Invite,
}

/// Typed params for `GET /organization/audit_logs` (`list-audit-logs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListAuditLogsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<ListAuditLogsParamsEffectiveAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "project_ids[]")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "event_types[]")]
    pub event_types: Option<Vec<AuditLogEventType>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "actor_ids[]")]
    pub actor_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "actor_emails[]")]
    pub actor_emails: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "resource_ids[]")]
    pub resource_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

/// Generated object `ListAuditLogsParamsEffectiveAt`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListAuditLogsParamsEffectiveAt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gt: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gte: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lt: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lte: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ListAuditLogsResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListAuditLogsResponse {
    pub object: ListAuditLogsResponseObjectEnum,
    pub data: Vec<AuditLog>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ListAuditLogsResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListAuditLogsResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListAuditLogsResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-audit-logs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListAuditLogsResult {
    #[serde(flatten)]
    pub body: ListAuditLogsResponse,
}

/// Generated object `ListCertificatesResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListCertificatesResponse {
    pub data: Vec<OrganizationCertificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    pub object: ListCertificatesResponseObjectEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ListCertificatesResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListCertificatesResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListCertificatesResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/groups/{group_id}/roles` (`list-group-role-assignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupRoleAssignmentsParams {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListGroupRoleAssignmentsParamsOrderEnum>,
}

/// Generated string enum `ListGroupRoleAssignmentsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListGroupRoleAssignmentsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListGroupRoleAssignmentsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-group-role-assignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupRoleAssignmentsResult {
    #[serde(flatten)]
    pub body: RoleListResource,
}

/// Typed params for `GET /organization/groups/{group_id}/users` (`list-group-users`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupUsersParams {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListGroupUsersParamsOrderEnum>,
}

/// Generated string enum `ListGroupUsersParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListGroupUsersParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListGroupUsersParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-group-users`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupUsersResult {
    #[serde(flatten)]
    pub body: UserListResource,
}

/// Typed params for `GET /organization/groups` (`list-groups`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListGroupsParamsOrderEnum>,
}

/// Generated string enum `ListGroupsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListGroupsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListGroupsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-groups`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListGroupsResult {
    #[serde(flatten)]
    pub body: GroupListResource,
}

/// Typed params for `GET /organization/invites` (`list-invites`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListInvitesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// JSON result for `list-invites`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListInvitesResult {
    #[serde(flatten)]
    pub body: InviteListResponse,
}

/// Typed params for `GET /organization/certificates` (`listOrganizationCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationCertificatesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListOrganizationCertificatesParamsOrderEnum>,
}

/// Generated string enum `ListOrganizationCertificatesParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListOrganizationCertificatesParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListOrganizationCertificatesParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `listOrganizationCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationCertificatesResult {
    #[serde(flatten)]
    pub body: ListCertificatesResponse,
}

/// Typed params for `GET /organization/spend_alerts` (`list-organization-spend-alerts`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationSpendAlertsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListOrganizationSpendAlertsParamsOrderEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

/// Generated string enum `ListOrganizationSpendAlertsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListOrganizationSpendAlertsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListOrganizationSpendAlertsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-organization-spend-alerts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListOrganizationSpendAlertsResult {
    #[serde(flatten)]
    pub body: OrganizationSpendAlertListResource,
}

/// Typed params for `GET /organization/projects/{project_id}/api_keys` (`list-project-api-keys`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectApiKeysParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_project_access: Option<ListProjectApiKeysParamsOwnerProjectAccessEnum>,
}

/// Generated string enum `ListProjectApiKeysParamsOwnerProjectAccessEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProjectApiKeysParamsOwnerProjectAccessEnum {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "any")]
    Any,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListProjectApiKeysParamsOwnerProjectAccessEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-project-api-keys`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectApiKeysResult {
    #[serde(flatten)]
    pub body: ProjectApiKeyListResponse,
}

/// Typed params for `GET /organization/projects/{project_id}/certificates` (`listProjectCertificates`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectCertificatesParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListProjectCertificatesParamsOrderEnum>,
}

/// Generated string enum `ListProjectCertificatesParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProjectCertificatesParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListProjectCertificatesParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ListProjectCertificatesResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectCertificatesResponse {
    pub data: Vec<OrganizationProjectCertificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    pub object: ListProjectCertificatesResponseObjectEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ListProjectCertificatesResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProjectCertificatesResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListProjectCertificatesResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `listProjectCertificates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectCertificatesResult {
    #[serde(flatten)]
    pub body: ListProjectCertificatesResponse,
}

/// Typed params for `GET /organization/projects/{project_id}/groups` (`list-project-groups`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectGroupsParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListProjectGroupsParamsOrderEnum>,
}

/// Generated string enum `ListProjectGroupsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProjectGroupsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListProjectGroupsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-project-groups`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectGroupsResult {
    #[serde(flatten)]
    pub body: ProjectGroupListResource,
}

/// Typed params for `GET /organization/projects/{project_id}/rate_limits` (`list-project-rate-limits`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectRateLimitsParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

/// JSON result for `list-project-rate-limits`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectRateLimitsResult {
    #[serde(flatten)]
    pub body: ProjectRateLimitListResponse,
}

/// Typed params for `GET /organization/projects/{project_id}/service_accounts` (`list-project-service-accounts`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectServiceAccountsParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// JSON result for `list-project-service-accounts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectServiceAccountsResult {
    #[serde(flatten)]
    pub body: ProjectServiceAccountListResponse,
}

/// Typed params for `GET /organization/projects/{project_id}/spend_alerts` (`list-project-spend-alerts`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectSpendAlertsParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListProjectSpendAlertsParamsOrderEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
}

/// Generated string enum `ListProjectSpendAlertsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListProjectSpendAlertsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListProjectSpendAlertsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-project-spend-alerts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectSpendAlertsResult {
    #[serde(flatten)]
    pub body: ProjectSpendAlertListResource,
}

/// Typed params for `GET /organization/projects/{project_id}/users` (`list-project-users`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectUsersParams {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// JSON result for `list-project-users`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectUsersResult {
    #[serde(flatten)]
    pub body: ProjectUserListResponse,
}

/// Typed params for `GET /organization/projects` (`list-projects`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_archived: Option<bool>,
}

/// JSON result for `list-projects`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListProjectsResult {
    #[serde(flatten)]
    pub body: ProjectListResponse,
}

/// Typed params for `GET /organization/roles` (`list-roles`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListRolesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListRolesParamsOrderEnum>,
}

/// Generated string enum `ListRolesParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListRolesParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListRolesParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-roles`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListRolesResult {
    #[serde(flatten)]
    pub body: PublicRoleListResource,
}

/// Typed params for `GET /organization/users/{user_id}/roles` (`list-user-role-assignments`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListUserRoleAssignmentsParams {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<ListUserRoleAssignmentsParamsOrderEnum>,
}

/// Generated string enum `ListUserRoleAssignmentsParamsOrderEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListUserRoleAssignmentsParamsOrderEnum {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ListUserRoleAssignmentsParamsOrderEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `list-user-role-assignments`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListUserRoleAssignmentsResult {
    #[serde(flatten)]
    pub body: RoleListResource,
}

/// Typed params for `GET /organization/users` (`list-users`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListUsersParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emails: Option<Vec<String>>,
}

/// JSON result for `list-users`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListUsersResult {
    #[serde(flatten)]
    pub body: UserListResponse,
}

/// Typed params for `POST /organization/certificates/{certificate_id}` (`modifyCertificate`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyCertificateParams {
    pub certificate_id: String,
    pub body: ModifyCertificateRequest,
}

/// Generated object `ModifyCertificateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyCertificateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// JSON result for `modifyCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyCertificateResult {
    #[serde(flatten)]
    pub body: Certificate,
}

/// Typed params for `POST /organization/projects/{project_id}` (`modify-project`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyProjectParams {
    pub project_id: String,
    pub body: ProjectUpdateRequest,
}

/// JSON result for `modify-project`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyProjectResult {
    #[serde(flatten)]
    pub body: Project,
}

/// Typed params for `POST /organization/projects/{project_id}/users/{user_id}` (`modify-project-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyProjectUserParams {
    pub project_id: String,
    pub user_id: String,
    pub body: ProjectUserUpdateRequest,
}

/// JSON result for `modify-project-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyProjectUserResult {
    #[serde(flatten)]
    pub body: ProjectUser,
}

/// Typed params for `POST /organization/users/{user_id}` (`modify-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyUserParams {
    pub user_id: String,
    pub body: UserRoleUpdateRequest,
}

/// JSON result for `modify-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifyUserResult {
    #[serde(flatten)]
    pub body: User,
}

/// Generated object `OrganizationCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationCertificate {
    pub object: OrganizationCertificateObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: i64,
    pub certificate_details: OrganizationCertificateCertificateDetails,
    pub active: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrganizationCertificateActivationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationCertificateActivationResponse {
    pub object: OrganizationCertificateActivationResponseObjectEnum,
    pub data: Vec<OrganizationCertificate>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationCertificateActivationResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationCertificateActivationResponseObjectEnum {
    #[serde(rename = "organization.certificate.activation")]
    OrganizationCertificateActivation,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationCertificateActivationResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationCertificateCertificateDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationCertificateCertificateDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrganizationCertificateDeactivationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationCertificateDeactivationResponse {
    pub object: OrganizationCertificateDeactivationResponseObjectEnum,
    pub data: Vec<OrganizationCertificate>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationCertificateDeactivationResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationCertificateDeactivationResponseObjectEnum {
    #[serde(rename = "organization.certificate.deactivation")]
    OrganizationCertificateDeactivation,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationCertificateDeactivationResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `OrganizationCertificateObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationCertificateObjectEnum {
    #[serde(rename = "organization.certificate")]
    OrganizationCertificate,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationCertificateObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationDataRetention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationDataRetention {
    pub object: OrganizationDataRetentionObjectEnum,
    #[serde(rename = "type")]
    pub type_: OrganizationDataRetentionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationDataRetentionObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationDataRetentionObjectEnum {
    #[serde(rename = "organization.data_retention")]
    OrganizationDataRetention,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationDataRetentionObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `OrganizationDataRetentionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationDataRetentionTypeEnum {
    #[serde(rename = "zero_data_retention")]
    ZeroDataRetention,
    #[serde(rename = "modified_abuse_monitoring")]
    ModifiedAbuseMonitoring,
    #[serde(rename = "enhanced_zero_data_retention")]
    EnhancedZeroDataRetention,
    #[serde(rename = "enhanced_modified_abuse_monitoring")]
    EnhancedModifiedAbuseMonitoring,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationDataRetentionTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationProjectCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationProjectCertificate {
    pub object: OrganizationProjectCertificateObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: i64,
    pub certificate_details: OrganizationProjectCertificateCertificateDetails,
    pub active: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrganizationProjectCertificateActivationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationProjectCertificateActivationResponse {
    pub object: OrganizationProjectCertificateActivationResponseObjectEnum,
    pub data: Vec<OrganizationProjectCertificate>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationProjectCertificateActivationResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationProjectCertificateActivationResponseObjectEnum {
    #[serde(rename = "organization.project.certificate.activation")]
    OrganizationProjectCertificateActivation,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationProjectCertificateActivationResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationProjectCertificateCertificateDetails`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationProjectCertificateCertificateDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `OrganizationProjectCertificateDeactivationResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationProjectCertificateDeactivationResponse {
    pub object: OrganizationProjectCertificateDeactivationResponseObjectEnum,
    pub data: Vec<OrganizationProjectCertificate>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationProjectCertificateDeactivationResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationProjectCertificateDeactivationResponseObjectEnum {
    #[serde(rename = "organization.project.certificate.deactivation")]
    OrganizationProjectCertificateDeactivation,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationProjectCertificateDeactivationResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `OrganizationProjectCertificateObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationProjectCertificateObjectEnum {
    #[serde(rename = "organization.project.certificate")]
    OrganizationProjectCertificate,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationProjectCertificateObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationSpendAlert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSpendAlert {
    pub id: String,
    pub object: OrganizationSpendAlertObjectEnum,
    pub threshold_amount: i64,
    pub currency: OrganizationSpendAlertCurrencyEnum,
    pub interval: OrganizationSpendAlertIntervalEnum,
    pub notification_channel: SpendAlertNotificationChannel,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationSpendAlertCurrencyEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendAlertCurrencyEnum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendAlertCurrencyEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationSpendAlertDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSpendAlertDeletedResource {
    pub id: String,
    pub object: OrganizationSpendAlertDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationSpendAlertDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendAlertDeletedResourceObjectEnum {
    #[serde(rename = "organization.spend_alert.deleted")]
    OrganizationSpendAlertDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendAlertDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `OrganizationSpendAlertIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendAlertIntervalEnum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendAlertIntervalEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationSpendAlertListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSpendAlertListResource {
    pub object: OrganizationSpendAlertListResourceObjectEnum,
    pub data: Vec<OrganizationSpendAlert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationSpendAlertListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendAlertListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendAlertListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `OrganizationSpendAlertObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendAlertObjectEnum {
    #[serde(rename = "organization.spend_alert")]
    OrganizationSpendAlert,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendAlertObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationSpendLimitDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSpendLimitDeletedResource {
    pub object: OrganizationSpendLimitDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationSpendLimitDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendLimitDeletedResourceObjectEnum {
    #[serde(rename = "organization.spend_limit.deleted")]
    OrganizationSpendLimitDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendLimitDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `OrganizationSpendLimitResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrganizationSpendLimitResource {
    pub object: OrganizationSpendLimitResourceObjectEnum,
    pub threshold_amount: i64,
    pub currency: SpendLimitCurrency,
    pub interval: SpendLimitInterval,
    pub enforcement: SpendLimitEnforcement,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `OrganizationSpendLimitResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrganizationSpendLimitResourceObjectEnum {
    #[serde(rename = "organization.spend_limit")]
    OrganizationSpendLimit,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for OrganizationSpendLimitResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `Project`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub object: ProjectObjectEnum,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectApiKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKey {
    pub object: ProjectApiKeyObjectEnum,
    pub redacted_value: String,
    pub name: String,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<i64>,
    pub id: String,
    pub owner_project_access: ProjectApiKeyOwnerProjectAccessEnum,
    pub owner: ProjectApiKeyOwner,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectApiKeyDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKeyDeleteResponse {
    pub object: ProjectApiKeyDeleteResponseObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectApiKeyDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectApiKeyDeleteResponseObjectEnum {
    #[serde(rename = "organization.project.api_key.deleted")]
    OrganizationProjectApiKeyDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectApiKeyDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectApiKeyListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKeyListResponse {
    pub object: ProjectApiKeyListResponseObjectEnum,
    pub data: Vec<ProjectApiKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectApiKeyListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectApiKeyListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectApiKeyListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectApiKeyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectApiKeyObjectEnum {
    #[serde(rename = "organization.project.api_key")]
    OrganizationProjectApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectApiKeyObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectApiKeyOwner`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKeyOwner {
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<ProjectApiKeyOwnerTypeEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<ProjectApiKeyOwnerUser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_account: Option<ProjectApiKeyOwnerServiceAccount>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectApiKeyOwnerProjectAccessEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectApiKeyOwnerProjectAccessEnum {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectApiKeyOwnerProjectAccessEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectApiKeyOwnerServiceAccount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKeyOwnerServiceAccount {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub role: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectApiKeyOwnerTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectApiKeyOwnerTypeEnum {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "service_account")]
    ServiceAccount,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectApiKeyOwnerTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectApiKeyOwnerUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectApiKeyOwnerUser {
    pub id: String,
    pub email: String,
    pub name: String,
    pub created_at: i64,
    pub role: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectCreateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectCreateRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geography: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectDataRetention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectDataRetention {
    pub object: ProjectDataRetentionObjectEnum,
    #[serde(rename = "type")]
    pub type_: ProjectDataRetentionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectDataRetentionObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectDataRetentionObjectEnum {
    #[serde(rename = "project.data_retention")]
    ProjectDataRetention,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectDataRetentionObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectDataRetentionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectDataRetentionTypeEnum {
    #[serde(rename = "organization_default")]
    OrganizationDefault,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "zero_data_retention")]
    ZeroDataRetention,
    #[serde(rename = "modified_abuse_monitoring")]
    ModifiedAbuseMonitoring,
    #[serde(rename = "enhanced_zero_data_retention")]
    EnhancedZeroDataRetention,
    #[serde(rename = "enhanced_modified_abuse_monitoring")]
    EnhancedModifiedAbuseMonitoring,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectDataRetentionTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectGroup`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroup {
    pub object: ProjectGroupObjectEnum,
    pub project_id: String,
    pub group_id: String,
    pub group_name: String,
    pub group_type: ProjectGroupGroupTypeEnum,
    pub created_at: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectGroupDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroupDeletedResource {
    pub object: ProjectGroupDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectGroupDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGroupDeletedResourceObjectEnum {
    #[serde(rename = "project.group.deleted")]
    ProjectGroupDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectGroupDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectGroupGroupTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGroupGroupTypeEnum {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "tenant_group")]
    TenantGroup,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectGroupGroupTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectGroupListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectGroupListResource {
    pub object: ProjectGroupListResourceObjectEnum,
    pub data: Vec<ProjectGroup>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectGroupListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGroupListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectGroupListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectGroupObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGroupObjectEnum {
    #[serde(rename = "project.group")]
    ProjectGroup,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectGroupObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectHostedToolPermissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectHostedToolPermissions {
    pub file_search: HostedToolPermission,
    pub web_search: HostedToolPermission,
    pub image_generation: HostedToolPermission,
    pub mcp: HostedToolPermission,
    pub code_interpreter: HostedToolPermission,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectHostedToolPermissionsUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectHostedToolPermissionsUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_search: Option<HostedToolPermissionUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search: Option<HostedToolPermissionUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_generation: Option<HostedToolPermissionUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<HostedToolPermissionUpdate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_interpreter: Option<HostedToolPermissionUpdate>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub object: ProjectListResponseObjectEnum,
    pub data: Vec<Project>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectModelPermissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectModelPermissions {
    pub object: ProjectModelPermissionsObjectEnum,
    pub mode: ProjectModelPermissionsModeEnum,
    pub model_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectModelPermissionsDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectModelPermissionsDeleteResponse {
    pub object: ProjectModelPermissionsDeleteResponseObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectModelPermissionsDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectModelPermissionsDeleteResponseObjectEnum {
    #[serde(rename = "project.model_permissions.deleted")]
    ProjectModelPermissionsDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectModelPermissionsDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectModelPermissionsModeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectModelPermissionsModeEnum {
    #[serde(rename = "allow_list")]
    AllowList,
    #[serde(rename = "deny_list")]
    DenyList,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectModelPermissionsModeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectModelPermissionsObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectModelPermissionsObjectEnum {
    #[serde(rename = "project.model_permissions")]
    ProjectModelPermissions,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectModelPermissionsObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectModelPermissionsUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectModelPermissionsUpdateRequest {
    pub mode: ProjectModelPermissionsUpdateRequestModeEnum,
    pub model_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectModelPermissionsUpdateRequestModeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectModelPermissionsUpdateRequestModeEnum {
    #[serde(rename = "allow_list")]
    AllowList,
    #[serde(rename = "deny_list")]
    DenyList,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectModelPermissionsUpdateRequestModeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectObjectEnum {
    #[serde(rename = "organization.project")]
    OrganizationProject,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectRateLimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectRateLimit {
    pub object: ProjectRateLimitObjectEnum,
    pub id: String,
    pub model: String,
    pub max_requests_per_1_minute: i64,
    pub max_tokens_per_1_minute: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_audio_megabytes_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_day: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_1_day_max_input_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectRateLimitListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectRateLimitListResponse {
    pub object: ProjectRateLimitListResponseObjectEnum,
    pub data: Vec<ProjectRateLimit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectRateLimitListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectRateLimitListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectRateLimitListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectRateLimitObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectRateLimitObjectEnum {
    #[serde(rename = "project.rate_limit")]
    ProjectRateLimit,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectRateLimitObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectRateLimitUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectRateLimitUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_images_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_audio_megabytes_per_1_minute: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_1_day: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_1_day_max_input_tokens: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectServiceAccount`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccount {
    pub object: ProjectServiceAccountObjectEnum,
    pub id: String,
    pub name: String,
    pub role: ProjectServiceAccountRoleEnum,
    pub created_at: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectServiceAccountApiKey`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccountApiKey {
    pub object: ProjectServiceAccountApiKeyObjectEnum,
    pub value: String,
    pub name: String,
    pub created_at: i64,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectServiceAccountApiKeyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountApiKeyObjectEnum {
    #[serde(rename = "organization.project.service_account.api_key")]
    OrganizationProjectServiceAccountApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountApiKeyObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectServiceAccountCreateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccountCreateRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_service_account_only: Option<bool>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectServiceAccountCreateResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccountCreateResponse {
    pub object: ProjectServiceAccountCreateResponseObjectEnum,
    pub id: String,
    pub name: String,
    pub role: ProjectServiceAccountCreateResponseRoleEnum,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<ProjectServiceAccountApiKey>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectServiceAccountCreateResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountCreateResponseObjectEnum {
    #[serde(rename = "organization.project.service_account")]
    OrganizationProjectServiceAccount,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountCreateResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectServiceAccountCreateResponseRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountCreateResponseRoleEnum {
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountCreateResponseRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectServiceAccountDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccountDeleteResponse {
    pub object: ProjectServiceAccountDeleteResponseObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectServiceAccountDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountDeleteResponseObjectEnum {
    #[serde(rename = "organization.project.service_account.deleted")]
    OrganizationProjectServiceAccountDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectServiceAccountListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectServiceAccountListResponse {
    pub object: ProjectServiceAccountListResponseObjectEnum,
    pub data: Vec<ProjectServiceAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectServiceAccountListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectServiceAccountObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountObjectEnum {
    #[serde(rename = "organization.project.service_account")]
    OrganizationProjectServiceAccount,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectServiceAccountRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectServiceAccountRoleEnum {
    #[serde(rename = "owner")]
    Owner,
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "none")]
    None,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectServiceAccountRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectSpendAlert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpendAlert {
    pub id: String,
    pub object: ProjectSpendAlertObjectEnum,
    pub threshold_amount: i64,
    pub currency: ProjectSpendAlertCurrencyEnum,
    pub interval: ProjectSpendAlertIntervalEnum,
    pub notification_channel: SpendAlertNotificationChannel,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectSpendAlertCurrencyEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendAlertCurrencyEnum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendAlertCurrencyEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectSpendAlertDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpendAlertDeletedResource {
    pub id: String,
    pub object: ProjectSpendAlertDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectSpendAlertDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendAlertDeletedResourceObjectEnum {
    #[serde(rename = "project.spend_alert.deleted")]
    ProjectSpendAlertDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendAlertDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectSpendAlertIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendAlertIntervalEnum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendAlertIntervalEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectSpendAlertListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpendAlertListResource {
    pub object: ProjectSpendAlertListResourceObjectEnum,
    pub data: Vec<ProjectSpendAlert>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectSpendAlertListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendAlertListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendAlertListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `ProjectSpendAlertObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendAlertObjectEnum {
    #[serde(rename = "project.spend_alert")]
    ProjectSpendAlert,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendAlertObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectSpendLimitDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpendLimitDeletedResource {
    pub object: ProjectSpendLimitDeletedResourceObjectEnum,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectSpendLimitDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendLimitDeletedResourceObjectEnum {
    #[serde(rename = "project.spend_limit.deleted")]
    ProjectSpendLimitDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendLimitDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectSpendLimitResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectSpendLimitResource {
    pub object: ProjectSpendLimitResourceObjectEnum,
    pub threshold_amount: i64,
    pub currency: SpendLimitCurrency,
    pub interval: SpendLimitInterval,
    pub enforcement: SpendLimitEnforcement,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectSpendLimitResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSpendLimitResourceObjectEnum {
    #[serde(rename = "project.spend_limit")]
    ProjectSpendLimit,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectSpendLimitResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geography: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUser {
    pub object: ProjectUserObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub role: String,
    pub added_at: i64,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectUserCreateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUserCreateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub role: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `ProjectUserDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUserDeleteResponse {
    pub object: ProjectUserDeleteResponseObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectUserDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectUserDeleteResponseObjectEnum {
    #[serde(rename = "organization.project.user.deleted")]
    OrganizationProjectUserDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectUserDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectUserListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUserListResponse {
    pub object: String,
    pub data: Vec<ProjectUser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ProjectUserObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectUserObjectEnum {
    #[serde(rename = "organization.project.user")]
    OrganizationProjectUser,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ProjectUserObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ProjectUserUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectUserUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `PublicAssignOrganizationGroupRoleBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PublicAssignOrganizationGroupRoleBody {
    pub role_id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `PublicCreateOrganizationRoleBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PublicCreateOrganizationRoleBody {
    pub role_name: String,
    pub permissions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `PublicRoleListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PublicRoleListResource {
    pub object: PublicRoleListResourceObjectEnum,
    pub data: Vec<Role>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `PublicRoleListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublicRoleListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for PublicRoleListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `PublicUpdateOrganizationRoleBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PublicUpdateOrganizationRoleBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_name: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `DELETE /organization/groups/{group_id}/users/{user_id}` (`remove-group-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoveGroupUserParams {
    pub group_id: String,
    pub user_id: String,
}

/// JSON result for `remove-group-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoveGroupUserResult {
    #[serde(flatten)]
    pub body: GroupUserDeletedResource,
}

/// Typed params for `DELETE /organization/projects/{project_id}/groups/{group_id}` (`remove-project-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoveProjectGroupParams {
    pub project_id: String,
    pub group_id: String,
}

/// JSON result for `remove-project-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RemoveProjectGroupResult {
    #[serde(flatten)]
    pub body: ProjectGroupDeletedResource,
}

/// Typed params for `GET /organization/groups/{group_id}` (`retrieve-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupParams {
    pub group_id: String,
}

/// JSON result for `retrieve-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupResult {
    #[serde(flatten)]
    pub body: GroupResponse,
}

/// Typed params for `GET /organization/groups/{group_id}/roles/{role_id}` (`retrieve-group-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupRoleParams {
    pub group_id: String,
    pub role_id: String,
}

/// JSON result for `retrieve-group-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupRoleResult {
    #[serde(flatten)]
    pub body: AssignedRoleDetails,
}

/// Typed params for `GET /organization/groups/{group_id}/users/{user_id}` (`retrieve-group-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupUserParams {
    pub group_id: String,
    pub user_id: String,
}

/// JSON result for `retrieve-group-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveGroupUserResult {
    #[serde(flatten)]
    pub body: GroupMemberUser,
}

/// Typed params for `GET /organization/invites/{invite_id}` (`retrieve-invite`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveInviteParams {
    pub invite_id: String,
}

/// JSON result for `retrieve-invite`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveInviteResult {
    #[serde(flatten)]
    pub body: Invite,
}

/// Typed params for `GET /organization/data_retention` (`retrieve-organization-data-retention`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveOrganizationDataRetentionParams {
}

/// JSON result for `retrieve-organization-data-retention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveOrganizationDataRetentionResult {
    #[serde(flatten)]
    pub body: OrganizationDataRetention,
}

/// Typed params for `GET /organization/spend_alerts/{alert_id}` (`retrieve-organization-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveOrganizationSpendAlertParams {
    pub alert_id: String,
}

/// JSON result for `retrieve-organization-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveOrganizationSpendAlertResult {
    #[serde(flatten)]
    pub body: OrganizationSpendAlert,
}

/// Typed params for `GET /organization/projects/{project_id}/api_keys/{api_key_id}` (`retrieve-project-api-key`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectApiKeyParams {
    pub project_id: String,
    pub api_key_id: String,
}

/// JSON result for `retrieve-project-api-key`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectApiKeyResult {
    #[serde(flatten)]
    pub body: ProjectApiKey,
}

/// Typed params for `GET /organization/projects/{project_id}/data_retention` (`retrieve-project-data-retention`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectDataRetentionParams {
    pub project_id: String,
}

/// JSON result for `retrieve-project-data-retention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectDataRetentionResult {
    #[serde(flatten)]
    pub body: ProjectDataRetention,
}

/// Typed params for `GET /organization/projects/{project_id}/groups/{group_id}` (`retrieve-project-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectGroupParams {
    pub project_id: String,
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_type: Option<RetrieveProjectGroupParamsGroupTypeEnum>,
}

/// Generated string enum `RetrieveProjectGroupParamsGroupTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrieveProjectGroupParamsGroupTypeEnum {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "tenant_group")]
    TenantGroup,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for RetrieveProjectGroupParamsGroupTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `retrieve-project-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectGroupResult {
    #[serde(flatten)]
    pub body: ProjectGroup,
}

/// Typed params for `GET /organization/projects/{project_id}/hosted_tool_permissions` (`retrieve-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectHostedToolPermissionsParams {
    pub project_id: String,
}

/// JSON result for `retrieve-project-hosted-tool-permissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectHostedToolPermissionsResult {
    #[serde(flatten)]
    pub body: ProjectHostedToolPermissions,
}

/// Typed params for `GET /organization/projects/{project_id}/model_permissions` (`retrieve-project-model-permissions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectModelPermissionsParams {
    pub project_id: String,
}

/// JSON result for `retrieve-project-model-permissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectModelPermissionsResult {
    #[serde(flatten)]
    pub body: ProjectModelPermissions,
}

/// Typed params for `GET /organization/projects/{project_id}` (`retrieve-project`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectParams {
    pub project_id: String,
}

/// JSON result for `retrieve-project`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectResult {
    #[serde(flatten)]
    pub body: Project,
}

/// Typed params for `GET /organization/projects/{project_id}/service_accounts/{service_account_id}` (`retrieve-project-service-account`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectServiceAccountParams {
    pub project_id: String,
    pub service_account_id: String,
}

/// JSON result for `retrieve-project-service-account`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectServiceAccountResult {
    #[serde(flatten)]
    pub body: ProjectServiceAccount,
}

/// Typed params for `GET /organization/projects/{project_id}/spend_alerts/{alert_id}` (`retrieve-project-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectSpendAlertParams {
    pub project_id: String,
    pub alert_id: String,
}

/// JSON result for `retrieve-project-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectSpendAlertResult {
    #[serde(flatten)]
    pub body: ProjectSpendAlert,
}

/// Typed params for `GET /organization/projects/{project_id}/users/{user_id}` (`retrieve-project-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectUserParams {
    pub project_id: String,
    pub user_id: String,
}

/// JSON result for `retrieve-project-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveProjectUserResult {
    #[serde(flatten)]
    pub body: ProjectUser,
}

/// Typed params for `GET /organization/roles/{role_id}` (`retrieve-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveRoleParams {
    pub role_id: String,
}

/// JSON result for `retrieve-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveRoleResult {
    #[serde(flatten)]
    pub body: Role,
}

/// Typed params for `GET /organization/users/{user_id}` (`retrieve-user`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveUserParams {
    pub user_id: String,
}

/// JSON result for `retrieve-user`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveUserResult {
    #[serde(flatten)]
    pub body: User,
}

/// Typed params for `GET /organization/users/{user_id}/roles/{role_id}` (`retrieve-user-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveUserRoleParams {
    pub user_id: String,
    pub role_id: String,
}

/// JSON result for `retrieve-user-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RetrieveUserRoleResult {
    #[serde(flatten)]
    pub body: AssignedRoleDetails,
}

/// Generated object `Role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Role {
    pub object: RoleObjectEnum,
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub permissions: Vec<String>,
    pub resource_type: String,
    pub predefined_role: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `RoleDeletedResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleDeletedResource {
    pub object: RoleDeletedResourceObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `RoleDeletedResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleDeletedResourceObjectEnum {
    #[serde(rename = "role.deleted")]
    RoleDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for RoleDeletedResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `RoleListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoleListResource {
    pub object: RoleListResourceObjectEnum,
    pub data: Vec<AssignedRoleDetails>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `RoleListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for RoleListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `RoleObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleObjectEnum {
    #[serde(rename = "role")]
    Role,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for RoleObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ServiceAccountApiKeyBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ServiceAccountApiKeyBody {
    pub object: ServiceAccountApiKeyBodyObjectEnum,
    pub value: String,
    pub name: String,
    pub created_at: i64,
    pub id: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `ServiceAccountApiKeyBodyObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAccountApiKeyBodyObjectEnum {
    #[serde(rename = "organization.project.service_account.api_key")]
    OrganizationProjectServiceAccountApiKey,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for ServiceAccountApiKeyBodyObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `SpendAlertNotificationChannel`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SpendAlertNotificationChannel {
    #[serde(rename = "type")]
    pub type_: SpendAlertNotificationChannelTypeEnum,
    pub recipients: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_prefix: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `SpendAlertNotificationChannelTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpendAlertNotificationChannelTypeEnum {
    #[serde(rename = "email")]
    Email,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for SpendAlertNotificationChannelTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated union `SpendLimitCurrency`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SpendLimitCurrency {
    Variant0(String),
    Variant1(SpendLimitCurrencyV1Enum),
    Unknown(serde_json::Value),
}
impl Default for SpendLimitCurrency { fn default() -> Self { Self::Unknown(serde_json::Value::Null) } }

/// Generated string enum `SpendLimitCurrencyV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpendLimitCurrencyV1Enum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for SpendLimitCurrencyV1Enum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `SpendLimitEnforcement`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SpendLimitEnforcement {
    pub status: SpendLimitEnforcementStatus,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated union `SpendLimitEnforcementStatus`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SpendLimitEnforcementStatus {
    Variant0(String),
    Variant1(SpendLimitEnforcementStatusV1Enum),
    Unknown(serde_json::Value),
}
impl Default for SpendLimitEnforcementStatus { fn default() -> Self { Self::Unknown(serde_json::Value::Null) } }

/// Generated string enum `SpendLimitEnforcementStatusV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpendLimitEnforcementStatusV1Enum {
    #[serde(rename = "inactive")]
    Inactive,
    #[serde(rename = "enforcing")]
    Enforcing,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for SpendLimitEnforcementStatusV1Enum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated union `SpendLimitInterval`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SpendLimitInterval {
    Variant0(String),
    Variant1(SpendLimitIntervalV1Enum),
    Unknown(serde_json::Value),
}
impl Default for SpendLimitInterval { fn default() -> Self { Self::Unknown(serde_json::Value::Null) } }

/// Generated string enum `SpendLimitIntervalV1Enum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpendLimitIntervalV1Enum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for SpendLimitIntervalV1Enum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `ToggleCertificatesRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToggleCertificatesRequest {
    pub certificate_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `DELETE /organization/groups/{group_id}/roles/{role_id}` (`unassign-group-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnassignGroupRoleParams {
    pub group_id: String,
    pub role_id: String,
}

/// JSON result for `unassign-group-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnassignGroupRoleResult {
    #[serde(flatten)]
    pub body: DeletedRoleAssignmentResource,
}

/// Typed params for `DELETE /organization/users/{user_id}/roles/{role_id}` (`unassign-user-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnassignUserRoleParams {
    pub user_id: String,
    pub role_id: String,
}

/// JSON result for `unassign-user-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UnassignUserRoleResult {
    #[serde(flatten)]
    pub body: DeletedRoleAssignmentResource,
}

/// Generated object `UpdateGroupBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateGroupBody {
    pub name: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Typed params for `POST /organization/groups/{group_id}` (`update-group`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateGroupParams {
    pub group_id: String,
    pub body: UpdateGroupBody,
}

/// JSON result for `update-group`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateGroupResult {
    #[serde(flatten)]
    pub body: GroupResourceWithSuccess,
}

/// Generated object `UpdateOrganizationDataRetentionBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationDataRetentionBody {
    pub retention_type: UpdateOrganizationDataRetentionBodyRetentionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateOrganizationDataRetentionBodyRetentionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateOrganizationDataRetentionBodyRetentionTypeEnum {
    #[serde(rename = "zero_data_retention")]
    ZeroDataRetention,
    #[serde(rename = "modified_abuse_monitoring")]
    ModifiedAbuseMonitoring,
    #[serde(rename = "enhanced_zero_data_retention")]
    EnhancedZeroDataRetention,
    #[serde(rename = "enhanced_modified_abuse_monitoring")]
    EnhancedModifiedAbuseMonitoring,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateOrganizationDataRetentionBodyRetentionTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/data_retention` (`update-organization-data-retention`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationDataRetentionParams {
    pub body: UpdateOrganizationDataRetentionBody,
}

/// JSON result for `update-organization-data-retention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationDataRetentionResult {
    #[serde(flatten)]
    pub body: OrganizationDataRetention,
}

/// Typed params for `POST /organization/spend_alerts/{alert_id}` (`update-organization-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationSpendAlertParams {
    pub alert_id: String,
    pub body: CreateSpendAlertBody,
}

/// JSON result for `update-organization-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationSpendAlertResult {
    #[serde(flatten)]
    pub body: OrganizationSpendAlert,
}

/// Generated object `UpdateOrganizationSpendLimitBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateOrganizationSpendLimitBody {
    pub threshold_amount: i64,
    pub currency: UpdateOrganizationSpendLimitBodyCurrencyEnum,
    pub interval: UpdateOrganizationSpendLimitBodyIntervalEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateOrganizationSpendLimitBodyCurrencyEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateOrganizationSpendLimitBodyCurrencyEnum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateOrganizationSpendLimitBodyCurrencyEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UpdateOrganizationSpendLimitBodyIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateOrganizationSpendLimitBodyIntervalEnum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateOrganizationSpendLimitBodyIntervalEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UpdateProjectDataRetentionBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectDataRetentionBody {
    pub retention_type: UpdateProjectDataRetentionBodyRetentionTypeEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateProjectDataRetentionBodyRetentionTypeEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateProjectDataRetentionBodyRetentionTypeEnum {
    #[serde(rename = "organization_default")]
    OrganizationDefault,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "zero_data_retention")]
    ZeroDataRetention,
    #[serde(rename = "modified_abuse_monitoring")]
    ModifiedAbuseMonitoring,
    #[serde(rename = "enhanced_zero_data_retention")]
    EnhancedZeroDataRetention,
    #[serde(rename = "enhanced_modified_abuse_monitoring")]
    EnhancedModifiedAbuseMonitoring,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateProjectDataRetentionBodyRetentionTypeEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/projects/{project_id}/data_retention` (`update-project-data-retention`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectDataRetentionParams {
    pub project_id: String,
    pub body: UpdateProjectDataRetentionBody,
}

/// JSON result for `update-project-data-retention`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectDataRetentionResult {
    #[serde(flatten)]
    pub body: ProjectDataRetention,
}

/// Typed params for `POST /organization/projects/{project_id}/hosted_tool_permissions` (`update-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectHostedToolPermissionsParams {
    pub project_id: String,
    pub body: ProjectHostedToolPermissionsUpdateRequest,
}

/// JSON result for `update-project-hosted-tool-permissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectHostedToolPermissionsResult {
    #[serde(flatten)]
    pub body: ProjectHostedToolPermissions,
}

/// Typed params for `POST /organization/projects/{project_id}/model_permissions` (`update-project-model-permissions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectModelPermissionsParams {
    pub project_id: String,
    pub body: ProjectModelPermissionsUpdateRequest,
}

/// JSON result for `update-project-model-permissions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectModelPermissionsResult {
    #[serde(flatten)]
    pub body: ProjectModelPermissions,
}

/// Typed params for `POST /organization/projects/{project_id}/rate_limits/{rate_limit_id}` (`update-project-rate-limits`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectRateLimitsParams {
    pub project_id: String,
    pub rate_limit_id: String,
    pub body: ProjectRateLimitUpdateRequest,
}

/// JSON result for `update-project-rate-limits`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectRateLimitsResult {
    #[serde(flatten)]
    pub body: ProjectRateLimit,
}

/// Generated object `UpdateProjectServiceAccountBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectServiceAccountBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<UpdateProjectServiceAccountBodyRoleEnum>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateProjectServiceAccountBodyRoleEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateProjectServiceAccountBodyRoleEnum {
    #[serde(rename = "member")]
    Member,
    #[serde(rename = "owner")]
    Owner,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateProjectServiceAccountBodyRoleEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}` (`update-project-service-account`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectServiceAccountParams {
    pub project_id: String,
    pub service_account_id: String,
    pub body: UpdateProjectServiceAccountBody,
}

/// JSON result for `update-project-service-account`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectServiceAccountResult {
    #[serde(flatten)]
    pub body: ProjectServiceAccount,
}

/// Typed params for `POST /organization/projects/{project_id}/spend_alerts/{alert_id}` (`update-project-spend-alert`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectSpendAlertParams {
    pub project_id: String,
    pub alert_id: String,
    pub body: CreateSpendAlertBody,
}

/// JSON result for `update-project-spend-alert`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectSpendAlertResult {
    #[serde(flatten)]
    pub body: ProjectSpendAlert,
}

/// Generated object `UpdateProjectSpendLimitBody`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateProjectSpendLimitBody {
    pub threshold_amount: i64,
    pub currency: UpdateProjectSpendLimitBodyCurrencyEnum,
    pub interval: UpdateProjectSpendLimitBodyIntervalEnum,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UpdateProjectSpendLimitBodyCurrencyEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateProjectSpendLimitBodyCurrencyEnum {
    #[serde(rename = "USD")]
    USD,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateProjectSpendLimitBodyCurrencyEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UpdateProjectSpendLimitBodyIntervalEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateProjectSpendLimitBodyIntervalEnum {
    #[serde(rename = "month")]
    Month,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UpdateProjectSpendLimitBodyIntervalEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `POST /organization/roles/{role_id}` (`update-role`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateRoleParams {
    pub role_id: String,
    pub body: PublicUpdateOrganizationRoleBody,
}

/// JSON result for `update-role`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateRoleResult {
    #[serde(flatten)]
    pub body: Role,
}

/// Typed params for `POST /organization/spend_limit` (`Updateorganizationspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateorganizationspendlimitParams {
    pub body: UpdateOrganizationSpendLimitBody,
}

/// JSON result for `Updateorganizationspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateorganizationspendlimitResult {
    #[serde(flatten)]
    pub body: OrganizationSpendLimitResource,
}

/// Typed params for `POST /organization/projects/{project_id}/spend_limit` (`Updateprojectspendlimit`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateprojectspendlimitParams {
    pub project_id: String,
    pub body: UpdateProjectSpendLimitBody,
}

/// JSON result for `Updateprojectspendlimit`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateprojectspendlimitResult {
    #[serde(flatten)]
    pub body: ProjectSpendLimitResource,
}

/// Typed params for `POST /organization/certificates` (`uploadCertificate`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadCertificateParams {
    pub body: UploadCertificateRequest,
}

/// Generated object `UploadCertificateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadCertificateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub certificate: String,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// JSON result for `uploadCertificate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UploadCertificateResult {
    #[serde(flatten)]
    pub body: Certificate,
}

/// Typed params for `GET /organization/usage/audio_speeches` (`usage-audio-speeches`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioSpeechesParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageAudioSpeechesParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageAudioSpeechesParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageAudioSpeechesParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioSpeechesParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioSpeechesParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageAudioSpeechesParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioSpeechesParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioSpeechesParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-audio-speeches`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioSpeechesResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageAudioSpeechesResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioSpeechesResult2 {
    pub object: UsageAudioSpeechesResult2ObjectEnum,
    pub characters: i64,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageAudioSpeechesResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioSpeechesResult2ObjectEnum {
    #[serde(rename = "organization.usage.audio_speeches.result")]
    OrganizationUsageAudioSpeechesResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioSpeechesResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/audio_transcriptions` (`usage-audio-transcriptions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioTranscriptionsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageAudioTranscriptionsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageAudioTranscriptionsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageAudioTranscriptionsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioTranscriptionsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioTranscriptionsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageAudioTranscriptionsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioTranscriptionsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioTranscriptionsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-audio-transcriptions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioTranscriptionsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageAudioTranscriptionsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageAudioTranscriptionsResult2 {
    pub object: UsageAudioTranscriptionsResult2ObjectEnum,
    pub seconds: i64,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageAudioTranscriptionsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageAudioTranscriptionsResult2ObjectEnum {
    #[serde(rename = "organization.usage.audio_transcriptions.result")]
    OrganizationUsageAudioTranscriptionsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageAudioTranscriptionsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/code_interpreter_sessions` (`usage-code-interpreter-sessions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCodeInterpreterSessionsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageCodeInterpreterSessionsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageCodeInterpreterSessionsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageCodeInterpreterSessionsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCodeInterpreterSessionsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCodeInterpreterSessionsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageCodeInterpreterSessionsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCodeInterpreterSessionsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCodeInterpreterSessionsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-code-interpreter-sessions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCodeInterpreterSessionsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageCodeInterpreterSessionsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCodeInterpreterSessionsResult2 {
    pub object: UsageCodeInterpreterSessionsResult2ObjectEnum,
    pub num_sessions: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageCodeInterpreterSessionsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCodeInterpreterSessionsResult2ObjectEnum {
    #[serde(rename = "organization.usage.code_interpreter_sessions.result")]
    OrganizationUsageCodeInterpreterSessionsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCodeInterpreterSessionsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/completions` (`usage-completions`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCompletionsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageCompletionsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageCompletionsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageCompletionsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCompletionsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCompletionsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageCompletionsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCompletionsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "batch")]
    Batch,
    #[serde(rename = "service_tier")]
    ServiceTier,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCompletionsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-completions`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCompletionsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageCompletionsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCompletionsResult2 {
    pub object: UsageCompletionsResult2ObjectEnum,
    pub input_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cached_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cache_write_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_uncached_tokens: Option<i64>,
    pub output_tokens: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_text_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_text_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cached_text_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cached_audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_audio_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_image_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_cached_image_tokens: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_image_tokens: Option<i64>,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageCompletionsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCompletionsResult2ObjectEnum {
    #[serde(rename = "organization.usage.completions.result")]
    OrganizationUsageCompletionsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCompletionsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/costs` (`usage-costs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCostsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageCostsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageCostsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageCostsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCostsParamsBucketWidthEnum {
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCostsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageCostsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageCostsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "line_item")]
    LineItem,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageCostsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-costs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageCostsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Typed params for `GET /organization/usage/embeddings` (`usage-embeddings`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageEmbeddingsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageEmbeddingsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageEmbeddingsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageEmbeddingsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageEmbeddingsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageEmbeddingsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageEmbeddingsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageEmbeddingsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageEmbeddingsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-embeddings`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageEmbeddingsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageEmbeddingsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageEmbeddingsResult2 {
    pub object: UsageEmbeddingsResult2ObjectEnum,
    pub input_tokens: i64,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageEmbeddingsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageEmbeddingsResult2ObjectEnum {
    #[serde(rename = "organization.usage.embeddings.result")]
    OrganizationUsageEmbeddingsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageEmbeddingsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/file_search_calls` (`usage-file-search-calls`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageFileSearchCallsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageFileSearchCallsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_store_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageFileSearchCallsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageFileSearchCallsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageFileSearchCallsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageFileSearchCallsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageFileSearchCallsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageFileSearchCallsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "vector_store_id")]
    VectorStoreId,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageFileSearchCallsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-file-search-calls`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageFileSearchCallsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageFileSearchCallsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageFileSearchCallsResult2 {
    pub object: UsageFileSearchCallsResult2ObjectEnum,
    pub num_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector_store_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageFileSearchCallsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageFileSearchCallsResult2ObjectEnum {
    #[serde(rename = "organization.usage.file_searches.result")]
    OrganizationUsageFileSearchesResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageFileSearchCallsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/images` (`usage-images`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageImagesParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageImagesParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<UsageImagesParamsSourcesItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<UsageImagesParamsSizesItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageImagesParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageImagesParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageImagesParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageImagesParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageImagesParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageImagesParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "size")]
    Size,
    #[serde(rename = "source")]
    Source,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageImagesParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageImagesParamsSizesItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageImagesParamsSizesItemEnum {
    #[serde(rename = "256x256")]
    T256x256,
    #[serde(rename = "512x512")]
    T512x512,
    #[serde(rename = "1024x1024")]
    T1024x1024,
    #[serde(rename = "1792x1792")]
    T1792x1792,
    #[serde(rename = "1024x1792")]
    T1024x1792,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageImagesParamsSizesItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageImagesParamsSourcesItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageImagesParamsSourcesItemEnum {
    #[serde(rename = "image.generation")]
    ImageGeneration,
    #[serde(rename = "image.edit")]
    ImageEdit,
    #[serde(rename = "image.variation")]
    ImageVariation,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageImagesParamsSourcesItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-images`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageImagesResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageImagesResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageImagesResult2 {
    pub object: UsageImagesResult2ObjectEnum,
    pub images: i64,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageImagesResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageImagesResult2ObjectEnum {
    #[serde(rename = "organization.usage.images.result")]
    OrganizationUsageImagesResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageImagesResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/moderations` (`usage-moderations`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageModerationsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageModerationsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageModerationsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageModerationsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageModerationsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageModerationsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageModerationsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageModerationsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageModerationsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-moderations`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageModerationsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageModerationsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageModerationsResult2 {
    pub object: UsageModerationsResult2ObjectEnum,
    pub input_tokens: i64,
    pub num_model_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageModerationsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageModerationsResult2ObjectEnum {
    #[serde(rename = "organization.usage.moderations.result")]
    OrganizationUsageModerationsResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageModerationsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UsageResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageResponse {
    pub object: UsageResponseObjectEnum,
    pub data: Vec<UsageTimeBucket>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_page: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageResponseObjectEnum {
    #[serde(rename = "page")]
    Page,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UsageTimeBucket`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageTimeBucket {
    pub object: UsageTimeBucketObjectEnum,
    pub start_time: i64,
    pub end_time: i64,
    pub results: Vec<UsageTimeBucketResultsItemUnion>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageTimeBucketObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageTimeBucketObjectEnum {
    #[serde(rename = "bucket")]
    Bucket,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageTimeBucketObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated union `UsageTimeBucketResultsItemUnion`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UsageTimeBucketResultsItemUnion {
    Variant0(UsageCompletionsResult2),
    Variant1(UsageEmbeddingsResult2),
    Variant2(UsageModerationsResult2),
    Variant3(UsageImagesResult2),
    Variant4(UsageAudioSpeechesResult2),
    Variant5(UsageAudioTranscriptionsResult2),
    Variant6(UsageVectorStoresResult2),
    Variant7(UsageCodeInterpreterSessionsResult2),
    Variant8(UsageFileSearchCallsResult2),
    Variant9(UsageWebSearchCallsResult2),
    Variant10(CostsResult),
    Unknown(serde_json::Value),
}
impl Default for UsageTimeBucketResultsItemUnion { fn default() -> Self { Self::Unknown(serde_json::Value::Null) } }

/// Typed params for `GET /organization/usage/vector_stores` (`usage-vector-stores`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageVectorStoresParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageVectorStoresParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageVectorStoresParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageVectorStoresParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageVectorStoresParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageVectorStoresParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageVectorStoresParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageVectorStoresParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageVectorStoresParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-vector-stores`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageVectorStoresResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageVectorStoresResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageVectorStoresResult2 {
    pub object: UsageVectorStoresResult2ObjectEnum,
    pub usage_bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageVectorStoresResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageVectorStoresResult2ObjectEnum {
    #[serde(rename = "organization.usage.vector_stores.result")]
    OrganizationUsageVectorStoresResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageVectorStoresResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Typed params for `GET /organization/usage/web_search_calls` (`usage-web-search-calls`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageWebSearchCallsParams {
    pub start_time: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_width: Option<UsageWebSearchCallsParamsBucketWidthEnum>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_levels: Option<Vec<UsageWebSearchCallsParamsContextLevelsItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Vec<UsageWebSearchCallsParamsGroupByItemEnum>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

/// Generated string enum `UsageWebSearchCallsParamsBucketWidthEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageWebSearchCallsParamsBucketWidthEnum {
    #[serde(rename = "1m")]
    T1m,
    #[serde(rename = "1h")]
    T1h,
    #[serde(rename = "1d")]
    T1d,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageWebSearchCallsParamsBucketWidthEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageWebSearchCallsParamsContextLevelsItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageWebSearchCallsParamsContextLevelsItemEnum {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageWebSearchCallsParamsContextLevelsItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UsageWebSearchCallsParamsGroupByItemEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageWebSearchCallsParamsGroupByItemEnum {
    #[serde(rename = "project_id")]
    ProjectId,
    #[serde(rename = "user_id")]
    UserId,
    #[serde(rename = "api_key_id")]
    ApiKeyId,
    #[serde(rename = "model")]
    Model,
    #[serde(rename = "context_level")]
    ContextLevel,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageWebSearchCallsParamsGroupByItemEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// JSON result for `usage-web-search-calls`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageWebSearchCallsResult {
    #[serde(flatten)]
    pub body: UsageResponse,
}

/// Generated object `UsageWebSearchCallsResult2`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageWebSearchCallsResult2 {
    pub object: UsageWebSearchCallsResult2ObjectEnum,
    pub num_model_requests: i64,
    pub num_requests: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_level: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UsageWebSearchCallsResult2ObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageWebSearchCallsResult2ObjectEnum {
    #[serde(rename = "organization.usage.web_searches.result")]
    OrganizationUsageWebSearchesResult,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UsageWebSearchCallsResult2ObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `User`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub object: UserObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<UserUser>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_service_account: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_scale_tier_authorized_purchaser: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_scim_managed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_last_used_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technical_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_persona: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projects: Option<UserProjects>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UserDeleteResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserDeleteResponse {
    pub object: UserDeleteResponseObjectEnum,
    pub id: String,
    pub deleted: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserDeleteResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserDeleteResponseObjectEnum {
    #[serde(rename = "organization.user.deleted")]
    OrganizationUserDeleted,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserDeleteResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UserListResource`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserListResource {
    pub object: UserListResourceObjectEnum,
    pub data: Vec<GroupUser>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserListResourceObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserListResourceObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserListResourceObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UserListResponse`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserListResponse {
    pub object: UserListResponseObjectEnum,
    pub data: Vec<User>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    pub has_more: bool,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserListResponseObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserListResponseObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserListResponseObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated string enum `UserObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserObjectEnum {
    #[serde(rename = "organization.user")]
    OrganizationUser,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UserProjects`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserProjects {
    pub object: UserProjectsObjectEnum,
    pub data: Vec<UserProjectsDataItem>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UserProjectsDataItem`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserProjectsDataItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserProjectsObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserProjectsObjectEnum {
    #[serde(rename = "list")]
    List,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserProjectsObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UserRoleAssignment`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserRoleAssignment {
    pub object: UserRoleAssignmentObjectEnum,
    pub user: User,
    pub role: Role,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserRoleAssignmentObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRoleAssignmentObjectEnum {
    #[serde(rename = "user.role")]
    UserRole,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserRoleAssignmentObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }

/// Generated object `UserRoleUpdateRequest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserRoleUpdateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technical_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer_persona: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated object `UserUser`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UserUser {
    pub object: UserUserObjectEnum,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banned_at: Option<i64>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Generated string enum `UserUserObjectEnum`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserUserObjectEnum {
    #[serde(rename = "user")]
    User,
    #[serde(untagged)]
    Unknown(String),
}
impl Default for UserUserObjectEnum { fn default() -> Self { Self::Unknown(String::new()) } }
