//! Private, validated exact model routes for child-session assignment.
//!
//! Public task and workflow payloads retain their legacy model strings. The
//! shell resolves those strings once at a trusted spawn boundary and carries
//! this value privately thereafter.

use xai_grok_inference::ProviderRouteContext;
use xai_grok_models::{CanonicalModelId, UpstreamModelId};

#[derive(Clone, Debug)]
pub(super) struct ExactRoute {
    canonical: CanonicalModelId,
    upstream: UpstreamModelId,
    context: ProviderRouteContext,
}

impl ExactRoute {
    pub(super) fn new(
        canonical: CanonicalModelId,
        upstream: UpstreamModelId,
        context: ProviderRouteContext,
    ) -> Option<Self> {
        (context.model_partition() == Some(upstream.as_str())).then_some(Self {
            canonical,
            upstream,
            context,
        })
    }

    pub(super) fn canonical(&self) -> &CanonicalModelId {
        &self.canonical
    }

    pub(super) fn upstream(&self) -> &UpstreamModelId {
        &self.upstream
    }

    pub(super) fn context(&self) -> &ProviderRouteContext {
        &self.context
    }

    /// Exact routes never silently bind a sibling account, a recreated
    /// provider, or a rotated durable credential binding.
    pub(super) fn matches_live(&self, live: &Self) -> bool {
        self.canonical == live.canonical
            && self.upstream == live.upstream
            && self.context.instance_id() == live.context.instance_id()
            && self.context.incarnation() == live.context.incarnation()
            && self.context.provider_kind() == live.context.provider_kind()
            && self.context.api_surface() == live.context.api_surface()
            && self.context.credential_route() == live.context.credential_route()
            && self.context.registry_generation() == live.context.registry_generation()
            && self.context.binding_generation() == live.context.binding_generation()
            && self.context.authority() == live.context.authority()
            && self.context.model_partition() == Some(self.upstream.as_str())
            && live.context.model_partition() == Some(live.upstream.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xai_grok_inference::{RouteApiSurface, RouteCredentialRoute, RouteProviderKind};

    fn route(instance: &str, binding: u64) -> ExactRoute {
        let upstream = UpstreamModelId::new("gpt-4o").unwrap();
        ExactRoute::new(
            CanonicalModelId::new(format!("{instance}:gpt-4o")).unwrap(),
            upstream,
            ProviderRouteContext::builder()
                .instance_id(instance)
                .incarnation("01234567-89ab-cdef-0123-456789abcdef")
                .provider_kind(RouteProviderKind::OpenAi)
                .api_surface(RouteApiSurface::OpenAiPlatform)
                .credential_route(RouteCredentialRoute::ApiKey)
                .registry_generation(9)
                .binding_generation(binding)
                .model_partition("gpt-4o")
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn exact_route_rejects_sibling_and_credential_rotation() {
        let primary = route("work-openai", 4);
        assert!(!primary.matches_live(&route("home-openai", 4)));
        assert!(!primary.matches_live(&route("work-openai", 5)));
        assert!(primary.matches_live(&route("work-openai", 4)));
    }
}
