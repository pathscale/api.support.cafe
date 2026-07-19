# api.support.cafe

Multi-tenant support-chat backend demonstrating [WorkTable](https://github.com/pathscale/WorkTable), [endpoint-libs](https://github.com/pathscale/endpoint-libs), and [endpoint-gen](https://github.com/pathscale/endpoint-gen).

Every endpoint is served over a single WebSocket in two protocols at once: the legacy `{method, seq, params}` protocol and **MCP** (JSON-RPC 2.0 tools, endpoint-libs ≥1.9). The per-service `docs/*_mcp_tools.json` files are the exact `tools/list` output. To add MCP support to another endpoint-libs backend, see the [migration guide](https://github.com/pathscale/endpoint-libs/blob/main/docs/mcp-migration.md).
