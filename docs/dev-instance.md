# The dev instance, and how to move it to Fly-terminated TLS

**This repo** is `support-cafe-master-fly`, reached at `api.support.cafe`, config prefix `CAFE__`. It has no separate prod app, so the master instance is the live one and the migration below is not a no-risk change here. It terminates its own TLS.

Written 2026-09-08 while moving `auth-honey-id-master-fly` to Fly-terminated
TLS, so the parts that did not work are here too. **This file is duplicated
verbatim in every backend that deploys to Fly**, because they all have the
same shape and none of them shares a docs tree. If you change it, change the
others; `auth.honey.id-backend` is where it was written first.

## What the dev instance is for

Functional, end-to-end testing of the fleet's frontends against a real
backend. Not a mock, not a local process, and **not Docker**, which is out by
design. The frontends are driven headlessly by `ps-qa` against a real browser
engine, and a sign-in has to complete for any check past "the page painted" to
mean anything.

Eight of the thirteen fleet sites authenticate over the WebSocket handshake,
passing the token in `Sec-WebSocket-Protocol` as `["0init", "1<token>"]`, and
they all point at honey.id. That is why honey.id gates the whole programme:
one auth contract, one dev instance, every site unblocked at once.

## The fleet, and which repo owns which app

Five repositories deploy to Fly. Every one of them serves WebSocket over
endpoint-libs, and all but one terminates its own TLS.

| repo | Fly app | prod app | hostname | config prefix | TLS |
|---|---|---|---|---|---|
| `auth.honey.id-backend` | `auth-honey-id-master-fly` | `auth-honey-id-prod-fly` | `auth-dev.honey.id` | `AUTH__` | **Fly**, since 2026-09-08 |
| `api.honey.id-backend` | `api-honey-id-master-fly` | `api-honey-id-prod-fly` | `api-dev.honey.id` | `API__` | the app |
| `nofilter.io-backend` | `api-nofilter-io-master-fly` (suspended) | `api-nofilter-io-prod-fly` | `api-dev.nofilter.io` | `NF__` | the app |
| `api.support.cafe` | `support-cafe-master-fly` | none | `api.support.cafe` | `CAFE__` | the app |
| `crates.vip-backend` | `api-crates-vip-master-fly` | none | `api.crates.vip` | `CRATES_VIP__` | **Fly** (the template) |
| `pays.online-backend` | not on Fly yet | none | none yet | | **start on Fly** |

`api-pathscale-master-fly` also exists on Fly, suspended since April 2026,
with no repository in `~/code` that deploys it.

`pays.online-backend` is the next one in. It is an endpoint-libs WebSocket
server like the rest, built with `["ws"]`, and it has no Fly config yet.
**Give it `[http_service]` from the start.** Copying an existing
`fly.dev.toml` from honey or nofilter would inherit `handlers = []`, and with
it a dedicated IPv4, an in-process ACME client, a Bunny DNS-01 challenge and a
TLS 1.3 floor that no client of ours needs. `pays.online-watcher` is not
affected either way: it consumes a stream, has no inbound traffic, and
declares no service at all.

The `*-master-fly` apps are the dev instances. They sleep
(`min_machines_running = 0`) and wake on connect in about three seconds.
**Nothing needs starting by hand.**

Use the real hostname, never the `.fly.dev` one. While a service is
app-terminated its `.fly.dev` name does not work at all, because the app
serves its own ACME certificate and the name fails on SNI. Once it is
Fly-terminated the `.fly.dev` name starts working, and that is a trap rather
than a convenience: it served 200 throughout a broken migration in which the
real hostname was down the whole time.

## Why they terminate their own TLS

One reason, and it is written up in `crates.vip-backend/docs/deploy-pipeline.md`:
**RFC 8441 WebSocket over HTTP/2**, extended CONNECT. Negotiating h2 needs
ALPN, ALPN needs owning the TLS handshake, and owning the handshake needs Fly
out of the path. That is what `handlers = []` buys, and the dedicated IPv4 is
its price, because Fly's shared IPv4 only works behind their HTTP handlers.

`api.crates.vip` is the fleet's one Fly-terminated service, because its surface
is request/response with nothing server-pushed, so a plain HTTP/1.1 `Upgrade`
is enough.

A **dev** instance has the same question and a different answer: it does not
need h2 WSS, so it should not pay for it.

