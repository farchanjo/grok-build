//! Generated typed operations for openai_admin platform baseline.
//! DO NOT EDIT BY HAND — regenerate via baselines/scripts/generate_platform_client.py

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec, PlatformTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Request for `GET /organization/admin_api_keys` (`admin-api-keys-list`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/admin_api_keys` (`admin-api-keys-list`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysListResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/admin_api_keys` (`admin-api-keys-create`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysCreateRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AdminApiKeysCreateBody,
}

/// Body for `admin-api-keys-create`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysCreateBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AdminApiKeysCreateBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/admin_api_keys` (`admin-api-keys-create`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysCreateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/admin_api_keys/{key_id}` (`admin-api-keys-delete`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysDeleteRequest {
    pub key_id: String,
}

/// Response for `DELETE /organization/admin_api_keys/{key_id}` (`admin-api-keys-delete`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysDeleteResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/admin_api_keys/{key_id}` (`admin-api-keys-get`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysGetRequest {
    pub key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/admin_api_keys/{key_id}` (`admin-api-keys-get`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AdminApiKeysGetResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/audit_logs` (`list-audit-logs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListAuditLogsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/audit_logs` (`list-audit-logs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListAuditLogsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/certificates` (`listOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationCertificatesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/certificates` (`listOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/certificates` (`uploadCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadCertificateRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UploadCertificateBody,
}

/// Body for `uploadCertificate`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadCertificateBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UploadCertificateBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/certificates` (`uploadCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UploadCertificateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/certificates/activate` (`activateOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateOrganizationCertificatesRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ActivateOrganizationCertificatesBody,
}

/// Body for `activateOrganizationCertificates`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateOrganizationCertificatesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ActivateOrganizationCertificatesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/certificates/activate` (`activateOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateOrganizationCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/certificates/deactivate` (`deactivateOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateOrganizationCertificatesRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: DeactivateOrganizationCertificatesBody,
}

/// Body for `deactivateOrganizationCertificates`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateOrganizationCertificatesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl DeactivateOrganizationCertificatesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/certificates/deactivate` (`deactivateOrganizationCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateOrganizationCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/certificates/{certificate_id}` (`deleteCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteCertificateRequest {
    pub certificate_id: String,
}

/// Response for `DELETE /organization/certificates/{certificate_id}` (`deleteCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteCertificateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/certificates/{certificate_id}` (`getCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCertificateRequest {
    pub certificate_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/certificates/{certificate_id}` (`getCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetCertificateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/certificates/{certificate_id}` (`modifyCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyCertificateRequest {
    pub certificate_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyCertificateBody,
}

/// Body for `modifyCertificate`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyCertificateBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyCertificateBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/certificates/{certificate_id}` (`modifyCertificate`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyCertificateResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/costs` (`usage-costs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCostsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/costs` (`usage-costs`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCostsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/data_retention` (`retrieve-organization-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveOrganizationDataRetentionRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/data_retention` (`retrieve-organization-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveOrganizationDataRetentionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/data_retention` (`update-organization-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationDataRetentionRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateOrganizationDataRetentionBody,
}

/// Body for `update-organization-data-retention`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationDataRetentionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateOrganizationDataRetentionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/data_retention` (`update-organization-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationDataRetentionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups` (`list-groups`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups` (`list-groups`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/groups` (`create-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGroupRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateGroupBody,
}

/// Body for `create-group`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGroupBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateGroupBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/groups` (`create-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/groups/{group_id}` (`delete-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteGroupRequest {
    pub group_id: String,
}

/// Response for `DELETE /organization/groups/{group_id}` (`delete-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups/{group_id}` (`retrieve-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupRequest {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups/{group_id}` (`retrieve-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/groups/{group_id}` (`update-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGroupRequest {
    pub group_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateGroupBody,
}

/// Body for `update-group`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGroupBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateGroupBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/groups/{group_id}` (`update-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups/{group_id}/roles` (`list-group-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupRoleAssignmentsRequest {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups/{group_id}/roles` (`list-group-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupRoleAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/groups/{group_id}/roles` (`assign-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignGroupRoleRequest {
    pub group_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AssignGroupRoleBody,
}

/// Body for `assign-group-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignGroupRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AssignGroupRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/groups/{group_id}/roles` (`assign-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/groups/{group_id}/roles/{role_id}` (`unassign-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignGroupRoleRequest {
    pub group_id: String,
    pub role_id: String,
}

/// Response for `DELETE /organization/groups/{group_id}/roles/{role_id}` (`unassign-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups/{group_id}/roles/{role_id}` (`retrieve-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupRoleRequest {
    pub group_id: String,
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups/{group_id}/roles/{role_id}` (`retrieve-group-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups/{group_id}/users` (`list-group-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupUsersRequest {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups/{group_id}/users` (`list-group-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListGroupUsersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/groups/{group_id}/users` (`add-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddGroupUserRequest {
    pub group_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AddGroupUserBody,
}

/// Body for `add-group-user`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddGroupUserBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AddGroupUserBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/groups/{group_id}/users` (`add-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddGroupUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/groups/{group_id}/users/{user_id}` (`remove-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoveGroupUserRequest {
    pub group_id: String,
    pub user_id: String,
}

/// Response for `DELETE /organization/groups/{group_id}/users/{user_id}` (`remove-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoveGroupUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/groups/{group_id}/users/{user_id}` (`retrieve-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupUserRequest {
    pub group_id: String,
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/groups/{group_id}/users/{user_id}` (`retrieve-group-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveGroupUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/invites` (`list-invites`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListInvitesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/invites` (`list-invites`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListInvitesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/invites` (`inviteUser`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InviteUserRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: InviteUserBody,
}

/// Body for `inviteUser`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InviteUserBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl InviteUserBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/invites` (`inviteUser`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct InviteUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/invites/{invite_id}` (`delete-invite`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteInviteRequest {
    pub invite_id: String,
}

/// Response for `DELETE /organization/invites/{invite_id}` (`delete-invite`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteInviteResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/invites/{invite_id}` (`retrieve-invite`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveInviteRequest {
    pub invite_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/invites/{invite_id}` (`retrieve-invite`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveInviteResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects` (`list-projects`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects` (`list-projects`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects` (`create-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateProjectBody,
}

/// Body for `create-project`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateProjectBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects` (`create-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}` (`retrieve-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}` (`retrieve-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}` (`modify-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyProjectBody,
}

/// Body for `modify-project`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyProjectBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}` (`modify-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/api_keys` (`list-project-api-keys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectApiKeysRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/api_keys` (`list-project-api-keys`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectApiKeysResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/api_keys/{api_key_id}` (`delete-project-api-key`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectApiKeyRequest {
    pub project_id: String,
    pub api_key_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/api_keys/{api_key_id}` (`delete-project-api-key`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectApiKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/api_keys/{api_key_id}` (`retrieve-project-api-key`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectApiKeyRequest {
    pub project_id: String,
    pub api_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/api_keys/{api_key_id}` (`retrieve-project-api-key`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectApiKeyResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/archive` (`archive-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ArchiveProjectRequest {
    pub project_id: String,
}

/// Response for `POST /organization/projects/{project_id}/archive` (`archive-project`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ArchiveProjectResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/certificates` (`listProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectCertificatesRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/certificates` (`listProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/certificates/activate` (`activateProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateProjectCertificatesRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ActivateProjectCertificatesBody,
}

/// Body for `activateProjectCertificates`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateProjectCertificatesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ActivateProjectCertificatesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/certificates/activate` (`activateProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ActivateProjectCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/certificates/deactivate` (`deactivateProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateProjectCertificatesRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: DeactivateProjectCertificatesBody,
}

/// Body for `deactivateProjectCertificates`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateProjectCertificatesBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl DeactivateProjectCertificatesBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/certificates/deactivate` (`deactivateProjectCertificates`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeactivateProjectCertificatesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/data_retention` (`retrieve-project-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectDataRetentionRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/data_retention` (`retrieve-project-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectDataRetentionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/data_retention` (`update-project-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectDataRetentionRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectDataRetentionBody,
}

/// Body for `update-project-data-retention`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectDataRetentionBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectDataRetentionBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/data_retention` (`update-project-data-retention`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectDataRetentionResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/groups` (`list-project-groups`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectGroupsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/groups` (`list-project-groups`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectGroupsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/groups` (`add-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddProjectGroupRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AddProjectGroupBody,
}

/// Body for `add-project-group`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddProjectGroupBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AddProjectGroupBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/groups` (`add-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AddProjectGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/groups/{group_id}` (`remove-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoveProjectGroupRequest {
    pub project_id: String,
    pub group_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/groups/{group_id}` (`remove-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RemoveProjectGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/groups/{group_id}` (`retrieve-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectGroupRequest {
    pub project_id: String,
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/groups/{group_id}` (`retrieve-project-group`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectGroupResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/hosted_tool_permissions` (`retrieve-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectHostedToolPermissionsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/hosted_tool_permissions` (`retrieve-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectHostedToolPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/hosted_tool_permissions` (`update-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectHostedToolPermissionsRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectHostedToolPermissionsBody,
}

/// Body for `update-project-hosted-tool-permissions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectHostedToolPermissionsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectHostedToolPermissionsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/hosted_tool_permissions` (`update-project-hosted-tool-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectHostedToolPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/model_permissions` (`delete-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectModelPermissionsRequest {
    pub project_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/model_permissions` (`delete-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectModelPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/model_permissions` (`retrieve-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectModelPermissionsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/model_permissions` (`retrieve-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectModelPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/model_permissions` (`update-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectModelPermissionsRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectModelPermissionsBody,
}

/// Body for `update-project-model-permissions`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectModelPermissionsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectModelPermissionsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/model_permissions` (`update-project-model-permissions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectModelPermissionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/rate_limits` (`list-project-rate-limits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectRateLimitsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/rate_limits` (`list-project-rate-limits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectRateLimitsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/rate_limits/{rate_limit_id}` (`update-project-rate-limits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRateLimitsRequest {
    pub project_id: String,
    pub rate_limit_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectRateLimitsBody,
}

/// Body for `update-project-rate-limits`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRateLimitsBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectRateLimitsBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/rate_limits/{rate_limit_id}` (`update-project-rate-limits`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectRateLimitsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/service_accounts` (`list-project-service-accounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectServiceAccountsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/service_accounts` (`list-project-service-accounts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectServiceAccountsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/service_accounts` (`create-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectServiceAccountRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateProjectServiceAccountBody,
}

/// Body for `create-project-service-account`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectServiceAccountBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateProjectServiceAccountBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/service_accounts` (`create-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectServiceAccountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/service_accounts/{service_account_id}` (`delete-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectServiceAccountRequest {
    pub project_id: String,
    pub service_account_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/service_accounts/{service_account_id}` (`delete-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectServiceAccountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/service_accounts/{service_account_id}` (`retrieve-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectServiceAccountRequest {
    pub project_id: String,
    pub service_account_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/service_accounts/{service_account_id}` (`retrieve-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectServiceAccountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}` (`update-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectServiceAccountRequest {
    pub project_id: String,
    pub service_account_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectServiceAccountBody,
}

/// Body for `update-project-service-account`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectServiceAccountBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectServiceAccountBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}` (`update-project-service-account`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectServiceAccountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys` (`CreateanAPIkeyforaserviceaccount`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateanAPIkeyforaserviceaccountRequest {
    pub project_id: String,
    pub service_account_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateanAPIkeyforaserviceaccountBody,
}

/// Body for `CreateanAPIkeyforaserviceaccount`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateanAPIkeyforaserviceaccountBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateanAPIkeyforaserviceaccountBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys` (`CreateanAPIkeyforaserviceaccount`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateanAPIkeyforaserviceaccountResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/spend_alerts` (`list-project-spend-alerts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectSpendAlertsRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/spend_alerts` (`list-project-spend-alerts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectSpendAlertsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/spend_alerts` (`create-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectSpendAlertRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateProjectSpendAlertBody,
}

/// Body for `create-project-spend-alert`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectSpendAlertBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateProjectSpendAlertBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/spend_alerts` (`create-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/spend_alerts/{alert_id}` (`delete-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectSpendAlertRequest {
    pub project_id: String,
    pub alert_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/spend_alerts/{alert_id}` (`delete-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/spend_alerts/{alert_id}` (`retrieve-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectSpendAlertRequest {
    pub project_id: String,
    pub alert_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/spend_alerts/{alert_id}` (`retrieve-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/spend_alerts/{alert_id}` (`update-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectSpendAlertRequest {
    pub project_id: String,
    pub alert_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateProjectSpendAlertBody,
}

/// Body for `update-project-spend-alert`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectSpendAlertBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateProjectSpendAlertBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/spend_alerts/{alert_id}` (`update-project-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateProjectSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/spend_limit` (`Deleteprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteprojectspendlimitRequest {
    pub project_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/spend_limit` (`Deleteprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteprojectspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/spend_limit` (`Getprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetprojectspendlimitRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/spend_limit` (`Getprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetprojectspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/spend_limit` (`Updateprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateprojectspendlimitRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateprojectspendlimitBody,
}

/// Body for `Updateprojectspendlimit`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateprojectspendlimitBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateprojectspendlimitBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/spend_limit` (`Updateprojectspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateprojectspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/users` (`list-project-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectUsersRequest {
    pub project_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/users` (`list-project-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListProjectUsersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/users` (`create-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectUserRequest {
    pub project_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateProjectUserBody,
}

/// Body for `create-project-user`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectUserBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateProjectUserBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/users` (`create-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateProjectUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/projects/{project_id}/users/{user_id}` (`delete-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectUserRequest {
    pub project_id: String,
    pub user_id: String,
}

/// Response for `DELETE /organization/projects/{project_id}/users/{user_id}` (`delete-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteProjectUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/projects/{project_id}/users/{user_id}` (`retrieve-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectUserRequest {
    pub project_id: String,
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/projects/{project_id}/users/{user_id}` (`retrieve-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveProjectUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/projects/{project_id}/users/{user_id}` (`modify-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectUserRequest {
    pub project_id: String,
    pub user_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyProjectUserBody,
}

/// Body for `modify-project-user`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectUserBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyProjectUserBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/projects/{project_id}/users/{user_id}` (`modify-project-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyProjectUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/roles` (`list-roles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRolesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/roles` (`list-roles`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListRolesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/roles` (`create-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRoleRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateRoleBody,
}

/// Body for `create-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/roles` (`create-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/roles/{role_id}` (`delete-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteRoleRequest {
    pub role_id: String,
}

/// Response for `DELETE /organization/roles/{role_id}` (`delete-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/roles/{role_id}` (`retrieve-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveRoleRequest {
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/roles/{role_id}` (`retrieve-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/roles/{role_id}` (`update-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateRoleRequest {
    pub role_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateRoleBody,
}

/// Body for `update-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/roles/{role_id}` (`update-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/spend_alerts` (`list-organization-spend-alerts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationSpendAlertsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/spend_alerts` (`list-organization-spend-alerts`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListOrganizationSpendAlertsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/spend_alerts` (`create-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateOrganizationSpendAlertRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: CreateOrganizationSpendAlertBody,
}

/// Body for `create-organization-spend-alert`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateOrganizationSpendAlertBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl CreateOrganizationSpendAlertBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/spend_alerts` (`create-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CreateOrganizationSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/spend_alerts/{alert_id}` (`delete-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteOrganizationSpendAlertRequest {
    pub alert_id: String,
}

/// Response for `DELETE /organization/spend_alerts/{alert_id}` (`delete-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteOrganizationSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/spend_alerts/{alert_id}` (`retrieve-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveOrganizationSpendAlertRequest {
    pub alert_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/spend_alerts/{alert_id}` (`retrieve-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveOrganizationSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/spend_alerts/{alert_id}` (`update-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationSpendAlertRequest {
    pub alert_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateOrganizationSpendAlertBody,
}

/// Body for `update-organization-spend-alert`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationSpendAlertBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateOrganizationSpendAlertBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/spend_alerts/{alert_id}` (`update-organization-spend-alert`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateOrganizationSpendAlertResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/spend_limit` (`Deleteorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteorganizationspendlimitRequest {
}

/// Response for `DELETE /organization/spend_limit` (`Deleteorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteorganizationspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/spend_limit` (`Getorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetorganizationspendlimitRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/spend_limit` (`Getorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GetorganizationspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/spend_limit` (`Updateorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateorganizationspendlimitRequest {
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: UpdateorganizationspendlimitBody,
}

/// Body for `Updateorganizationspendlimit`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateorganizationspendlimitBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl UpdateorganizationspendlimitBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/spend_limit` (`Updateorganizationspendlimit`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UpdateorganizationspendlimitResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/audio_speeches` (`usage-audio-speeches`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageAudioSpeechesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/audio_speeches` (`usage-audio-speeches`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageAudioSpeechesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/audio_transcriptions` (`usage-audio-transcriptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageAudioTranscriptionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/audio_transcriptions` (`usage-audio-transcriptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageAudioTranscriptionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/code_interpreter_sessions` (`usage-code-interpreter-sessions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCodeInterpreterSessionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/code_interpreter_sessions` (`usage-code-interpreter-sessions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCodeInterpreterSessionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/completions` (`usage-completions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCompletionsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/completions` (`usage-completions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageCompletionsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/embeddings` (`usage-embeddings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageEmbeddingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/embeddings` (`usage-embeddings`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageEmbeddingsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/file_search_calls` (`usage-file-search-calls`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageFileSearchCallsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/file_search_calls` (`usage-file-search-calls`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageFileSearchCallsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/images` (`usage-images`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageImagesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/images` (`usage-images`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageImagesResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/moderations` (`usage-moderations`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageModerationsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/moderations` (`usage-moderations`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageModerationsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/vector_stores` (`usage-vector-stores`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageVectorStoresRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/vector_stores` (`usage-vector-stores`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageVectorStoresResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/usage/web_search_calls` (`usage-web-search-calls`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageWebSearchCallsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/usage/web_search_calls` (`usage-web-search-calls`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UsageWebSearchCallsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/users` (`list-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListUsersRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/users` (`list-users`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListUsersResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/users/{user_id}` (`delete-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteUserRequest {
    pub user_id: String,
}

/// Response for `DELETE /organization/users/{user_id}` (`delete-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DeleteUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/users/{user_id}` (`retrieve-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveUserRequest {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/users/{user_id}` (`retrieve-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/users/{user_id}` (`modify-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyUserRequest {
    pub user_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: ModifyUserBody,
}

/// Body for `modify-user`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyUserBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl ModifyUserBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/users/{user_id}` (`modify-user`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModifyUserResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/users/{user_id}/roles` (`list-user-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListUserRoleAssignmentsRequest {
    pub user_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/users/{user_id}/roles` (`list-user-role-assignments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListUserRoleAssignmentsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `POST /organization/users/{user_id}/roles` (`assign-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignUserRoleRequest {
    pub user_id: String,
    /// Typed JSON body for this operation (deserialized, not raw-forwarded).
    #[serde(flatten)]
    pub body: AssignUserRoleBody,
}

/// Body for `assign-user-role`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignUserRoleBody {
    /// Documented and additive fields accepted for this operation.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl AssignUserRoleBody {
    pub fn from_json(value: Value) -> PlatformResult<Self> {
        serde_json::from_value(value).map_err(|e| PlatformError::InvalidRequest(e.to_string()))
    }
}

/// Response for `POST /organization/users/{user_id}/roles` (`assign-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct AssignUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `DELETE /organization/users/{user_id}/roles/{role_id}` (`unassign-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignUserRoleRequest {
    pub user_id: String,
    pub role_id: String,
}

/// Response for `DELETE /organization/users/{user_id}/roles/{role_id}` (`unassign-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnassignUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

/// Request for `GET /organization/users/{user_id}/roles/{role_id}` (`retrieve-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveUserRoleRequest {
    pub user_id: String,
    pub role_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    /// Additional documented query parameters for this operation.
    #[serde(default, flatten)]
    pub query: BTreeMap<String, Value>,
}

/// Response for `GET /organization/users/{user_id}/roles/{role_id}` (`retrieve-user-role`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RetrieveUserRoleResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<Value>>,
    #[serde(default, flatten)]
    pub fields: BTreeMap<String, Value>,
}

impl crate::openai_platform::client::OpenAiAdminClient {
    /// `GET /organization/admin_api_keys` — `admin-api-keys-list`.
    pub async fn admin_api_keys_list(
        &self,
        request: AdminApiKeysListRequest,
    ) -> PlatformResult<AdminApiKeysListResponse> {
        let mut path = String::from("/organization/admin_api_keys");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "admin-api-keys-list",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/admin_api_keys` — `admin-api-keys-create`.
    pub async fn admin_api_keys_create(
        &self,
        request: AdminApiKeysCreateRequest,
    ) -> PlatformResult<AdminApiKeysCreateResponse> {
        let mut path = String::from("/organization/admin_api_keys");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "admin-api-keys-create",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/admin_api_keys/{key_id}` — `admin-api-keys-delete`.
    pub async fn admin_api_keys_delete(
        &self,
        request: AdminApiKeysDeleteRequest,
    ) -> PlatformResult<AdminApiKeysDeleteResponse> {
        let mut path = String::from("/organization/admin_api_keys/{key_id}");
        path = path.replace("{key_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.key_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "admin-api-keys-delete",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/admin_api_keys/{key_id}` — `admin-api-keys-get`.
    pub async fn admin_api_keys_get(
        &self,
        request: AdminApiKeysGetRequest,
    ) -> PlatformResult<AdminApiKeysGetResponse> {
        let mut path = String::from("/organization/admin_api_keys/{key_id}");
        path = path.replace("{key_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.key_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "admin-api-keys-get",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/audit_logs` — `list-audit-logs`.
    pub async fn list_audit_logs(
        &self,
        request: ListAuditLogsRequest,
    ) -> PlatformResult<ListAuditLogsResponse> {
        let mut path = String::from("/organization/audit_logs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-audit-logs",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/certificates` — `listOrganizationCertificates`.
    pub async fn list_organization_certificates(
        &self,
        request: ListOrganizationCertificatesRequest,
    ) -> PlatformResult<ListOrganizationCertificatesResponse> {
        let mut path = String::from("/organization/certificates");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listOrganizationCertificates",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/certificates` — `uploadCertificate`.
    pub async fn upload_certificate(
        &self,
        request: UploadCertificateRequest,
    ) -> PlatformResult<UploadCertificateResponse> {
        let mut path = String::from("/organization/certificates");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "uploadCertificate",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/certificates/activate` — `activateOrganizationCertificates`.
    pub async fn activate_organization_certificates(
        &self,
        request: ActivateOrganizationCertificatesRequest,
    ) -> PlatformResult<ActivateOrganizationCertificatesResponse> {
        let mut path = String::from("/organization/certificates/activate");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "activateOrganizationCertificates",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/certificates/deactivate` — `deactivateOrganizationCertificates`.
    pub async fn deactivate_organization_certificates(
        &self,
        request: DeactivateOrganizationCertificatesRequest,
    ) -> PlatformResult<DeactivateOrganizationCertificatesResponse> {
        let mut path = String::from("/organization/certificates/deactivate");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deactivateOrganizationCertificates",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/certificates/{certificate_id}` — `deleteCertificate`.
    pub async fn delete_certificate(
        &self,
        request: DeleteCertificateRequest,
    ) -> PlatformResult<DeleteCertificateResponse> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace("{certificate_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deleteCertificate",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/certificates/{certificate_id}` — `getCertificate`.
    pub async fn get_certificate(
        &self,
        request: GetCertificateRequest,
    ) -> PlatformResult<GetCertificateResponse> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace("{certificate_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "getCertificate",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/certificates/{certificate_id}` — `modifyCertificate`.
    pub async fn modify_certificate(
        &self,
        request: ModifyCertificateRequest,
    ) -> PlatformResult<ModifyCertificateResponse> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace("{certificate_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modifyCertificate",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/costs` — `usage-costs`.
    pub async fn usage_costs(
        &self,
        request: UsageCostsRequest,
    ) -> PlatformResult<UsageCostsResponse> {
        let mut path = String::from("/organization/costs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-costs",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/data_retention` — `retrieve-organization-data-retention`.
    pub async fn retrieve_organization_data_retention(
        &self,
        request: RetrieveOrganizationDataRetentionRequest,
    ) -> PlatformResult<RetrieveOrganizationDataRetentionResponse> {
        let mut path = String::from("/organization/data_retention");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-organization-data-retention",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/data_retention` — `update-organization-data-retention`.
    pub async fn update_organization_data_retention(
        &self,
        request: UpdateOrganizationDataRetentionRequest,
    ) -> PlatformResult<UpdateOrganizationDataRetentionResponse> {
        let mut path = String::from("/organization/data_retention");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-organization-data-retention",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups` — `list-groups`.
    pub async fn list_groups(
        &self,
        request: ListGroupsRequest,
    ) -> PlatformResult<ListGroupsResponse> {
        let mut path = String::from("/organization/groups");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-groups",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/groups` — `create-group`.
    pub async fn create_group(
        &self,
        request: CreateGroupRequest,
    ) -> PlatformResult<CreateGroupResponse> {
        let mut path = String::from("/organization/groups");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-group",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/groups/{group_id}` — `delete-group`.
    pub async fn delete_group(
        &self,
        request: DeleteGroupRequest,
    ) -> PlatformResult<DeleteGroupResponse> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-group",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups/{group_id}` — `retrieve-group`.
    pub async fn retrieve_group(
        &self,
        request: RetrieveGroupRequest,
    ) -> PlatformResult<RetrieveGroupResponse> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-group",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/groups/{group_id}` — `update-group`.
    pub async fn update_group(
        &self,
        request: UpdateGroupRequest,
    ) -> PlatformResult<UpdateGroupResponse> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-group",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups/{group_id}/roles` — `list-group-role-assignments`.
    pub async fn list_group_role_assignments(
        &self,
        request: ListGroupRoleAssignmentsRequest,
    ) -> PlatformResult<ListGroupRoleAssignmentsResponse> {
        let mut path = String::from("/organization/groups/{group_id}/roles");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-group-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/groups/{group_id}/roles` — `assign-group-role`.
    pub async fn assign_group_role(
        &self,
        request: AssignGroupRoleRequest,
    ) -> PlatformResult<AssignGroupRoleResponse> {
        let mut path = String::from("/organization/groups/{group_id}/roles");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "assign-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/groups/{group_id}/roles/{role_id}` — `unassign-group-role`.
    pub async fn unassign_group_role(
        &self,
        request: UnassignGroupRoleRequest,
    ) -> PlatformResult<UnassignGroupRoleResponse> {
        let mut path = String::from("/organization/groups/{group_id}/roles/{role_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "unassign-group-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups/{group_id}/roles/{role_id}` — `retrieve-group-role`.
    pub async fn retrieve_group_role(
        &self,
        request: RetrieveGroupRoleRequest,
    ) -> PlatformResult<RetrieveGroupRoleResponse> {
        let mut path = String::from("/organization/groups/{group_id}/roles/{role_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-group-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups/{group_id}/users` — `list-group-users`.
    pub async fn list_group_users(
        &self,
        request: ListGroupUsersRequest,
    ) -> PlatformResult<ListGroupUsersResponse> {
        let mut path = String::from("/organization/groups/{group_id}/users");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-group-users",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/groups/{group_id}/users` — `add-group-user`.
    pub async fn add_group_user(
        &self,
        request: AddGroupUserRequest,
    ) -> PlatformResult<AddGroupUserResponse> {
        let mut path = String::from("/organization/groups/{group_id}/users");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "add-group-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/groups/{group_id}/users/{user_id}` — `remove-group-user`.
    pub async fn remove_group_user(
        &self,
        request: RemoveGroupUserRequest,
    ) -> PlatformResult<RemoveGroupUserResponse> {
        let mut path = String::from("/organization/groups/{group_id}/users/{user_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "remove-group-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/groups/{group_id}/users/{user_id}` — `retrieve-group-user`.
    pub async fn retrieve_group_user(
        &self,
        request: RetrieveGroupUserRequest,
    ) -> PlatformResult<RetrieveGroupUserResponse> {
        let mut path = String::from("/organization/groups/{group_id}/users/{user_id}");
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-group-user",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/invites` — `list-invites`.
    pub async fn list_invites(
        &self,
        request: ListInvitesRequest,
    ) -> PlatformResult<ListInvitesResponse> {
        let mut path = String::from("/organization/invites");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-invites",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/invites` — `inviteUser`.
    pub async fn invite_user(
        &self,
        request: InviteUserRequest,
    ) -> PlatformResult<InviteUserResponse> {
        let mut path = String::from("/organization/invites");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "inviteUser",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/invites/{invite_id}` — `delete-invite`.
    pub async fn delete_invite(
        &self,
        request: DeleteInviteRequest,
    ) -> PlatformResult<DeleteInviteResponse> {
        let mut path = String::from("/organization/invites/{invite_id}");
        path = path.replace("{invite_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.invite_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-invite",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/invites/{invite_id}` — `retrieve-invite`.
    pub async fn retrieve_invite(
        &self,
        request: RetrieveInviteRequest,
    ) -> PlatformResult<RetrieveInviteResponse> {
        let mut path = String::from("/organization/invites/{invite_id}");
        path = path.replace("{invite_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.invite_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-invite",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects` — `list-projects`.
    pub async fn list_projects(
        &self,
        request: ListProjectsRequest,
    ) -> PlatformResult<ListProjectsResponse> {
        let mut path = String::from("/organization/projects");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-projects",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects` — `create-project`.
    pub async fn create_project(
        &self,
        request: CreateProjectRequest,
    ) -> PlatformResult<CreateProjectResponse> {
        let mut path = String::from("/organization/projects");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-project",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}` — `retrieve-project`.
    pub async fn retrieve_project(
        &self,
        request: RetrieveProjectRequest,
    ) -> PlatformResult<RetrieveProjectResponse> {
        let mut path = String::from("/organization/projects/{project_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}` — `modify-project`.
    pub async fn modify_project(
        &self,
        request: ModifyProjectRequest,
    ) -> PlatformResult<ModifyProjectResponse> {
        let mut path = String::from("/organization/projects/{project_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modify-project",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/api_keys` — `list-project-api-keys`.
    pub async fn list_project_api_keys(
        &self,
        request: ListProjectApiKeysRequest,
    ) -> PlatformResult<ListProjectApiKeysResponse> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-api-keys",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/api_keys/{api_key_id}` — `delete-project-api-key`.
    pub async fn delete_project_api_key(
        &self,
        request: DeleteProjectApiKeyRequest,
    ) -> PlatformResult<DeleteProjectApiKeyResponse> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys/{api_key_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{api_key_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.api_key_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-api-key",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/api_keys/{api_key_id}` — `retrieve-project-api-key`.
    pub async fn retrieve_project_api_key(
        &self,
        request: RetrieveProjectApiKeyRequest,
    ) -> PlatformResult<RetrieveProjectApiKeyResponse> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys/{api_key_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{api_key_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.api_key_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-api-key",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/archive` — `archive-project`.
    pub async fn archive_project(
        &self,
        request: ArchiveProjectRequest,
    ) -> PlatformResult<ArchiveProjectResponse> {
        let mut path = String::from("/organization/projects/{project_id}/archive");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "archive-project",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/certificates` — `listProjectCertificates`.
    pub async fn list_project_certificates(
        &self,
        request: ListProjectCertificatesRequest,
    ) -> PlatformResult<ListProjectCertificatesResponse> {
        let mut path = String::from("/organization/projects/{project_id}/certificates");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "listProjectCertificates",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/certificates/activate` — `activateProjectCertificates`.
    pub async fn activate_project_certificates(
        &self,
        request: ActivateProjectCertificatesRequest,
    ) -> PlatformResult<ActivateProjectCertificatesResponse> {
        let mut path = String::from("/organization/projects/{project_id}/certificates/activate");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "activateProjectCertificates",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/certificates/deactivate` — `deactivateProjectCertificates`.
    pub async fn deactivate_project_certificates(
        &self,
        request: DeactivateProjectCertificatesRequest,
    ) -> PlatformResult<DeactivateProjectCertificatesResponse> {
        let mut path = String::from("/organization/projects/{project_id}/certificates/deactivate");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "deactivateProjectCertificates",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/data_retention` — `retrieve-project-data-retention`.
    pub async fn retrieve_project_data_retention(
        &self,
        request: RetrieveProjectDataRetentionRequest,
    ) -> PlatformResult<RetrieveProjectDataRetentionResponse> {
        let mut path = String::from("/organization/projects/{project_id}/data_retention");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-data-retention",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/data_retention` — `update-project-data-retention`.
    pub async fn update_project_data_retention(
        &self,
        request: UpdateProjectDataRetentionRequest,
    ) -> PlatformResult<UpdateProjectDataRetentionResponse> {
        let mut path = String::from("/organization/projects/{project_id}/data_retention");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-data-retention",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/groups` — `list-project-groups`.
    pub async fn list_project_groups(
        &self,
        request: ListProjectGroupsRequest,
    ) -> PlatformResult<ListProjectGroupsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/groups");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-groups",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/groups` — `add-project-group`.
    pub async fn add_project_group(
        &self,
        request: AddProjectGroupRequest,
    ) -> PlatformResult<AddProjectGroupResponse> {
        let mut path = String::from("/organization/projects/{project_id}/groups");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "add-project-group",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/groups/{group_id}` — `remove-project-group`.
    pub async fn remove_project_group(
        &self,
        request: RemoveProjectGroupRequest,
    ) -> PlatformResult<RemoveProjectGroupResponse> {
        let mut path = String::from("/organization/projects/{project_id}/groups/{group_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "remove-project-group",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/groups/{group_id}` — `retrieve-project-group`.
    pub async fn retrieve_project_group(
        &self,
        request: RetrieveProjectGroupRequest,
    ) -> PlatformResult<RetrieveProjectGroupResponse> {
        let mut path = String::from("/organization/projects/{project_id}/groups/{group_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{group_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.group_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-group",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/hosted_tool_permissions` — `retrieve-project-hosted-tool-permissions`.
    pub async fn retrieve_project_hosted_tool_permissions(
        &self,
        request: RetrieveProjectHostedToolPermissionsRequest,
    ) -> PlatformResult<RetrieveProjectHostedToolPermissionsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/hosted_tool_permissions");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-hosted-tool-permissions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/hosted_tool_permissions` — `update-project-hosted-tool-permissions`.
    pub async fn update_project_hosted_tool_permissions(
        &self,
        request: UpdateProjectHostedToolPermissionsRequest,
    ) -> PlatformResult<UpdateProjectHostedToolPermissionsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/hosted_tool_permissions");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-hosted-tool-permissions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/model_permissions` — `delete-project-model-permissions`.
    pub async fn delete_project_model_permissions(
        &self,
        request: DeleteProjectModelPermissionsRequest,
    ) -> PlatformResult<DeleteProjectModelPermissionsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-model-permissions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/model_permissions` — `retrieve-project-model-permissions`.
    pub async fn retrieve_project_model_permissions(
        &self,
        request: RetrieveProjectModelPermissionsRequest,
    ) -> PlatformResult<RetrieveProjectModelPermissionsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-model-permissions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/model_permissions` — `update-project-model-permissions`.
    pub async fn update_project_model_permissions(
        &self,
        request: UpdateProjectModelPermissionsRequest,
    ) -> PlatformResult<UpdateProjectModelPermissionsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-model-permissions",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/rate_limits` — `list-project-rate-limits`.
    pub async fn list_project_rate_limits(
        &self,
        request: ListProjectRateLimitsRequest,
    ) -> PlatformResult<ListProjectRateLimitsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/rate_limits");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-rate-limits",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/rate_limits/{rate_limit_id}` — `update-project-rate-limits`.
    pub async fn update_project_rate_limits(
        &self,
        request: UpdateProjectRateLimitsRequest,
    ) -> PlatformResult<UpdateProjectRateLimitsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/rate_limits/{rate_limit_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{rate_limit_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.rate_limit_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-rate-limits",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/service_accounts` — `list-project-service-accounts`.
    pub async fn list_project_service_accounts(
        &self,
        request: ListProjectServiceAccountsRequest,
    ) -> PlatformResult<ListProjectServiceAccountsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-service-accounts",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/service_accounts` — `create-project-service-account`.
    pub async fn create_project_service_account(
        &self,
        request: CreateProjectServiceAccountRequest,
    ) -> PlatformResult<CreateProjectServiceAccountResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-project-service-account",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/service_accounts/{service_account_id}` — `delete-project-service-account`.
    pub async fn delete_project_service_account(
        &self,
        request: DeleteProjectServiceAccountRequest,
    ) -> PlatformResult<DeleteProjectServiceAccountResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts/{service_account_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{service_account_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-service-account",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/service_accounts/{service_account_id}` — `retrieve-project-service-account`.
    pub async fn retrieve_project_service_account(
        &self,
        request: RetrieveProjectServiceAccountRequest,
    ) -> PlatformResult<RetrieveProjectServiceAccountResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts/{service_account_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{service_account_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-service-account",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/service_accounts/{service_account_id}` — `update-project-service-account`.
    pub async fn update_project_service_account(
        &self,
        request: UpdateProjectServiceAccountRequest,
    ) -> PlatformResult<UpdateProjectServiceAccountResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts/{service_account_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{service_account_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-service-account",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys` — `CreateanAPIkeyforaserviceaccount`.
    pub async fn createan_ap_ikeyforaserviceaccount(
        &self,
        request: CreateanAPIkeyforaserviceaccountRequest,
    ) -> PlatformResult<CreateanAPIkeyforaserviceaccountResponse> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{service_account_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "CreateanAPIkeyforaserviceaccount",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/spend_alerts` — `list-project-spend-alerts`.
    pub async fn list_project_spend_alerts(
        &self,
        request: ListProjectSpendAlertsRequest,
    ) -> PlatformResult<ListProjectSpendAlertsResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-spend-alerts",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/spend_alerts` — `create-project-spend-alert`.
    pub async fn create_project_spend_alert(
        &self,
        request: CreateProjectSpendAlertRequest,
    ) -> PlatformResult<CreateProjectSpendAlertResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-project-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/spend_alerts/{alert_id}` — `delete-project-spend-alert`.
    pub async fn delete_project_spend_alert(
        &self,
        request: DeleteProjectSpendAlertRequest,
    ) -> PlatformResult<DeleteProjectSpendAlertResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/spend_alerts/{alert_id}` — `retrieve-project-spend-alert`.
    pub async fn retrieve_project_spend_alert(
        &self,
        request: RetrieveProjectSpendAlertRequest,
    ) -> PlatformResult<RetrieveProjectSpendAlertResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-spend-alert",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/spend_alerts/{alert_id}` — `update-project-spend-alert`.
    pub async fn update_project_spend_alert(
        &self,
        request: UpdateProjectSpendAlertRequest,
    ) -> PlatformResult<UpdateProjectSpendAlertResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-project-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/spend_limit` — `Deleteprojectspendlimit`.
    pub async fn deleteprojectspendlimit(
        &self,
        request: DeleteprojectspendlimitRequest,
    ) -> PlatformResult<DeleteprojectspendlimitResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Deleteprojectspendlimit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/spend_limit` — `Getprojectspendlimit`.
    pub async fn getprojectspendlimit(
        &self,
        request: GetprojectspendlimitRequest,
    ) -> PlatformResult<GetprojectspendlimitResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Getprojectspendlimit",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/spend_limit` — `Updateprojectspendlimit`.
    pub async fn updateprojectspendlimit(
        &self,
        request: UpdateprojectspendlimitRequest,
    ) -> PlatformResult<UpdateprojectspendlimitResponse> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Updateprojectspendlimit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/users` — `list-project-users`.
    pub async fn list_project_users(
        &self,
        request: ListProjectUsersRequest,
    ) -> PlatformResult<ListProjectUsersResponse> {
        let mut path = String::from("/organization/projects/{project_id}/users");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-project-users",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/users` — `create-project-user`.
    pub async fn create_project_user(
        &self,
        request: CreateProjectUserRequest,
    ) -> PlatformResult<CreateProjectUserResponse> {
        let mut path = String::from("/organization/projects/{project_id}/users");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-project-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/projects/{project_id}/users/{user_id}` — `delete-project-user`.
    pub async fn delete_project_user(
        &self,
        request: DeleteProjectUserRequest,
    ) -> PlatformResult<DeleteProjectUserResponse> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-project-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/projects/{project_id}/users/{user_id}` — `retrieve-project-user`.
    pub async fn retrieve_project_user(
        &self,
        request: RetrieveProjectUserRequest,
    ) -> PlatformResult<RetrieveProjectUserResponse> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-project-user",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/projects/{project_id}/users/{user_id}` — `modify-project-user`.
    pub async fn modify_project_user(
        &self,
        request: ModifyProjectUserRequest,
    ) -> PlatformResult<ModifyProjectUserResponse> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace("{project_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.project_id));
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modify-project-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/roles` — `list-roles`.
    pub async fn list_roles(
        &self,
        request: ListRolesRequest,
    ) -> PlatformResult<ListRolesResponse> {
        let mut path = String::from("/organization/roles");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-roles",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/roles` — `create-role`.
    pub async fn create_role(
        &self,
        request: CreateRoleRequest,
    ) -> PlatformResult<CreateRoleResponse> {
        let mut path = String::from("/organization/roles");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/roles/{role_id}` — `delete-role`.
    pub async fn delete_role(
        &self,
        request: DeleteRoleRequest,
    ) -> PlatformResult<DeleteRoleResponse> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/roles/{role_id}` — `retrieve-role`.
    pub async fn retrieve_role(
        &self,
        request: RetrieveRoleRequest,
    ) -> PlatformResult<RetrieveRoleResponse> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/roles/{role_id}` — `update-role`.
    pub async fn update_role(
        &self,
        request: UpdateRoleRequest,
    ) -> PlatformResult<UpdateRoleResponse> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/spend_alerts` — `list-organization-spend-alerts`.
    pub async fn list_organization_spend_alerts(
        &self,
        request: ListOrganizationSpendAlertsRequest,
    ) -> PlatformResult<ListOrganizationSpendAlertsResponse> {
        let mut path = String::from("/organization/spend_alerts");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-organization-spend-alerts",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/spend_alerts` — `create-organization-spend-alert`.
    pub async fn create_organization_spend_alert(
        &self,
        request: CreateOrganizationSpendAlertRequest,
    ) -> PlatformResult<CreateOrganizationSpendAlertResponse> {
        let mut path = String::from("/organization/spend_alerts");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "create-organization-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/spend_alerts/{alert_id}` — `delete-organization-spend-alert`.
    pub async fn delete_organization_spend_alert(
        &self,
        request: DeleteOrganizationSpendAlertRequest,
    ) -> PlatformResult<DeleteOrganizationSpendAlertResponse> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-organization-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/spend_alerts/{alert_id}` — `retrieve-organization-spend-alert`.
    pub async fn retrieve_organization_spend_alert(
        &self,
        request: RetrieveOrganizationSpendAlertRequest,
    ) -> PlatformResult<RetrieveOrganizationSpendAlertResponse> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-organization-spend-alert",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/spend_alerts/{alert_id}` — `update-organization-spend-alert`.
    pub async fn update_organization_spend_alert(
        &self,
        request: UpdateOrganizationSpendAlertRequest,
    ) -> PlatformResult<UpdateOrganizationSpendAlertResponse> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace("{alert_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "update-organization-spend-alert",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/spend_limit` — `Deleteorganizationspendlimit`.
    pub async fn deleteorganizationspendlimit(
        &self,
        request: DeleteorganizationspendlimitRequest,
    ) -> PlatformResult<DeleteorganizationspendlimitResponse> {
        let mut path = String::from("/organization/spend_limit");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Deleteorganizationspendlimit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/spend_limit` — `Getorganizationspendlimit`.
    pub async fn getorganizationspendlimit(
        &self,
        request: GetorganizationspendlimitRequest,
    ) -> PlatformResult<GetorganizationspendlimitResponse> {
        let mut path = String::from("/organization/spend_limit");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Getorganizationspendlimit",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/spend_limit` — `Updateorganizationspendlimit`.
    pub async fn updateorganizationspendlimit(
        &self,
        request: UpdateorganizationspendlimitRequest,
    ) -> PlatformResult<UpdateorganizationspendlimitResponse> {
        let mut path = String::from("/organization/spend_limit");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "Updateorganizationspendlimit",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/audio_speeches` — `usage-audio-speeches`.
    pub async fn usage_audio_speeches(
        &self,
        request: UsageAudioSpeechesRequest,
    ) -> PlatformResult<UsageAudioSpeechesResponse> {
        let mut path = String::from("/organization/usage/audio_speeches");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-audio-speeches",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/audio_transcriptions` — `usage-audio-transcriptions`.
    pub async fn usage_audio_transcriptions(
        &self,
        request: UsageAudioTranscriptionsRequest,
    ) -> PlatformResult<UsageAudioTranscriptionsResponse> {
        let mut path = String::from("/organization/usage/audio_transcriptions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-audio-transcriptions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/code_interpreter_sessions` — `usage-code-interpreter-sessions`.
    pub async fn usage_code_interpreter_sessions(
        &self,
        request: UsageCodeInterpreterSessionsRequest,
    ) -> PlatformResult<UsageCodeInterpreterSessionsResponse> {
        let mut path = String::from("/organization/usage/code_interpreter_sessions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-code-interpreter-sessions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/completions` — `usage-completions`.
    pub async fn usage_completions(
        &self,
        request: UsageCompletionsRequest,
    ) -> PlatformResult<UsageCompletionsResponse> {
        let mut path = String::from("/organization/usage/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-completions",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/embeddings` — `usage-embeddings`.
    pub async fn usage_embeddings(
        &self,
        request: UsageEmbeddingsRequest,
    ) -> PlatformResult<UsageEmbeddingsResponse> {
        let mut path = String::from("/organization/usage/embeddings");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-embeddings",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/file_search_calls` — `usage-file-search-calls`.
    pub async fn usage_file_search_calls(
        &self,
        request: UsageFileSearchCallsRequest,
    ) -> PlatformResult<UsageFileSearchCallsResponse> {
        let mut path = String::from("/organization/usage/file_search_calls");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-file-search-calls",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/images` — `usage-images`.
    pub async fn usage_images(
        &self,
        request: UsageImagesRequest,
    ) -> PlatformResult<UsageImagesResponse> {
        let mut path = String::from("/organization/usage/images");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-images",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/moderations` — `usage-moderations`.
    pub async fn usage_moderations(
        &self,
        request: UsageModerationsRequest,
    ) -> PlatformResult<UsageModerationsResponse> {
        let mut path = String::from("/organization/usage/moderations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-moderations",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/vector_stores` — `usage-vector-stores`.
    pub async fn usage_vector_stores(
        &self,
        request: UsageVectorStoresRequest,
    ) -> PlatformResult<UsageVectorStoresResponse> {
        let mut path = String::from("/organization/usage/vector_stores");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-vector-stores",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/usage/web_search_calls` — `usage-web-search-calls`.
    pub async fn usage_web_search_calls(
        &self,
        request: UsageWebSearchCallsRequest,
    ) -> PlatformResult<UsageWebSearchCallsResponse> {
        let mut path = String::from("/organization/usage/web_search_calls");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "usage-web-search-calls",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/users` — `list-users`.
    pub async fn list_users(
        &self,
        request: ListUsersRequest,
    ) -> PlatformResult<ListUsersResponse> {
        let mut path = String::from("/organization/users");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-users",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/users/{user_id}` — `delete-user`.
    pub async fn delete_user(
        &self,
        request: DeleteUserRequest,
    ) -> PlatformResult<DeleteUserResponse> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "delete-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/users/{user_id}` — `retrieve-user`.
    pub async fn retrieve_user(
        &self,
        request: RetrieveUserRequest,
    ) -> PlatformResult<RetrieveUserResponse> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-user",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/users/{user_id}` — `modify-user`.
    pub async fn modify_user(
        &self,
        request: ModifyUserRequest,
    ) -> PlatformResult<ModifyUserResponse> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "modify-user",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/users/{user_id}/roles` — `list-user-role-assignments`.
    pub async fn list_user_role_assignments(
        &self,
        request: ListUserRoleAssignmentsRequest,
    ) -> PlatformResult<ListUserRoleAssignmentsResponse> {
        let mut path = String::from("/organization/users/{user_id}/roles");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "list-user-role-assignments",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `POST /organization/users/{user_id}/roles` — `assign-user-role`.
    pub async fn assign_user_role(
        &self,
        request: AssignUserRoleRequest,
    ) -> PlatformResult<AssignUserRoleResponse> {
        let mut path = String::from("/organization/users/{user_id}/roles");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(serde_json::to_value(&request.body).map_err(|e| PlatformError::InvalidRequest(e.to_string()))?);
        let spec = HttpRequestSpec {
            method: "POST",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "assign-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `DELETE /organization/users/{user_id}/roles/{role_id}` — `unassign-user-role`.
    pub async fn unassign_user_role(
        &self,
        request: UnassignUserRoleRequest,
    ) -> PlatformResult<UnassignUserRoleResponse> {
        let mut path = String::from("/organization/users/{user_id}/roles/{role_id}");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "DELETE",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "unassign-user-role",
            idempotent: false,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

    /// `GET /organization/users/{user_id}/roles/{role_id}` — `retrieve-user-role`.
    pub async fn retrieve_user_role(
        &self,
        request: RetrieveUserRoleRequest,
    ) -> PlatformResult<RetrieveUserRoleResponse> {
        let mut path = String::from("/organization/users/{user_id}/roles/{role_id}");
        path = path.replace("{user_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.user_id));
        path = path.replace("{role_id}", &crate::openai_platform::url_policy::encode_path_segment(&request.role_id));
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit { query.insert("limit".into(), v.to_string()); }
        if let Some(v) = request.after.as_ref() { query.insert("after".into(), v.clone()); }
        if let Some(v) = request.before.as_ref() { query.insert("before".into(), v.clone()); }
        if let Some(v) = request.order.as_ref() { query.insert("order".into(), v.clone()); }
        for (k, v) in &request.query {
            if let Some(s) = v.as_str() { query.insert(k.clone(), s.to_owned()); }
            else if !v.is_null() { query.insert(k.clone(), v.to_string()); }
        }
        let body: Option<Value> = None;
        let spec = HttpRequestSpec {
            method: "GET",
            path,
            query,
            body,
            credential: CredentialKind::Admin,
            expect_sse: false,
            expect_binary: false,
            multipart: false,
            operation_id: "retrieve-user-role",
            idempotent: true,
        };
        let raw = self.transport.execute_json(spec).await?;
        serde_json::from_value(raw).map_err(|e| PlatformError::Decode(e.to_string()))
    }

}
