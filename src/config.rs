use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{
    RoleClient, ServiceExt,
    model::{ClientCapabilities, ClientInfo, Implementation, InitializeRequestParam},
    service::RunningService,
    transport::{SseClientTransport, StreamableHttpClientTransport},
};
use serde::de::IntoDeserializer;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, sync::Arc};
use tokio::process::Command;

use crate::error::Error;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    #[serde(rename = "mcpServers", alias = "servers", alias = "mcpServers")]
    pub servers: HashMap<Arc<str>, Arc<McpServerConfig>>,
}

impl Config {
    pub fn read<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        Ok(serde_json::from_reader(std::fs::File::open(&path)?)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct McpSseConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Arc<str>>,
    url: Arc<str>,
}

impl McpSseConfig {
    async fn create_client(
        &self,
        client_info: ClientInfo,
    ) -> Result<Arc<RunningService<RoleClient, InitializeRequestParam>>, Error> {
        let transport = SseClientTransport::start(self.url.clone()).await?;

        let client = client_info
            .serve(transport)
            .await
            .map(Arc::new)
            .inspect_err(|e| {
                tracing::error!("client error: {:?}", e);
            })?;

        Ok(client)
    }
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

impl<T: Into<Arc<str>>> From<T> for McpSseConfig {
    fn from(value: T) -> Self {
        Self {
            url: value.into(),
            name: None,
            description: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct McpStdioConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Arc<str>>,
    command: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    env: Option<HashMap<String, String>>,
}

impl McpStdioConfig {
    async fn create_client(
        &self,
        client_info: ClientInfo,
    ) -> Result<Arc<RunningService<RoleClient, InitializeRequestParam>>, Error> {
        let client = client_info
            .serve(TokioChildProcess::new(
                Command::new(&self.command).configure(|cmd| {
                    for arg in &self.args {
                        cmd.arg(arg);
                    }
                    if let Some(cwd) = self.cwd.as_deref() {
                        cmd.current_dir(cwd);
                    }
                    if let Some(env) = self.env.as_ref() {
                        for (n, v) in env.iter() {
                            cmd.env(n, v);
                        }
                    }
                }),
            )?)
            .await
            .map(Arc::new)?;

        Ok(client)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct McpStreamableConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<Arc<str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Arc<str>>,
    url: Arc<str>,
}

impl McpStreamableConfig {
    async fn create_client(
        &self,
        client_info: ClientInfo,
    ) -> Result<Arc<RunningService<RoleClient, InitializeRequestParam>>, Error> {
        let transport = StreamableHttpClientTransport::from_uri(self.url.clone());
        let client = client_info
            .serve(transport)
            .await
            .map(Arc::new)
            .inspect_err(|e| {
                tracing::error!("client error: {:?}", e);
            })?;

        Ok(client)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
}

impl<T: Into<Arc<str>>> From<T> for McpStreamableConfig {
    fn from(value: T) -> Self {
        Self {
            url: value.into(),
            name: None,
            description: None,
        }
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum McpServerConfig {
    #[serde(rename = "sse")]
    Sse(McpSseConfig),
    #[serde(rename = "stdio")]
    Stdio(McpStdioConfig),
    #[serde(rename = "streamableHttp", alias = "streamable")]
    Streamable(McpStreamableConfig),
}

impl McpServerConfig {
    pub async fn create_client(
        &self,
        client_info: Option<ClientInfo>,
    ) -> Result<Arc<RunningService<RoleClient, InitializeRequestParam>>, Error> {
        let client_info = client_info.unwrap_or_else(|| ClientInfo {
            protocol_version: Default::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test sse client".to_string(),
                version: "0.0.1".to_string(),
            },
        });
        match self {
            McpServerConfig::Sse(config) => config.create_client(client_info).await,
            McpServerConfig::Stdio(config) => config.create_client(client_info).await,
            McpServerConfig::Streamable(config) => config.create_client(client_info).await,
        }
    }

    pub fn to_sse<T: Into<Arc<str>>>(&self, url: T) -> Self {
        Self::Sse(McpSseConfig {
            name: self.name().map(|s| s.into()),
            description: self.description().map(|s| s.into()),
            url: url.into(),
        })
    }

    pub fn to_streamable<T: Into<Arc<str>>>(&self, url: T) -> Self {
        Self::Streamable(McpStreamableConfig {
            name: self.name().map(|s| s.into()),
            description: self.description().map(|s| s.into()),
            url: url.into(),
        })
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            McpServerConfig::Sse(c) => c.name(),
            McpServerConfig::Stdio(c) => c.name(),
            McpServerConfig::Streamable(c) => c.name(),
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            McpServerConfig::Sse(c) => c.description(),
            McpServerConfig::Stdio(c) => c.description(),
            McpServerConfig::Streamable(c) => c.description(),
        }
    }
}

impl From<McpSseConfig> for McpServerConfig {
    fn from(value: McpSseConfig) -> Self {
        Self::Sse(value)
    }
}

impl From<McpStdioConfig> for McpServerConfig {
    fn from(value: McpStdioConfig) -> Self {
        Self::Stdio(value)
    }
}

impl From<McpStreamableConfig> for McpServerConfig {
    fn from(value: McpStreamableConfig) -> Self {
        Self::Streamable(value)
    }
}

impl<'de> serde::Deserialize<'de> for McpServerConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use McpServerConfig::*;
        use serde::__private::de::Content::Map;
        use serde::Deserialize;
        use serde::de;

        let content = Deserialize::deserialize(deserializer)?;

        let Map(mut map) = content else {
            return Err(de::Error::invalid_type(crate::into(&content), &"map"));
        };

        let typ = map
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| matches!(n.as_str(), Some(s) if s == "type"))
            .map(|(i, _)| i)
            .next()
            .map(|i| map.remove(i))
            .map(|(_, v)| v);

        let typ = typ.as_ref().and_then(|t| t.as_str()).unwrap_or_default();

        let deserializer = Map(map).into_deserializer();

        const SSE: &str = "sse";
        const STREAMABLE: &str = "streamable";
        const STREAMABLE_HTTP: &str = "streamableHttp";
        const STDIO: &str = "stdio";

        const VARIANTS: &[&str] = &[SSE, STDIO, STREAMABLE, STREAMABLE_HTTP];

        Ok(match typ {
            SSE => Sse(Deserialize::deserialize(deserializer)?),
            STREAMABLE | STREAMABLE_HTTP => Streamable(Deserialize::deserialize(deserializer)?),
            STDIO | "" => Stdio(Deserialize::deserialize(deserializer)?),
            typ => {
                return Err(de::Error::unknown_variant(typ, VARIANTS))?;
            }
        })
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_stdio() {
        let input = r#"
        {
            "command": "echo",
            "args": [
                "hello"
            ] 
        }
        "#;

        assert_eq!(
            serde_json::from_str::<McpServerConfig>(input).unwrap(),
            McpStdioConfig {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                name: None,
                description: None,
                cwd: None,
                env: None,
            }
            .into()
        )
    }
}
