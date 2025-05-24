# MCP Gateway

The MCP Gateway is a lightweight tool that can convert the `mcpServers` configuration in configuration files into **Streamable HTTP** or **Server-Sent Events (SSE)** HTTP services for remote invocation.

## Features
- Supports multiple backend service types: stdio/SSE/streamableHttp
- Provides a unified service discovery interface
- Supports various runtime methods: Docker/Podman/NPM, etc.
- Lightweight deployment with single binary execution

## Quick Start

### 1. Download the Program
Get the binary for your platform from the releases page.

### 2. Configure MCP Server
Create a configuration file `./config.json`:

```json
{
    "mcpServers": {
        "filesystem": {
            "name": "File System",
            "description": "Operations on file system files",
            "type": "stdio",
            "command": "podman",
            "cwd": "./work/filesystem",
            "args": [
                "run",
                "-i",
                "--rm",
                "-v", "./:/projects",
                "--user", "0:0",
                "mcp/filesystem",
                "/projects"
            ]
        },
        "sequentialthinking": {
            "name": "Sequential Thinking",
            "description": "A tool for dynamic reflective problem-solving through structured thought processes",
            "type": "stdio",
            "command": "podman",
            "args": [
                "run",
                "-i",
                "--rm",
                "mcp/sequentialthinking"
            ]
        }
    }
}
```

### 3. Start the Service
```bash
./mcpgate -C ./config.json -P 8080
```

### 4. Verify the Service
Access the service discovery interface in your browser:
```
http://localhost:8080/mcp/config.json
```

## Service Configuration Methods

### 1. Using Existing HTTP Services
Directly configure remote SSE or Streamable HTTP endpoints:

```json
{
    "mcpServers": {
        "sequentialthinking_sse": {
            "type": "sse",
            "url": "https://mcpserver.com/sequentialthinking/sse"
        },
        "sequentialthinking_http": {
            "type": "streamable",
            "url": "https://mcpserver.com/sequentialthinking"
        }
    }
}
```

### 2. Using Container Runtime
Get official images from [Docker Hub - MCP](https://hub.docker.com/u/mcp):

```json
{
    "mcpServers": {
        "sequentialthinking": {
            "type": "stdio",
            "command": "docker",
            "args": [
                "run",
                "-i",
                "--rm",
                "mcp/sequentialthinking"
            ]
        }
    }
}
```

### 3. Using NPM Packages
Requires pre-installed [Node.js](https://nodejs.org/) environment:

```json
{
    "mcpServers": {
        "memory": {
            "type": "stdio",
            "command": "npx",
            "args": [
                "run",
                "-y",
                "@modelcontextprotocol/server-memory"
            ]
        }
    }
}
```

### 4. Custom Service Development
Refer to language SDKs to develop custom services:

#### Python Example
Using [Python SDK](https://github.com/modelcontextprotocol/python-sdk):

```json
{
    "mcpServers": {
        "custom_server": {
            "type": "stdio",
            "command": "python",
            "args": ["server.py"]
        }
    }
}
```

## Advanced Configuration
| Parameter | Description | Default |
|-----------|-------------|---------|
| -C        | Configuration file path | ./config.json |
| -H        | Bind IP | 0.0.0.0 |
| -P        | Service listening port | 8080 |

## Notes
1. Ensure stdio-type services are designed for standard input/output
2. SSE services must support `text/event-stream` format
3. HTTPS configuration is recommended for production environments

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.