use async_trait::async_trait;
use azure_core::http::{
    Context, Request,
    policies::{Policy, PolicyResult},
};
use std::sync::Arc;

// Define a policy that will remove the User-Agent header.
// https://github.com/Azure/azure-sdk-for-rust#data-collection
#[derive(Debug)]
pub(crate) struct RemoveUserAgent;

#[async_trait]
impl Policy for RemoveUserAgent {
    async fn send(
        &self,
        ctx: &Context,
        request: &mut Request,
        next: &[Arc<dyn Policy>],
    ) -> PolicyResult {
        let headers = request.headers_mut();

        // Note: HTTP headers are case-insensitive but client-added headers are normalized to lowercase
        headers.remove("user-agent");

        next[0].send(ctx, request, &next[1..]).await
    }
}
