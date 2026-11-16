# Anonymous widget connections

Connect over WSS using the AppConnect method (20000). Offer these WebSocket
subprotocol tokens in order.

| Action | Offer |
| --- | --- |
| Start a new visitor | `0appconnect,1<appPublicId>` |
| Reconnect | `0appconnect,1<appPublicId>,2<userPublicId>,3<visitorSecret>` |

Wait for the successful method-20000 frame before using chat RPCs. The
response includes `appPublicId`, `appName` and `userPublicId`. The first
successful connection includes `visitorSecret` as 64 hexadecimal characters.
On reconnect `visitorSecret` is null and the visitor ID stays the same.

Save the ID and secret together in widget storage scoped to the API and app.
Keep the secret out of URLs, analytics and logs. For CreateChatSession (20001)
use the returned ID as `userPubId`. SubmitContactMessage (20009) uses the
identity of the connection. Treat a rejected resume as a failed sign-in.
Starting a new identity is a separate action and uses the two-token offer.

A visitor with only an ID from an earlier version starts a new identity.
Existing chat records remain available to authorized staff and account users
through their existing access paths.

## Connection limits

Each app accepts at most 64 open widget connections at a time. A new visitor
takes a place before its credential is issued. A reconnect takes a place
only after its secret is checked. When the app is full, AppConnect fails with
TooManyRequests (100429). Closing the connection frees its place.

The server does not limit connections per client yet. Behind Fly every
connection arrives from a Fly proxy address, so the server cannot tell clients
apart by address, and `fly.toml` does not enable `proxy_proto`. A later change
may enable `proxy_proto` to recover the client address for per-client limits.

## Upgrade an existing store

Follow the [WorkTable cutover](worktable-v3-cutover.md) for the storage format
in use. Stop the existing writer. With the reviewed new service binary and
the existing store configuration, run the service with
`--migrate-visitor-credentials` before starting it normally. The command
reserves the existing session owner IDs before committing a completion
record. Keep writers stopped until the command succeeds. If the command
is interrupted, retry with the same store. If WorkTable reports unreadable
data, recover the affected table before continuing. New stores initialize
the reservation table at startup. The CI deploy job stops unless it is
dispatched with `visitor_migration_confirmed` set, so run the migration
before you deploy.

Keep one writer per database and storage prefix. Roll out the updated widget
with the new server. A server version that accepts a bare visitor ID cannot
serve the same traffic during the cutover or rollback.

When using `worktable_rebuild` for a migration into a fresh S3 prefix, give it
an owned scratch directory with `WORKTABLE_REBUILD_DIR` and an unused target
prefix with `WORKTABLE_REBUILD_TARGET_PREFIX`. Stop writes to the source
while rebuilding. Set
`WORKTABLE_REBUILD_LEGACY_VISITORS=1` only for a confirmed pre-credential
source. For a source that already serves credential-based visitors, leave
that variable unset so the rebuild copies the issued credentials and
reservations. After an interrupted rebuild, use a fresh scratch directory
and an unused target prefix. Use the target only after a successful rebuild
and cold-open verification.
