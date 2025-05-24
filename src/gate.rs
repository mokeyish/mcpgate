use std::sync::Arc;

use rmcp::Error as McpError;
use rmcp::{
    RoleClient, RoleServer, Service, ServiceError,
    model::{
        ClientNotification, ClientRequest, ErrorCode, InitializeRequestParam, ListPromptsResult,
        ListResourceTemplatesResult, ListResourcesResult, ListToolsResult, ServerInfo,
        ServerResult,
    },
    service::{RequestContext, RunningService, ServiceRole},
};
use tokio::sync::RwLock;

use crate::config::McpServerConfig;
use crate::error::Error;

#[derive(Debug)]
pub struct Gate {
    config: Arc<McpServerConfig>,
    client: RwLock<Option<Arc<RunningService<RoleClient, InitializeRequestParam>>>>,
}

impl Gate {
    pub fn new(config: Arc<McpServerConfig>) -> Self {
        Self {
            config,
            client: Default::default(),
        }
    }
}

impl Service<RoleServer> for Gate {
    async fn handle_request(
        &self,
        request: <RoleServer as ServiceRole>::PeerReq,
        ctx: RequestContext<RoleServer>,
    ) -> Result<<RoleServer as ServiceRole>::Resp, McpError> {
        match request {
            ClientRequest::InitializeRequest(_) => {
                let client_info = ctx.peer.peer_info().cloned();
                let client = self.config.create_client(client_info).await?;
                *(self.client.write().await) = Some(client.clone());

                let res = client.peer_info().cloned().unwrap_or_default();

                Ok(ServerResult::InitializeResult(res))
            }
            ClientRequest::PingRequest(_) => Ok(ServerResult::empty(())),
            ClientRequest::CompleteRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();

                let res = client.complete(request.params).await.map_err(mcp_err)?;

                Ok(ServerResult::CompleteResult(res))
            }
            ClientRequest::SetLevelRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();

                client.set_level(request.params).await.map_err(mcp_err)?;

                Ok(ServerResult::empty(()))
            }
            ClientRequest::GetPromptRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                let res = client.get_prompt(request.params).await.map_err(mcp_err)?;
                Ok(ServerResult::GetPromptResult(res))
            }
            ClientRequest::ListPromptsRequest(_) => {
                let client = self.client.read().await.as_ref().unwrap().clone();

                let prompts = client.list_all_prompts().await.map_err(mcp_err)?;

                Ok(ServerResult::ListPromptsResult(ListPromptsResult {
                    next_cursor: None,
                    prompts,
                }))
            }
            ClientRequest::ListResourcesRequest(_) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                let resources = client.list_all_resources().await.map_err(mcp_err)?;
                Ok(ServerResult::ListResourcesResult(ListResourcesResult {
                    next_cursor: None,
                    resources,
                }))
            }
            ClientRequest::ListResourceTemplatesRequest(_) => {
                let client = self.client.read().await.as_ref().unwrap().clone();

                let resource_templates = client
                    .list_all_resource_templates()
                    .await
                    .map_err(mcp_err)?;

                Ok(ServerResult::ListResourceTemplatesResult(
                    ListResourceTemplatesResult {
                        next_cursor: None,
                        resource_templates,
                    },
                ))
            }
            ClientRequest::ReadResourceRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();

                let res = client
                    .read_resource(request.params)
                    .await
                    .map_err(mcp_err)?;
                Ok(ServerResult::ReadResourceResult(res))
            }
            ClientRequest::SubscribeRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                client.subscribe(request.params).await.map_err(mcp_err)?;
                Ok(ServerResult::empty(()))
            }
            ClientRequest::UnsubscribeRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                client.unsubscribe(request.params).await.map_err(mcp_err)?;
                Ok(ServerResult::empty(()))
            }
            ClientRequest::CallToolRequest(request) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                let res = client.call_tool(request.params).await.map_err(mcp_err)?;
                Ok(ServerResult::CallToolResult(res))
            }
            ClientRequest::ListToolsRequest(_) => {
                let client = self.client.read().await.as_ref().unwrap().clone();
                let tools = client.list_all_tools().await.map_err(mcp_err)?;
                Ok(ServerResult::ListToolsResult(ListToolsResult {
                    next_cursor: None,
                    tools,
                }))
            }
        }
    }

    async fn handle_notification(
        &self,
        notification: <RoleServer as ServiceRole>::PeerNot,
    ) -> Result<(), McpError> {
        match notification {
            ClientNotification::CancelledNotification(_) => Ok(()),
            ClientNotification::ProgressNotification(_) => Ok(()),
            ClientNotification::InitializedNotification(_notification) => Ok(()),
            ClientNotification::RootsListChangedNotification(_notification) => Ok(()),
        }
    }

    fn get_info(&self) -> <RoleServer as ServiceRole>::Info {
        ServerInfo::default()
    }
}

fn mcp_err(err: ServiceError) -> McpError {
    McpError::new(ErrorCode::INTERNAL_ERROR, err.to_string(), None)
}

impl From<Error> for McpError {
    fn from(err: Error) -> Self {
        McpError::new(ErrorCode::INTERNAL_ERROR, err.to_string(), None)
    }
}
