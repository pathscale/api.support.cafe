# Fly deployment, and how to move to Fly-terminated TLS

This service is reached at `api.support.cafe`, config prefix `CAFE__`. It has
no separate prod app, so the master instance is the live one and the migration
below is **not** a no-risk change here. It currently terminates its own TLS.

This file describes this repo only. Other backends in the fleet are private
and their app names, hostnames and posture do not belong in a public
repository: if you need the fleet-wide picture, it lives with those services.

## What the dev instance is for

Functional, end-to-end testing of frontends against a real backend. Not a
mock, not a local process, and **not Docker**, which is out by design. The
frontends are driven headlessly by `ps-qa` against a real browser engine, and
a sign-in has to complete for any check past "the page painted" to mean
anything.

Authentication happens over the WebSocket handshake, passing the token in
`Sec-WebSocket-Protocol` as `["0init", "1<token>"]`.

The app sleeps (`min_machines_running = 0`) and wakes on connect in about
three seconds. **Nothing needs starting by hand.**

Use the real hostname, never the `.fly.dev` one. While a service is
app-terminated its `.fly.dev` name does not work at all, because the app
serves its own ACME certificate and the name fails on SNI. Once it is
Fly-terminated the `.fly.dev` name starts working, and that is a trap rather
than a convenience: it served 200 throughout a broken migration in which the
real hostname was down the whole time.

## Why a service might terminate its own TLS

**RFC 8441 WebSocket over HTTP/2**, extended CONNECT. Negotiating h2 needs
ALPN, ALPN needs owning the TLS handshake, and owning the handshake needs Fly
out of the path. That is what `handlers = []` buys, and a dedicated IPv4 is
its price, because Fly's shared IPv4 only works behind their HTTP handlers.

A service whose surface is request/response with nothing server-pushed does
not need that: a plain HTTP/1.1 `Upgrade` is enough, and it can let Fly
terminate.

A **dev** instance has the same question and a different answer: it does not
need h2 WSS, so it should not pay for it.

## The consequence nobody planned: TLS 1.3 only

An app-terminated build here refuses TLS 1.2:

    api.support.cafe    tls1.2=REFUSED  tls1.3=ok

That is a build choice, not a defect: endpoint-libs has a `ws-tls12` feature
and this service does not enable it. Any modern client is fine.

It is worth knowing because it is how the whole thing was found. `blitz-net`
builds `reqwest` with `native-tls`, which on macOS offers a TLS 1.2
ClientHello with no `supported_versions` extension, so the browser engine
under test cannot reach the backend. The server says so plainly:

    ERROR endpoint_libs::libs::ws::server: Error while handshaking stream:
    peer is incompatible: SupportedVersionsExtensionRequired

**Do not fix that by relaxing a server.** The engine needs TLS 1.3.

And note the trap this creates: once a dev instance is Fly-terminated it stops
exercising the production TLS path, so a TLS 1.2 client will pass against dev
and still fail against production. Assert the handshake directly, do not infer
it from a green dev run.

## Moving to Fly-terminated TLS

### Copy a config that works. Do not improvise.

The whole cost of this exercise was writing a config by hand instead of
copying one that already works. `[[services]]` with explicit `handlers` and a
`tls_options` block deploys fine, serves the `.fly.dev` hostname fine, and
**never gets a certificate for the custom hostname**, because
`tls_options.versions = ["TLSv1.3"]` blocks Fly's own ACME validation. Fifteen
minutes of dev downtime and three theories later, copying the shape of a
working Fly-terminated service issued the certificate on the first poll, in
under twenty seconds.

Set no `tls_options` at all. Do not set `alpn` either: TLS-ALPN-01 needs
`acme-tls/1`, and listing only `h2` and `http/1.1` removes it.

If you want a TLS 1.3 floor, that is a separate change made **after** the
certificate is `Issued`, and it needs re-verifying that renewal still works.

### The config

Needs no code change. The loader is file, then Doppler, then environment, and
environment wins, so the address and transport come from `[env]`:

```toml
[env]
  CAFE__SERVER__ADDRESS = "0.0.0.0:8080"
  CAFE__SERVER__INSECURE = "true"

[http_service]
  internal_port = 8080
  force_https = true
  # A dev instance should sleep. A service that must never wait on a cold
  # build keeps a machine resident instead.
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0
```

`insecure` is a bad name for what this is. The app speaks plain HTTP on its
internal port and Fly terminates TLS at the edge; every client still gets TLS
via `force_https`.

### `http_service` carries the WebSocket. Verified, not assumed.

The reasonable worry is that this is a WebSocket service and `http_service` is
for HTTP. A WebSocket is an HTTP/1.1 upgrade, and Fly proxies it. Measured
with a real client after a move, the `0init` subprotocol negotiates, which is
the half that carries the auth token, so the handshake actually depended on
works.

This also disproves a blocker recorded earlier: building endpoint-libs with
`["ws"]` and no `ws-http1` was read as "h2 extended CONNECT only, so it cannot
sit behind Fly". It answers a plain HTTP/1.1 upgrade regardless. No code
change is needed to move.

### Order of operations, with no outage

An outage here is avoidable. A hostname can reach `Issued` from the DNS
records alone, with its config untouched, which means the certificate can be
ready **before** anything is deployed.

1. `fly certs add <hostname> -a <app>`. Inert while `handlers = []`.
2. Add both records. Neither is sufficient alone:

       AAAA  <sub>                  <the app's dedicated IPv6>
       CNAME _acme-challenge.<sub>  <from `fly certs setup`, a *.flydns.net name>

3. Poll `fly certs check <hostname>` until `Issued`. **Wait here.** This is
   the step that makes the rest of it free.
4. Deploy the config above with the existing image, no rebuild:

       fly deploy --config fly.toml -a <app> \
         --image registry.fly.io/<app>:<tag> --ha=false

   `fly image show -a <app>` gives the tag.
5. Verify the real hostname, never the `.fly.dev` one. The `.fly.dev` name
   served 200 throughout a broken attempt, which is exactly why it is useless
   as a check: it says the plumbing works and nothing about the hostname
   clients use.

       curl -o /dev/null -w '%{http_code}' https://<hostname>/
       cargo run --manifest-path ../EndpointValidator/ws-load-test/Cargo.toml \
         --bin ws-simple -- --server-url wss://<hostname> --type connection \
         --num-parallel 1 --num-requests 1 --protocol-header-file <header-file>

6. Swap the A record to Fly's shared IPv4 and release the dedicated one. Only
   now, and only once steps 4 and 5 pass. Prove it works first; save the $2
   second.

### Always take a shared IPv4. Never allocate a dedicated one

A dedicated v4 costs $2/mo and buys nothing once Fly terminates. Fly routes a
shared address by reading the TLS ClientHello SNI, so it never has to decrypt.

    fly ips allocate-v4 --shared -a <app>

Allocating is additive and free, so hold both while you check, then release
the dedicated one.

### Rollback, which was needed and which works

    fly deploy --config <the committed app-terminated toml> -a <app> \
      --image registry.fly.io/<app>:<tag> --ha=false

Back to app-terminated in about thirty seconds. Expect one `000` on the first
request afterwards: the machine auto-stopped and is waking.