## The consequence nobody planned: TLS 1.3 only

Every app-terminated service in the fleet refuses TLS 1.2:

    auth-dev.honey.id   tls1.2=REFUSED  tls1.3=ok
    api-dev.honey.id    tls1.2=REFUSED  tls1.3=ok
    auth.honey.id       tls1.2=REFUSED  tls1.3=ok
    api.honey.id        tls1.2=REFUSED  tls1.3=ok
    api.support.cafe    tls1.2=REFUSED  tls1.3=ok
    api.crates.vip      reachable on both (Fly terminates)

That is a build choice, not a defect: endpoint-libs has a `ws-tls12` feature
and these services do not enable it. Any modern client is fine.

It is worth knowing because it is how the whole thing was found. `blitz-net`
builds `reqwest` with `native-tls`, which on macOS offers a TLS 1.2
ClientHello with no `supported_versions` extension, so the browser engine
under test cannot reach **any** of these backends. The server says so plainly:

    ERROR endpoint_libs::libs::ws::server: Error while handshaking stream:
    peer is incompatible: SupportedVersionsExtensionRequired

**Do not fix that by relaxing a server.** The engine needs TLS 1.3.

And note the trap this creates: once a dev instance is Fly-terminated it stops
exercising the production TLS path, so a TLS 1.2 client will pass against dev
and still fail against every production backend. Assert the handshake
directly, do not infer it from a green dev run.

## Moving one to Fly-terminated TLS

**Done on `auth-honey-id-master-fly` on 2026-09-08 and verified end to end.**

### Copy crates.vip. Do not improvise.

The whole cost of this exercise was writing a config by hand instead of
copying the one that works. `[[services]]` with explicit `handlers` and a
`tls_options` block deploys fine, serves the `.fly.dev` hostname fine, and
**never gets a certificate for the custom hostname**, because
`tls_options.versions = ["TLSv1.3"]` blocks Fly's own ACME validation. Fifteen
minutes of dev downtime and three theories later, copying crates.vip's shape
issued the certificate on the first poll, under twenty seconds.

`api.crates.vip` sets no `tls_options` at all. Neither should you. Do not set
`alpn` either: TLS-ALPN-01 needs `acme-tls/1`, and listing only `h2` and
`http/1.1` removes it.

If you want a TLS 1.3 floor, that is a separate change made **after** the
certificate is `Issued`, and it needs re-verifying that renewal still works.

### The config

Needs no code change. The loader is file, then Doppler, then environment, and
environment wins, so the address and transport come from `[env]`:

```toml
[env]
  AUTH__SERVER__ADDRESS = "0.0.0.0:8080"
  AUTH__SERVER__INSECURE = "true"

[http_service]
  internal_port = 8080
  force_https = true
  # A dev instance should sleep. This is the one deliberate divergence from
  # crates.vip, which keeps a machine resident so a cold build never waits.
  auto_stop_machines = "stop"
  auto_start_machines = true
  min_machines_running = 0
```

The prefix is the service's own: `AUTH__` for auth, `API__` for api.

`insecure` is a bad name for what this is. The app speaks plain HTTP on its
internal port and Fly terminates TLS at the edge; every client still gets TLS
via `force_https`. `api.crates.vip` runs exactly this way in production and
says so in its own `ServerConfig`.

### `http_service` carries the WebSocket. Verified, not assumed.

The reasonable worry is that these are pure WebSocket services and
`http_service` is for HTTP. A WebSocket is an HTTP/1.1 upgrade, and Fly
proxies it. Measured with a real client after the move:

    wss://auth-dev.honey.id    OPEN protocol=0init   <- Fly-terminated
    wss://api-dev.honey.id     OPEN protocol=0init   <- app-terminated
    wss://api.crates.vip       OPEN protocol=0init   <- Fly-terminated

The `0init` subprotocol negotiates, which is the half that carries the auth
token, so the handshake the fleet actually depends on works.

**This also disproves a blocker recorded earlier in this file.**
`api.honey.id-backend` builds endpoint-libs with `["ws"]` and no `ws-http1`,
and that was read as "h2 extended CONNECT only, so it cannot sit behind Fly".
It answered a plain HTTP/1.1 upgrade regardless. No code change is needed for
api to move.

### Order of operations, with no outage

