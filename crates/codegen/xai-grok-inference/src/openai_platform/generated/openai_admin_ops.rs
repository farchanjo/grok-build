//! Generated typed operations for openai_admin.
//! DO NOT EDIT BY HAND.

use super::super::error::{PlatformError, PlatformResult};
use super::super::transport::{CredentialKind, HttpRequestSpec};
use super::openai_admin_types::*;
use std::collections::BTreeMap;

fn query_value<T: serde::Serialize + ?Sized>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        Ok(other) if !other.is_null() => other.to_string(),
        _ => String::new(),
    }
}

impl crate::openai_platform::client::OpenAiAdminClient {
    /// `GET /organization/admin_api_keys` — `admin-api-keys-list` (json).
    /// Transports: http_json.
    pub async fn admin_api_keys_list(
        &self,
        request: AdminApiKeysListParams,
    ) -> PlatformResult<AdminApiKeysListResult> {
        let path = String::from("/organization/admin_api_keys");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/admin_api_keys` — `admin-api-keys-create` (json).
    /// Transports: http_json.
    pub async fn admin_api_keys_create(
        &self,
        request: AdminApiKeysCreateParams,
    ) -> PlatformResult<AdminApiKeysCreateResult> {
        let path = String::from("/organization/admin_api_keys");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/admin_api_keys/{key_id}` — `admin-api-keys-get` (json).
    /// Transports: http_json.
    pub async fn admin_api_keys_get(
        &self,
        request: AdminApiKeysGetParams,
    ) -> PlatformResult<AdminApiKeysGetResult> {
        let mut path = String::from("/organization/admin_api_keys/{key_id}");
        path = path.replace(
            "{key_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.key_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/admin_api_keys/{key_id}` — `admin-api-keys-delete` (json).
    /// Transports: http_json.
    pub async fn admin_api_keys_delete(
        &self,
        request: AdminApiKeysDeleteParams,
    ) -> PlatformResult<AdminApiKeysDeleteResult> {
        let mut path = String::from("/organization/admin_api_keys/{key_id}");
        path = path.replace(
            "{key_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.key_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/audit_logs` — `list-audit-logs` (json).
    /// Transports: http_json.
    pub async fn list_audit_logs(
        &self,
        request: ListAuditLogsParams,
    ) -> PlatformResult<ListAuditLogsResult> {
        let path = String::from("/organization/audit_logs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.effective_at.as_ref() {
            query.insert("effective_at".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids[]".into(), query_value(v));
        }
        if let Some(v) = request.event_types.as_ref() {
            query.insert("event_types[]".into(), query_value(v));
        }
        if let Some(v) = request.actor_ids.as_ref() {
            query.insert("actor_ids[]".into(), query_value(v));
        }
        if let Some(v) = request.actor_emails.as_ref() {
            query.insert("actor_emails[]".into(), query_value(v));
        }
        if let Some(v) = request.resource_ids.as_ref() {
            query.insert("resource_ids[]".into(), query_value(v));
        }
        if let Some(v) = request.tenant_only.as_ref() {
            query.insert("tenant_only".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/certificates` — `listOrganizationCertificates` (json).
    /// Transports: http_json.
    pub async fn list_organization_certificates(
        &self,
        request: ListOrganizationCertificatesParams,
    ) -> PlatformResult<ListOrganizationCertificatesResult> {
        let path = String::from("/organization/certificates");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/certificates` — `uploadCertificate` (json).
    /// Transports: http_json.
    pub async fn upload_certificate(
        &self,
        request: UploadCertificateParams,
    ) -> PlatformResult<UploadCertificateResult> {
        let path = String::from("/organization/certificates");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /organization/certificates/activate` — `activateOrganizationCertificates` (json).
    /// Transports: http_json.
    pub async fn activate_organization_certificates(
        &self,
        request: ActivateOrganizationCertificatesParams,
    ) -> PlatformResult<ActivateOrganizationCertificatesResult> {
        let path = String::from("/organization/certificates/activate");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /organization/certificates/deactivate` — `deactivateOrganizationCertificates` (json).
    /// Transports: http_json.
    pub async fn deactivate_organization_certificates(
        &self,
        request: DeactivateOrganizationCertificatesParams,
    ) -> PlatformResult<DeactivateOrganizationCertificatesResult> {
        let path = String::from("/organization/certificates/deactivate");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/certificates/{certificate_id}` — `getCertificate` (json).
    /// Transports: http_json.
    pub async fn get_certificate(
        &self,
        request: GetCertificateParams,
    ) -> PlatformResult<GetCertificateResult> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace(
            "{certificate_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.include.as_ref() {
            query.insert("include".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/certificates/{certificate_id}` — `modifyCertificate` (json).
    /// Transports: http_json.
    pub async fn modify_certificate(
        &self,
        request: ModifyCertificateParams,
    ) -> PlatformResult<ModifyCertificateResult> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace(
            "{certificate_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/certificates/{certificate_id}` — `deleteCertificate` (json).
    /// Transports: http_json.
    pub async fn delete_certificate(
        &self,
        request: DeleteCertificateParams,
    ) -> PlatformResult<DeleteCertificateResult> {
        let mut path = String::from("/organization/certificates/{certificate_id}");
        path = path.replace(
            "{certificate_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.certificate_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/costs` — `usage-costs` (json).
    /// Transports: http_json.
    pub async fn usage_costs(&self, request: UsageCostsParams) -> PlatformResult<UsageCostsResult> {
        let path = String::from("/organization/costs");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/data_retention` — `retrieve-organization-data-retention` (json).
    /// Transports: http_json.
    pub async fn retrieve_organization_data_retention(
        &self,
        _request: RetrieveOrganizationDataRetentionParams,
    ) -> PlatformResult<RetrieveOrganizationDataRetentionResult> {
        let path = String::from("/organization/data_retention");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/data_retention` — `update-organization-data-retention` (json).
    /// Transports: http_json.
    pub async fn update_organization_data_retention(
        &self,
        request: UpdateOrganizationDataRetentionParams,
    ) -> PlatformResult<UpdateOrganizationDataRetentionResult> {
        let path = String::from("/organization/data_retention");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/groups` — `list-groups` (json).
    /// Transports: http_json.
    pub async fn list_groups(&self, request: ListGroupsParams) -> PlatformResult<ListGroupsResult> {
        let path = String::from("/organization/groups");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/groups` — `create-group` (json).
    /// Transports: http_json.
    pub async fn create_group(
        &self,
        request: CreateGroupParams,
    ) -> PlatformResult<CreateGroupResult> {
        let path = String::from("/organization/groups");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/groups/{group_id}` — `retrieve-group` (json).
    /// Transports: http_json.
    pub async fn retrieve_group(
        &self,
        request: RetrieveGroupParams,
    ) -> PlatformResult<RetrieveGroupResult> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/groups/{group_id}` — `update-group` (json).
    /// Transports: http_json.
    pub async fn update_group(
        &self,
        request: UpdateGroupParams,
    ) -> PlatformResult<UpdateGroupResult> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/groups/{group_id}` — `delete-group` (json).
    /// Transports: http_json.
    pub async fn delete_group(
        &self,
        request: DeleteGroupParams,
    ) -> PlatformResult<DeleteGroupResult> {
        let mut path = String::from("/organization/groups/{group_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/groups/{group_id}/roles` — `list-group-role-assignments` (json).
    /// Transports: http_json.
    pub async fn list_group_role_assignments(
        &self,
        request: ListGroupRoleAssignmentsParams,
    ) -> PlatformResult<ListGroupRoleAssignmentsResult> {
        let mut path = String::from("/organization/groups/{group_id}/roles");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/groups/{group_id}/roles` — `assign-group-role` (json).
    /// Transports: http_json.
    pub async fn assign_group_role(
        &self,
        request: AssignGroupRoleParams,
    ) -> PlatformResult<AssignGroupRoleResult> {
        let mut path = String::from("/organization/groups/{group_id}/roles");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/groups/{group_id}/roles/{role_id}` — `retrieve-group-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_group_role(
        &self,
        request: RetrieveGroupRoleParams,
    ) -> PlatformResult<RetrieveGroupRoleResult> {
        let mut path = String::from("/organization/groups/{group_id}/roles/{role_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/groups/{group_id}/roles/{role_id}` — `unassign-group-role` (json).
    /// Transports: http_json.
    pub async fn unassign_group_role(
        &self,
        request: UnassignGroupRoleParams,
    ) -> PlatformResult<UnassignGroupRoleResult> {
        let mut path = String::from("/organization/groups/{group_id}/roles/{role_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/groups/{group_id}/users` — `list-group-users` (json).
    /// Transports: http_json.
    pub async fn list_group_users(
        &self,
        request: ListGroupUsersParams,
    ) -> PlatformResult<ListGroupUsersResult> {
        let mut path = String::from("/organization/groups/{group_id}/users");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/groups/{group_id}/users` — `add-group-user` (json).
    /// Transports: http_json.
    pub async fn add_group_user(
        &self,
        request: AddGroupUserParams,
    ) -> PlatformResult<AddGroupUserResult> {
        let mut path = String::from("/organization/groups/{group_id}/users");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/groups/{group_id}/users/{user_id}` — `retrieve-group-user` (json).
    /// Transports: http_json.
    pub async fn retrieve_group_user(
        &self,
        request: RetrieveGroupUserParams,
    ) -> PlatformResult<RetrieveGroupUserResult> {
        let mut path = String::from("/organization/groups/{group_id}/users/{user_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/groups/{group_id}/users/{user_id}` — `remove-group-user` (json).
    /// Transports: http_json.
    pub async fn remove_group_user(
        &self,
        request: RemoveGroupUserParams,
    ) -> PlatformResult<RemoveGroupUserResult> {
        let mut path = String::from("/organization/groups/{group_id}/users/{user_id}");
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/invites` — `list-invites` (json).
    /// Transports: http_json.
    pub async fn list_invites(
        &self,
        request: ListInvitesParams,
    ) -> PlatformResult<ListInvitesResult> {
        let path = String::from("/organization/invites");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/invites` — `inviteUser` (json).
    /// Transports: http_json.
    pub async fn invite_user(&self, request: InviteUserParams) -> PlatformResult<InviteUserResult> {
        let path = String::from("/organization/invites");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/invites/{invite_id}` — `retrieve-invite` (json).
    /// Transports: http_json.
    pub async fn retrieve_invite(
        &self,
        request: RetrieveInviteParams,
    ) -> PlatformResult<RetrieveInviteResult> {
        let mut path = String::from("/organization/invites/{invite_id}");
        path = path.replace(
            "{invite_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.invite_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/invites/{invite_id}` — `delete-invite` (json).
    /// Transports: http_json.
    pub async fn delete_invite(
        &self,
        request: DeleteInviteParams,
    ) -> PlatformResult<DeleteInviteResult> {
        let mut path = String::from("/organization/invites/{invite_id}");
        path = path.replace(
            "{invite_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.invite_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects` — `list-projects` (json).
    /// Transports: http_json.
    pub async fn list_projects(
        &self,
        request: ListProjectsParams,
    ) -> PlatformResult<ListProjectsResult> {
        let path = String::from("/organization/projects");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.include_archived.as_ref() {
            query.insert("include_archived".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects` — `create-project` (json).
    /// Transports: http_json.
    pub async fn create_project(
        &self,
        request: CreateProjectParams,
    ) -> PlatformResult<CreateProjectResult> {
        let path = String::from("/organization/projects");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}` — `retrieve-project` (json).
    /// Transports: http_json.
    pub async fn retrieve_project(
        &self,
        request: RetrieveProjectParams,
    ) -> PlatformResult<RetrieveProjectResult> {
        let mut path = String::from("/organization/projects/{project_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}` — `modify-project` (json).
    /// Transports: http_json.
    pub async fn modify_project(
        &self,
        request: ModifyProjectParams,
    ) -> PlatformResult<ModifyProjectResult> {
        let mut path = String::from("/organization/projects/{project_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/api_keys` — `list-project-api-keys` (json).
    /// Transports: http_json.
    pub async fn list_project_api_keys(
        &self,
        request: ListProjectApiKeysParams,
    ) -> PlatformResult<ListProjectApiKeysResult> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.owner_project_access.as_ref() {
            query.insert("owner_project_access".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects/{project_id}/api_keys/{api_key_id}` — `retrieve-project-api-key` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_api_key(
        &self,
        request: RetrieveProjectApiKeyParams,
    ) -> PlatformResult<RetrieveProjectApiKeyResult> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys/{api_key_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{api_key_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.api_key_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/projects/{project_id}/api_keys/{api_key_id}` — `delete-project-api-key` (json).
    /// Transports: http_json.
    pub async fn delete_project_api_key(
        &self,
        request: DeleteProjectApiKeyParams,
    ) -> PlatformResult<DeleteProjectApiKeyResult> {
        let mut path = String::from("/organization/projects/{project_id}/api_keys/{api_key_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{api_key_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.api_key_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/archive` — `archive-project` (json).
    /// Transports: http_json.
    pub async fn archive_project(
        &self,
        request: ArchiveProjectParams,
    ) -> PlatformResult<ArchiveProjectResult> {
        let mut path = String::from("/organization/projects/{project_id}/archive");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects/{project_id}/certificates` — `listProjectCertificates` (json).
    /// Transports: http_json.
    pub async fn list_project_certificates(
        &self,
        request: ListProjectCertificatesParams,
    ) -> PlatformResult<ListProjectCertificatesResult> {
        let mut path = String::from("/organization/projects/{project_id}/certificates");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/certificates/activate` — `activateProjectCertificates` (json).
    /// Transports: http_json.
    pub async fn activate_project_certificates(
        &self,
        request: ActivateProjectCertificatesParams,
    ) -> PlatformResult<ActivateProjectCertificatesResult> {
        let mut path = String::from("/organization/projects/{project_id}/certificates/activate");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `POST /organization/projects/{project_id}/certificates/deactivate` — `deactivateProjectCertificates` (json).
    /// Transports: http_json.
    pub async fn deactivate_project_certificates(
        &self,
        request: DeactivateProjectCertificatesParams,
    ) -> PlatformResult<DeactivateProjectCertificatesResult> {
        let mut path = String::from("/organization/projects/{project_id}/certificates/deactivate");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/data_retention` — `retrieve-project-data-retention` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_data_retention(
        &self,
        request: RetrieveProjectDataRetentionParams,
    ) -> PlatformResult<RetrieveProjectDataRetentionResult> {
        let mut path = String::from("/organization/projects/{project_id}/data_retention");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/data_retention` — `update-project-data-retention` (json).
    /// Transports: http_json.
    pub async fn update_project_data_retention(
        &self,
        request: UpdateProjectDataRetentionParams,
    ) -> PlatformResult<UpdateProjectDataRetentionResult> {
        let mut path = String::from("/organization/projects/{project_id}/data_retention");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/groups` — `list-project-groups` (json).
    /// Transports: http_json.
    pub async fn list_project_groups(
        &self,
        request: ListProjectGroupsParams,
    ) -> PlatformResult<ListProjectGroupsResult> {
        let mut path = String::from("/organization/projects/{project_id}/groups");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/groups` — `add-project-group` (json).
    /// Transports: http_json.
    pub async fn add_project_group(
        &self,
        request: AddProjectGroupParams,
    ) -> PlatformResult<AddProjectGroupResult> {
        let mut path = String::from("/organization/projects/{project_id}/groups");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/groups/{group_id}` — `retrieve-project-group` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_group(
        &self,
        request: RetrieveProjectGroupParams,
    ) -> PlatformResult<RetrieveProjectGroupResult> {
        let mut path = String::from("/organization/projects/{project_id}/groups/{group_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.group_type.as_ref() {
            query.insert("group_type".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/projects/{project_id}/groups/{group_id}` — `remove-project-group` (json).
    /// Transports: http_json.
    pub async fn remove_project_group(
        &self,
        request: RemoveProjectGroupParams,
    ) -> PlatformResult<RemoveProjectGroupResult> {
        let mut path = String::from("/organization/projects/{project_id}/groups/{group_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{group_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.group_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects/{project_id}/hosted_tool_permissions` — `retrieve-project-hosted-tool-permissions` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_hosted_tool_permissions(
        &self,
        request: RetrieveProjectHostedToolPermissionsParams,
    ) -> PlatformResult<RetrieveProjectHostedToolPermissionsResult> {
        let mut path = String::from("/organization/projects/{project_id}/hosted_tool_permissions");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/hosted_tool_permissions` — `update-project-hosted-tool-permissions` (json).
    /// Transports: http_json.
    pub async fn update_project_hosted_tool_permissions(
        &self,
        request: UpdateProjectHostedToolPermissionsParams,
    ) -> PlatformResult<UpdateProjectHostedToolPermissionsResult> {
        let mut path = String::from("/organization/projects/{project_id}/hosted_tool_permissions");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/model_permissions` — `retrieve-project-model-permissions` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_model_permissions(
        &self,
        request: RetrieveProjectModelPermissionsParams,
    ) -> PlatformResult<RetrieveProjectModelPermissionsResult> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/model_permissions` — `update-project-model-permissions` (json).
    /// Transports: http_json.
    pub async fn update_project_model_permissions(
        &self,
        request: UpdateProjectModelPermissionsParams,
    ) -> PlatformResult<UpdateProjectModelPermissionsResult> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/projects/{project_id}/model_permissions` — `delete-project-model-permissions` (json).
    /// Transports: http_json.
    pub async fn delete_project_model_permissions(
        &self,
        request: DeleteProjectModelPermissionsParams,
    ) -> PlatformResult<DeleteProjectModelPermissionsResult> {
        let mut path = String::from("/organization/projects/{project_id}/model_permissions");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects/{project_id}/rate_limits` — `list-project-rate-limits` (json).
    /// Transports: http_json.
    pub async fn list_project_rate_limits(
        &self,
        request: ListProjectRateLimitsParams,
    ) -> PlatformResult<ListProjectRateLimitsResult> {
        let mut path = String::from("/organization/projects/{project_id}/rate_limits");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/rate_limits/{rate_limit_id}` — `update-project-rate-limits` (json).
    /// Transports: http_json.
    pub async fn update_project_rate_limits(
        &self,
        request: UpdateProjectRateLimitsParams,
    ) -> PlatformResult<UpdateProjectRateLimitsResult> {
        let mut path =
            String::from("/organization/projects/{project_id}/rate_limits/{rate_limit_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{rate_limit_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.rate_limit_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/service_accounts` — `list-project-service-accounts` (json).
    /// Transports: http_json.
    pub async fn list_project_service_accounts(
        &self,
        request: ListProjectServiceAccountsParams,
    ) -> PlatformResult<ListProjectServiceAccountsResult> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/service_accounts` — `create-project-service-account` (json).
    /// Transports: http_json.
    pub async fn create_project_service_account(
        &self,
        request: CreateProjectServiceAccountParams,
    ) -> PlatformResult<CreateProjectServiceAccountResult> {
        let mut path = String::from("/organization/projects/{project_id}/service_accounts");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/service_accounts/{service_account_id}` — `retrieve-project-service-account` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_service_account(
        &self,
        request: RetrieveProjectServiceAccountParams,
    ) -> PlatformResult<RetrieveProjectServiceAccountResult> {
        let mut path = String::from(
            "/organization/projects/{project_id}/service_accounts/{service_account_id}",
        );
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{service_account_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/service_accounts/{service_account_id}` — `update-project-service-account` (json).
    /// Transports: http_json.
    pub async fn update_project_service_account(
        &self,
        request: UpdateProjectServiceAccountParams,
    ) -> PlatformResult<UpdateProjectServiceAccountResult> {
        let mut path = String::from(
            "/organization/projects/{project_id}/service_accounts/{service_account_id}",
        );
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{service_account_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/projects/{project_id}/service_accounts/{service_account_id}` — `delete-project-service-account` (json).
    /// Transports: http_json.
    pub async fn delete_project_service_account(
        &self,
        request: DeleteProjectServiceAccountParams,
    ) -> PlatformResult<DeleteProjectServiceAccountResult> {
        let mut path = String::from(
            "/organization/projects/{project_id}/service_accounts/{service_account_id}",
        );
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{service_account_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys` — `CreateanAPIkeyforaserviceaccount` (json).
    /// Transports: http_json.
    pub async fn createan_ap_ikeyforaserviceaccount(
        &self,
        request: CreateanAPIkeyforaserviceaccountParams,
    ) -> PlatformResult<CreateanAPIkeyforaserviceaccountResult> {
        let mut path = String::from(
            "/organization/projects/{project_id}/service_accounts/{service_account_id}/api_keys",
        );
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{service_account_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.service_account_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/spend_alerts` — `list-project-spend-alerts` (json).
    /// Transports: http_json.
    pub async fn list_project_spend_alerts(
        &self,
        request: ListProjectSpendAlertsParams,
    ) -> PlatformResult<ListProjectSpendAlertsResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/spend_alerts` — `create-project-spend-alert` (json).
    /// Transports: http_json.
    pub async fn create_project_spend_alert(
        &self,
        request: CreateProjectSpendAlertParams,
    ) -> PlatformResult<CreateProjectSpendAlertResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/spend_alerts/{alert_id}` — `retrieve-project-spend-alert` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_spend_alert(
        &self,
        request: RetrieveProjectSpendAlertParams,
    ) -> PlatformResult<RetrieveProjectSpendAlertResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/spend_alerts/{alert_id}` — `update-project-spend-alert` (json).
    /// Transports: http_json.
    pub async fn update_project_spend_alert(
        &self,
        request: UpdateProjectSpendAlertParams,
    ) -> PlatformResult<UpdateProjectSpendAlertResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/projects/{project_id}/spend_alerts/{alert_id}` — `delete-project-spend-alert` (json).
    /// Transports: http_json.
    pub async fn delete_project_spend_alert(
        &self,
        request: DeleteProjectSpendAlertParams,
    ) -> PlatformResult<DeleteProjectSpendAlertResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_alerts/{alert_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/projects/{project_id}/spend_limit` — `Getprojectspendlimit` (json).
    /// Transports: http_json.
    pub async fn getprojectspendlimit(
        &self,
        request: GetprojectspendlimitParams,
    ) -> PlatformResult<GetprojectspendlimitResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/projects/{project_id}/spend_limit` — `Deleteprojectspendlimit` (json).
    /// Transports: http_json.
    pub async fn deleteprojectspendlimit(
        &self,
        request: DeleteprojectspendlimitParams,
    ) -> PlatformResult<DeleteprojectspendlimitResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/spend_limit` — `Updateprojectspendlimit` (json).
    /// Transports: http_json.
    pub async fn updateprojectspendlimit(
        &self,
        request: UpdateprojectspendlimitParams,
    ) -> PlatformResult<UpdateprojectspendlimitResult> {
        let mut path = String::from("/organization/projects/{project_id}/spend_limit");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/users` — `list-project-users` (json).
    /// Transports: http_json.
    pub async fn list_project_users(
        &self,
        request: ListProjectUsersParams,
    ) -> PlatformResult<ListProjectUsersResult> {
        let mut path = String::from("/organization/projects/{project_id}/users");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/users` — `create-project-user` (json).
    /// Transports: http_json.
    pub async fn create_project_user(
        &self,
        request: CreateProjectUserParams,
    ) -> PlatformResult<CreateProjectUserResult> {
        let mut path = String::from("/organization/projects/{project_id}/users");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/projects/{project_id}/users/{user_id}` — `retrieve-project-user` (json).
    /// Transports: http_json.
    pub async fn retrieve_project_user(
        &self,
        request: RetrieveProjectUserParams,
    ) -> PlatformResult<RetrieveProjectUserResult> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/projects/{project_id}/users/{user_id}` — `modify-project-user` (json).
    /// Transports: http_json.
    pub async fn modify_project_user(
        &self,
        request: ModifyProjectUserParams,
    ) -> PlatformResult<ModifyProjectUserResult> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/projects/{project_id}/users/{user_id}` — `delete-project-user` (json).
    /// Transports: http_json.
    pub async fn delete_project_user(
        &self,
        request: DeleteProjectUserParams,
    ) -> PlatformResult<DeleteProjectUserResult> {
        let mut path = String::from("/organization/projects/{project_id}/users/{user_id}");
        path = path.replace(
            "{project_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.project_id),
        );
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/roles` — `list-roles` (json).
    /// Transports: http_json.
    pub async fn list_roles(&self, request: ListRolesParams) -> PlatformResult<ListRolesResult> {
        let path = String::from("/organization/roles");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/roles` — `create-role` (json).
    /// Transports: http_json.
    pub async fn create_role(&self, request: CreateRoleParams) -> PlatformResult<CreateRoleResult> {
        let path = String::from("/organization/roles");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/roles/{role_id}` — `retrieve-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_role(
        &self,
        request: RetrieveRoleParams,
    ) -> PlatformResult<RetrieveRoleResult> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/roles/{role_id}` — `update-role` (json).
    /// Transports: http_json.
    pub async fn update_role(&self, request: UpdateRoleParams) -> PlatformResult<UpdateRoleResult> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/roles/{role_id}` — `delete-role` (json).
    /// Transports: http_json.
    pub async fn delete_role(&self, request: DeleteRoleParams) -> PlatformResult<DeleteRoleResult> {
        let mut path = String::from("/organization/roles/{role_id}");
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/spend_alerts` — `list-organization-spend-alerts` (json).
    /// Transports: http_json.
    pub async fn list_organization_spend_alerts(
        &self,
        request: ListOrganizationSpendAlertsParams,
    ) -> PlatformResult<ListOrganizationSpendAlertsResult> {
        let path = String::from("/organization/spend_alerts");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.before.as_ref() {
            query.insert("before".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/spend_alerts` — `create-organization-spend-alert` (json).
    /// Transports: http_json.
    pub async fn create_organization_spend_alert(
        &self,
        request: CreateOrganizationSpendAlertParams,
    ) -> PlatformResult<CreateOrganizationSpendAlertResult> {
        let path = String::from("/organization/spend_alerts");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/spend_alerts/{alert_id}` — `retrieve-organization-spend-alert` (json).
    /// Transports: http_json.
    pub async fn retrieve_organization_spend_alert(
        &self,
        request: RetrieveOrganizationSpendAlertParams,
    ) -> PlatformResult<RetrieveOrganizationSpendAlertResult> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/spend_alerts/{alert_id}` — `update-organization-spend-alert` (json).
    /// Transports: http_json.
    pub async fn update_organization_spend_alert(
        &self,
        request: UpdateOrganizationSpendAlertParams,
    ) -> PlatformResult<UpdateOrganizationSpendAlertResult> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/spend_alerts/{alert_id}` — `delete-organization-spend-alert` (json).
    /// Transports: http_json.
    pub async fn delete_organization_spend_alert(
        &self,
        request: DeleteOrganizationSpendAlertParams,
    ) -> PlatformResult<DeleteOrganizationSpendAlertResult> {
        let mut path = String::from("/organization/spend_alerts/{alert_id}");
        path = path.replace(
            "{alert_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.alert_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/spend_limit` — `Getorganizationspendlimit` (json).
    /// Transports: http_json.
    pub async fn getorganizationspendlimit(
        &self,
        _request: GetorganizationspendlimitParams,
    ) -> PlatformResult<GetorganizationspendlimitResult> {
        let path = String::from("/organization/spend_limit");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/spend_limit` — `Updateorganizationspendlimit` (json).
    /// Transports: http_json.
    pub async fn updateorganizationspendlimit(
        &self,
        request: UpdateorganizationspendlimitParams,
    ) -> PlatformResult<UpdateorganizationspendlimitResult> {
        let path = String::from("/organization/spend_limit");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/spend_limit` — `Deleteorganizationspendlimit` (json).
    /// Transports: http_json.
    pub async fn deleteorganizationspendlimit(
        &self,
        _request: DeleteorganizationspendlimitParams,
    ) -> PlatformResult<DeleteorganizationspendlimitResult> {
        let path = String::from("/organization/spend_limit");
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/audio_speeches` — `usage-audio-speeches` (json).
    /// Transports: http_json.
    pub async fn usage_audio_speeches(
        &self,
        request: UsageAudioSpeechesParams,
    ) -> PlatformResult<UsageAudioSpeechesResult> {
        let path = String::from("/organization/usage/audio_speeches");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/audio_transcriptions` — `usage-audio-transcriptions` (json).
    /// Transports: http_json.
    pub async fn usage_audio_transcriptions(
        &self,
        request: UsageAudioTranscriptionsParams,
    ) -> PlatformResult<UsageAudioTranscriptionsResult> {
        let path = String::from("/organization/usage/audio_transcriptions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/code_interpreter_sessions` — `usage-code-interpreter-sessions` (json).
    /// Transports: http_json.
    pub async fn usage_code_interpreter_sessions(
        &self,
        request: UsageCodeInterpreterSessionsParams,
    ) -> PlatformResult<UsageCodeInterpreterSessionsResult> {
        let path = String::from("/organization/usage/code_interpreter_sessions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/completions` — `usage-completions` (json).
    /// Transports: http_json.
    pub async fn usage_completions(
        &self,
        request: UsageCompletionsParams,
    ) -> PlatformResult<UsageCompletionsResult> {
        let path = String::from("/organization/usage/completions");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.batch.as_ref() {
            query.insert("batch".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/embeddings` — `usage-embeddings` (json).
    /// Transports: http_json.
    pub async fn usage_embeddings(
        &self,
        request: UsageEmbeddingsParams,
    ) -> PlatformResult<UsageEmbeddingsResult> {
        let path = String::from("/organization/usage/embeddings");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/file_search_calls` — `usage-file-search-calls` (json).
    /// Transports: http_json.
    pub async fn usage_file_search_calls(
        &self,
        request: UsageFileSearchCallsParams,
    ) -> PlatformResult<UsageFileSearchCallsResult> {
        let path = String::from("/organization/usage/file_search_calls");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.vector_store_ids.as_ref() {
            query.insert("vector_store_ids".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/images` — `usage-images` (json).
    /// Transports: http_json.
    pub async fn usage_images(
        &self,
        request: UsageImagesParams,
    ) -> PlatformResult<UsageImagesResult> {
        let path = String::from("/organization/usage/images");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.sources.as_ref() {
            query.insert("sources".into(), query_value(v));
        }
        if let Some(v) = request.sizes.as_ref() {
            query.insert("sizes".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/moderations` — `usage-moderations` (json).
    /// Transports: http_json.
    pub async fn usage_moderations(
        &self,
        request: UsageModerationsParams,
    ) -> PlatformResult<UsageModerationsResult> {
        let path = String::from("/organization/usage/moderations");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/vector_stores` — `usage-vector-stores` (json).
    /// Transports: http_json.
    pub async fn usage_vector_stores(
        &self,
        request: UsageVectorStoresParams,
    ) -> PlatformResult<UsageVectorStoresResult> {
        let path = String::from("/organization/usage/vector_stores");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/usage/web_search_calls` — `usage-web-search-calls` (json).
    /// Transports: http_json.
    pub async fn usage_web_search_calls(
        &self,
        request: UsageWebSearchCallsParams,
    ) -> PlatformResult<UsageWebSearchCallsResult> {
        let path = String::from("/organization/usage/web_search_calls");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        query.insert("start_time".into(), query_value(&request.start_time));
        if let Some(v) = request.end_time.as_ref() {
            query.insert("end_time".into(), query_value(v));
        }
        if let Some(v) = request.bucket_width.as_ref() {
            query.insert("bucket_width".into(), query_value(v));
        }
        if let Some(v) = request.project_ids.as_ref() {
            query.insert("project_ids".into(), query_value(v));
        }
        if let Some(v) = request.user_ids.as_ref() {
            query.insert("user_ids".into(), query_value(v));
        }
        if let Some(v) = request.api_key_ids.as_ref() {
            query.insert("api_key_ids".into(), query_value(v));
        }
        if let Some(v) = request.models.as_ref() {
            query.insert("models".into(), query_value(v));
        }
        if let Some(v) = request.context_levels.as_ref() {
            query.insert("context_levels".into(), query_value(v));
        }
        if let Some(v) = request.group_by.as_ref() {
            query.insert("group_by".into(), query_value(v));
        }
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.page.as_ref() {
            query.insert("page".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/users` — `list-users` (json).
    /// Transports: http_json.
    pub async fn list_users(&self, request: ListUsersParams) -> PlatformResult<ListUsersResult> {
        let path = String::from("/organization/users");
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.emails.as_ref() {
            query.insert("emails".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/users/{user_id}` — `retrieve-user` (json).
    /// Transports: http_json.
    pub async fn retrieve_user(
        &self,
        request: RetrieveUserParams,
    ) -> PlatformResult<RetrieveUserResult> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/users/{user_id}` — `modify-user` (json).
    /// Transports: http_json.
    pub async fn modify_user(&self, request: ModifyUserParams) -> PlatformResult<ModifyUserResult> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `DELETE /organization/users/{user_id}` — `delete-user` (json).
    /// Transports: http_json.
    pub async fn delete_user(&self, request: DeleteUserParams) -> PlatformResult<DeleteUserResult> {
        let mut path = String::from("/organization/users/{user_id}");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `GET /organization/users/{user_id}/roles` — `list-user-role-assignments` (json).
    /// Transports: http_json.
    pub async fn list_user_role_assignments(
        &self,
        request: ListUserRoleAssignmentsParams,
    ) -> PlatformResult<ListUserRoleAssignmentsResult> {
        let mut path = String::from("/organization/users/{user_id}/roles");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let mut query: BTreeMap<String, String> = BTreeMap::new();
        if let Some(v) = request.limit.as_ref() {
            query.insert("limit".into(), query_value(v));
        }
        if let Some(v) = request.after.as_ref() {
            query.insert("after".into(), query_value(v));
        }
        if let Some(v) = request.order.as_ref() {
            query.insert("order".into(), query_value(v));
        }
        let body: Option<serde_json::Value> = None;
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

    /// `POST /organization/users/{user_id}/roles` — `assign-user-role` (json).
    /// Transports: http_json.
    pub async fn assign_user_role(
        &self,
        request: AssignUserRoleParams,
    ) -> PlatformResult<AssignUserRoleResult> {
        let mut path = String::from("/organization/users/{user_id}/roles");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body = Some(
            serde_json::to_value(&request.body)
                .map_err(|e| PlatformError::InvalidRequest(e.to_string()))?,
        );
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

    /// `GET /organization/users/{user_id}/roles/{role_id}` — `retrieve-user-role` (json).
    /// Transports: http_json.
    pub async fn retrieve_user_role(
        &self,
        request: RetrieveUserRoleParams,
    ) -> PlatformResult<RetrieveUserRoleResult> {
        let mut path = String::from("/organization/users/{user_id}/roles/{role_id}");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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

    /// `DELETE /organization/users/{user_id}/roles/{role_id}` — `unassign-user-role` (json).
    /// Transports: http_json.
    pub async fn unassign_user_role(
        &self,
        request: UnassignUserRoleParams,
    ) -> PlatformResult<UnassignUserRoleResult> {
        let mut path = String::from("/organization/users/{user_id}/roles/{role_id}");
        path = path.replace(
            "{user_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.user_id),
        );
        path = path.replace(
            "{role_id}",
            &crate::openai_platform::url_policy::encode_path_segment(&request.role_id),
        );
        let query: BTreeMap<String, String> = BTreeMap::new();
        let body: Option<serde_json::Value> = None;
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
}
