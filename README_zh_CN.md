# MCP 网关

MCP 网关是一个轻量级工具，能够将配置文件中的 `mcpServers` 配置一次性转换为 **Streamable HTTP** 或 **Server-Sent Events (SSE)** HTTP 服务，方便远程调用。

## 功能特性
- 支持多种后端服务类型：stdio/SSE/streamableHttp
- 提供统一的服务发现接口
- 支持 Docker/Podman/NPM 等多种运行方式
- 轻量级部署，单二进制运行

## 快速开始

### 1. 下载程序
从发布页面获取对应平台的二进制文件。

### 2. 配置 MCP Server
创建配置文件 `./config.json`：

```json
{
    "mcpServers": {
        "filesystem": {
            "name": "文件系统",
            "description": "操作文件系统的文件",
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
            "name": "顺序思考",
            "description": "通过结构化思维过程实现动态反思性解决问题的工具",
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

### 3. 启动服务
```bash
./mcpgate -C ./config.json -P 8080
```

### 4. 验证服务
浏览器访问服务发现接口：
```
http://localhost:8080/mcp/config.json
```

## 服务配置方式

### 1. 使用现有 HTTP 服务
直接配置远程 SSE 或 Streamable HTTP 端点：

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

### 2. 使用容器运行时
访问 [Docker Hub - MCP](https://hub.docker.com/u/mcp) 获取官方镜像：

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

### 3. 使用 NPM 包
需预先安装 [Node.js](https://nodejs.org/) 环境：

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

### 4. 自定义服务开发
参考各语言 SDK 开发自定义服务：

#### Python 示例
使用 [Python SDK](https://github.com/modelcontextprotocol/python-sdk):

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

## 高级配置
| 参数 | 说明 | 默认值 |
|------|------|--------|
| -C   | 配置文件路径 | ./config.json |
| -H   | 绑定 IP | 0.0.0.0 |
| -P   | 服务监听端口 | 8080 |

## 注意事项
1. 确保 stdio 类型的服务是面向标准输入输出的
2. SSE 服务需支持 `text/event-stream` 格式
3. 生产环境建议配置 HTTPS