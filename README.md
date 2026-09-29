# api.support.cafe

Multi-tenant support-chat backend demonstrating [WorkTable](https://github.com/pathscale/WorkTable), [endpoint-libs](https://github.com/pathscale/endpoint-libs), and [endpoint-gen](https://github.com/pathscale/endpoint-gen).

Every endpoint is served over a single WebSocket in two protocols at once: the legacy `{method, seq, params}` protocol and **MCP** (JSON-RPC 2.0 tools, endpoint-libs ≥1.9). The per-service `docs/*_mcp_tools.json` files are the exact `tools/list` output. To add MCP support to another endpoint-libs backend, see the [migration guide](https://github.com/pathscale/endpoint-libs/blob/main/docs/mcp-migration.md).

## Registering with honey.id and setting its secrets

A platform admin creates the application in the honey.id dashboard under **Applications, Create Application**. honey.id issues the app public ID and auth API key when the application is created. Copy the key then. It is shown only once, when created or after **Regenerate API Key**.

Set `CAFE__HONEY_ID__APP_PUBLIC_ID` to the app public ID. honey.id uses it to identify this application. Set `CAFE__HONEY_ID__AUTH_API_KEY` to the app's auth API key. honey.id uses this key to authorize callbacks to the application. The frontend and backend must use the same app public ID. These names are also the environment variable names the backend reads.

Set `CAFE__HONEY_ID__ADDR` to honey.id's WebSocket address. It defaults to `wss://auth.honey.id:443`. `CAFE__HONEY_ID__ADMIN_PUB_ID` is optional. If set, the backend gives that existing local user the `Admin` role at startup. If the user does not exist yet, startup continues without promoting them.

Production uses Doppler. The Fly app `support-cafe-master-fly` holds only the Doppler loader variables as Fly secrets: `CAFE_SECRETS_ENABLED`, `CAFE_SECRETS_DOPPLER_SERVICE_TOKEN`, `CAFE_SECRETS_DOPPLER_PROJECT`, and `CAFE_SECRETS_DOPPLER_CONFIG`. Store the honey.id values in the Doppler project and config named by the last two variables. The deploy workflow deploys the image; it does not set these values.

To see which Doppler project and config the running app uses:

```sh
fly ssh console -a support-cafe-master-fly -C 'sh -c "echo $CAFE_SECRETS_DOPPLER_PROJECT $CAFE_SECRETS_DOPPLER_CONFIG"'
```

Set the app ID and, if needed, the address or admin public ID in that Doppler project and config. Replace the placeholders with the values from the command above.

```sh
doppler secrets set CAFE__HONEY_ID__APP_PUBLIC_ID=<app public id> --project <project> --config <config>
doppler secrets set CAFE__HONEY_ID__ADDR=<auth address> --project <project> --config <config>
doppler secrets set CAFE__HONEY_ID__ADMIN_PUB_ID=<admin user public id> --project <project> --config <config>
```

The address and admin ID are optional when using the default address and no admin bootstrap. Set the auth API key without putting it on the command line or in shell history:

```sh
read -rs KEY
printf %s "$KEY" | doppler secrets set CAFE__HONEY_ID__AUTH_API_KEY --project <project> --config <config> && unset KEY
```

After a Doppler change, restart the app so it reloads the values:

```sh
fly apps restart support-cafe-master-fly
```

Sign in on the site with an unknown username. The response should be `User not found`. `App not found` means auth.honey.id does not know the app public ID the frontend or backend is using.

See the [fleet-wide procedure](https://github.com/pathscale/honey.id/blob/master/docs/site-auth-and-qa-fixtures.md#registering-a-site-with-honeyid).