The outage taken on 2026-09-08 was avoidable. `api-dev.honey.id` reached
`Issued` from the DNS records alone, with its config untouched, which means
the certificate can be ready **before** anything is deployed.

1. `fly certs add <hostname> -a <app>`. Inert while `handlers = []`.
2. Add both records. Neither is sufficient alone:

       AAAA  <sub>                  <the app's dedicated IPv6>
       CNAME _acme-challenge.<sub>  <from `fly certs setup`, a *.flydns.net name>

3. Poll `fly certs check <hostname>` until `Issued`. **Wait here.** This is
   the step that makes the rest of it free.
4. Deploy the config above with the existing image, no rebuild:

       fly deploy --config fly.dev.toml -a <app> \
         --image registry.fly.io/<app>:<tag> --ha=false

   `fly image show -a <app>` gives the tag.
5. Verify the real hostname, never the `.fly.dev` one. The `.fly.dev` name
   served 200 throughout the broken attempt, which is exactly why it is
   useless as a check: it says the plumbing works and nothing about the
   hostname the fleet uses.

       curl -o /dev/null -w '%{http_code}' https://<hostname>/
       bun wstest.ts wss://<hostname>          # expect OPEN protocol=0init

6. Swap the A record to Fly's shared IPv4 and release the dedicated one. Only
   now, and only once steps 4 and 5 pass. Prove it works first; save the $2
   second.

### Rollback, which was needed and which works

    fly deploy --config <the committed fly.dev.toml> -a <app> \
      --image registry.fly.io/<app>:<tag> --ha=false

Back to app-terminated in about thirty seconds. Expect one `000` on the first
request afterwards: the machine auto-stopped and is waking.

### What it looks like when it has worked

    fly certs list -a auth-honey-id-master-fly
      auth-dev.honey.id   Fly   Issued

    curl https://auth-dev.honey.id/            -> 200
    server: Fly/...                            <- not AuthHoneyServer/...
    openssl s_client -tls1_2 ... -> accepted   <- the edge, not the app
    openssl s_client -tls1_3 ... -> accepted

That `server:` header is the quickest way to tell which end terminated.

## One thing that still blocks the fleet

**ACME keeps running on a Fly-terminated dev instance.**
`AcmeConfig::is_enabled()` is `self.bunny_api_key.is_some()`, and that key
comes from Doppler, so the app goes on provisioning a certificate nothing
uses. Harmless but wasteful, and it burns Let's Encrypt issuance against the
hostname. The clean fix is to skip ACME when `server.insecure` is set, which
is what the flag already means. That is a code change, not a config one.

The `ws-http1` blocker recorded in an earlier draft of this file **was
wrong**, and is disproved above: `api-dev` answered a plain HTTP/1.1 upgrade
without it.

## State as of 2026-09-08

| | auth-dev | api-dev |
|---|---|---|
| Fly certificate | `Issued` | `Issued` |
| AAAA record | added | added |
| `_acme-challenge` CNAME | added | added |
| TLS terminated by | **Fly** | the app, still |
| Reachable by the test browser | **yes** | no |
| Dedicated IPv4 released | not yet | not yet |

`auth-dev` is done and verified: certificate issued, 200 over HTTPS, WebSocket
opens with the `0init` subprotocol, and the headless browser engine can now
fetch it, which it could not before.

`api-dev` has both DNS records and an issued certificate, and its config is
prepared, but the deploy has not been run. It is one command, and its
certificate is already waiting, so it should be a no-outage change.

Neither dedicated IPv4 has been released yet. Do that last, after both are
verified, by swapping the A record to Fly's shared IPv4 and running
`fly ips release`.

## Tooling

Bunny DNS is driven with `hoppy`, and the key is at `~/.config/hoppy/env`:

    set -a; . ~/.config/hoppy/env; set +a
    hoppy dns zone list
    hoppy dns record list --id <zone>
    hoppy dns record add --id <zone> --type AAAA --name <sub> --value <v> --ttl 300 --yes

`--dry-run` prints the exact request without sending it; use it first. Every
fleet zone is on Bunny **except pathscale.com**. `honey.id` is zone `648843`,
`crates.vip` is `857590`.

There is no Doppler CLI on this machine, and Fly secret values cannot be read
back, so anything living in Doppler has to be changed there.
