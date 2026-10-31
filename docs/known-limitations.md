# Known security limitations

These came out of the security audit of 2026-10-14. Each is known and accepted
for now. Read this before building on the parts it names.

## Public chat trusts a visitor id the widget mints

`AppConnect` is public on purpose: a website visitor chats with support without
a honey.id login. The widget mints a random 16-character visitor id, keeps it in
the visitor's browser, and presents it on connect. That id is the visitor's only
credential. Whoever holds it can read that visitor's chats and write as them.

What protects it today:

- The id is about 95 bits of randomness when the widget mints it properly.
- `AppConnect` refuses an app that does not exist or is inactive, and refuses
  the public id of a registered account, which signs in with `Init` instead.
- `CreateChatSession` opens a session only for the connection's own visitor.

What is left:

- An app's staff see its visitors' ids and could connect as one of them.
- The server cannot check that a widget mints its ids at random.

No client calls `AppConnect` yet. Build this together with the widget: when
`AppConnect` arrives without an id, the server mints a visitor id and a 32-byte
secret, stores only the secret's hash, and returns both once. Reconnecting
needs both. Staff never see the secret. This adds request and response fields
and a persisted table, so it ships as a WorkTable deployment event.

## No rate limits or connection caps

Nothing limits connections per address or per app, or how fast sessions and
messages are created. Contact-form messages have a length cap; chat messages
do not. At the current scale this is accepted. Add limits before public chat
goes live.

## Connection registries are not cleaned up on disconnect

`AppConnect` and `Init` record each connection's app and user. Nothing removes
the entry when the connection closes, so the maps grow with every handshake
until the process restarts. endpoint-libs offers `add_on_disconnect_hook`;
unregistering there fixes it.

## Telegram handles are not verified

A support member claims a Telegram handle by typing it, and `/start` binds
whichever chat sends it. Nothing proves the member owns the handle, and handles
are stored as typed although Telegram treats them case-insensitively. Fix by
lowercasing handles and binding through a one-time code the member sends to the
bot.

## CI hardening

The deploy job passes the Fly token to `buildah login` on the command line,
installs `cargo-sweep` without a pinned version into a cached directory, and
pins actions by tag rather than commit. A compromised dependency or cache could
read the token. Fix with `--password-stdin`, a pinned `--locked` install
outside the cache, and actions pinned by commit.
