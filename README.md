# Catasterism Companion API

[Loco](https://loco.rs) powers this service, which polls the Catasterism explorer,
stores chain health history, and sends transition alerts. The chain stats API is
public and read-only; it has no user accounts or authentication.

## Quick Start

```sh
cargo loco start
```

```sh
$ cargo loco start
Finished dev [unoptimized + debuginfo] target(s) in 21.63s
    Running `target/debug/myapp start`

    :
    :
    :

controller/app_routes.rs:203: [Middleware] Adding log trace id

                      ▄     ▀
                                 ▀  ▄
                  ▄       ▀     ▄  ▄ ▄▀
                                    ▄ ▀▄▄
                        ▄     ▀    ▀  ▀▄▀█▄
                                          ▀█▄
▄▄▄▄▄▄▄  ▄▄▄▄▄▄▄▄▄   ▄▄▄▄▄▄▄▄▄▄▄ ▄▄▄▄▄▄▄▄▄ ▀▀█
 ██████  █████   ███ █████   ███ █████   ███ ▀█
 ██████  █████   ███ █████   ▀▀▀ █████   ███ ▄█▄
 ██████  █████   ███ █████       █████   ███ ████▄
 ██████  █████   ███ █████   ▄▄▄ █████   ███ █████
 ██████  █████   ███  ████   ███ █████   ███ ████▀
   ▀▀▀██▄ ▀▀▀▀▀▀▀▀▀▀  ▀▀▀▀▀▀▀▀▀▀  ▀▀▀▀▀▀▀▀▀▀ ██▀
       ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
                https://loco.rs

environment: development
   database: automigrate
     logger: debug
compilation: debug
      modes: server

listening on http://localhost:5150
```


## Catasterism companion API

This app is a companion service for [Catasterism](https://catasterism.xyz), an
experimental prime-constellation cryptocurrency. It polls the chain explorer,
keeps a history of what it sees, and emails alerts when chain health changes.

- `GET /api/chain/status` — health right now: latest poll, freshness, the
  explorer's ledger agreement with its node, peer connectivity.
- `GET /api/chain/snapshots` — paginated poll history, newest first.
- `GET /api/chain/blocks` — paginated mined blocks, deduplicated by hash.

Polling is a task on a schedule — the Loco pattern for recurring work:

```sh
cargo loco task chain_poll     # poll once, by hand
cargo loco scheduler           # run the configured schedule (every minute)
cargo loco scheduler --list    # inspect the schedule
```

Every poll stores a `chain_snapshots` row, success or failure, plus any new
blocks in `chain_blocks`. Alerts are edge-triggered off the transition between
the previous row and the new one, so a broken explorer mails once, not every
minute. The snapshot and any resulting `chain_alerts` rows are committed
together. If Mailgun is unavailable, pending alerts are retried by the next poll.
Delivery is at-least-once: a process crash after Mailgun accepts a message but
before the database records delivery can result in a duplicate.

| Transition | Alert |
|---|---|
| poll ok → failed | explorer unreachable |
| failed → ok | explorer reachable again |
| ledger check "ok" → anything else | ledger disagreement |
| anything else → "ok" | ledger agreement restored |
| peer true → false | explorer lost its node peer |
| false → true | peer restored |

Settings live under `settings:` in `config/<env>.yaml`:

| Key | Meaning |
|---|---|
| `explorer_url` | explorer base URL (`EXPLORER_URL`; required in production) |
| `alert_email` | where alerts are mailed (`COMPANION_ALERT_EMAIL`; required in production) |
| `stale_after_secs` | age after which the latest poll, or the explorer's own view inside it, counts as stale (default 300) |

Mailgun settings are nested under `settings.mailgun`. The API base URL defaults
to `https://api.mailgun.net`; set it to `https://api.eu.mailgun.net` for an
EU-region Mailgun domain. The sender defaults to
`Catasterism Alerts <alerts@MAILGUN_DOMAIN>` and can be overridden with
`MAILGUN_FROM`. Tests send requests only to a local mock Mailgun endpoint.

## Production

Production configuration requires these environment variables:

| Variable | Purpose |
|---|---|
| `DATABASE_URL` | PostgreSQL connection URL |
| `HOST` | Public host used in the Loco server configuration |
| `MAILGUN_API_KEY` | Mailgun private API key (never commit it) |
| `MAILGUN_DOMAIN` | Verified Mailgun sending domain |
| `MAILGUN_FROM` | Optional sender address; defaults to `alerts@MAILGUN_DOMAIN` |
| `MAILGUN_API_BASE_URL` | Optional API region endpoint; defaults to the US endpoint |
| `EXPLORER_URL` | Catasterism explorer base URL |
| `COMPANION_ALERT_EMAIL` | Alert recipient |

Set secrets through the deployment platform, not in committed config files.
For local development, `cargo loco` reads values from `.env`; `.env` is ignored
by git. `PORT` defaults to `5150`.

Run migrations once as a release step, then run one API process and exactly one
scheduler process against the same database:

```sh
cargo loco db migrate --environment production
cargo loco start --environment production
cargo loco scheduler --environment production
```

`DB_AUTO_MIGRATE` defaults to `false` in production so replicas do not race to
migrate at startup. Set it to `true` only for a deliberate single-instance
deployment. Keep the scheduler at one replica and ensure each poll finishes
within its one-minute interval; concurrent scheduler runs can duplicate poll
records and alert attempts.

## Docker Compose deployment

```sh
cp .env.example .env            # fill in; POSTGRES_PASSWORD: openssl rand -hex 24
docker compose up -d --build
```

| Service | Role |
|---|---|
| `db` | Postgres 17 on an internal network with no route to the internet |
| `migrate` | runs `db migrate` once per deploy; the app and scheduler wait for it |
| `app` | the API and stats page; never published to the host |
| `scheduler` | exactly one; polls the explorer every minute |
| `proxy` | Apache 2.4, the only published port (`127.0.0.1:8080` by default) |

Every Loco container runs as a non-root user with a read-only filesystem, all
capabilities dropped, and `no-new-privileges`. Secrets come from `.env` and are
never copied into the image.

### The proxy refuses by default

`deploy/apache/rules.conf` forwards only `/`, `/favicon.*` and
`/api/chain/{status,snapshots,blocks}` (paging parameters only, `page_size` at
most 100). Everything else gets a bare `403 Forbidden` and never reaches the app:
scanner paths (`/.env`, `/wp-login.php`, `/.git`, ...), non-GET/HEAD methods,
traversal and injection patterns, unknown `Host` headers, and clients whose
User-Agent is empty, a crawler or SEO bot, a scripting library, `curl`/`wget`, a
headless browser or a known scanner. Slow-request, header-size and URL-length
limits are set in `httpd.conf`. Apache has no built-in rate limiting; put
fail2ban or a CDN in front of it if you need that.

Because crawlers are refused, search engines will not index the site and
`robots.txt` says `Disallow: /`. To allow one, remove its name from the
`bad_client` patterns in `rules.conf`. To `curl` the site yourself, send a
browser-style `-A` header.

### HTTPS

```sh
mkdir -p deploy/certs           # fullchain.pem and privkey.pem, readable by uid 82
# .env: BIND_ADDR=0.0.0.0 HTTP_PORT=80 HTTPS_PORT=443
#       SERVER_NAME=your-domain PUBLIC_URL=https://your-domain
docker compose -f compose.yaml -f compose.tls.yaml up -d
```

Plain HTTP then answers with a 308 redirect, TLS 1.2+ only, with HSTS.

## Getting help

Check out [a quick tour](https://loco.rs/docs/tutorials/the-tour/) or [build your first app](https://loco.rs/docs/tutorials/your-first-app/).

### Host Apache in front of the stack (stats.catasterism.xyz)

`deploy/apache-host/` holds a public-facing Apache 2.4 for Debian/Ubuntu that
terminates TLS and forwards an allowlist of URLs to the compose proxy on
`127.0.0.1:8088`:

```sh
sudo a2enmod ssl proxy proxy_http headers rewrite reqtimeout socache_shmcb
sudo cp deploy/apache-host/hardening.conf /etc/apache2/conf-available/stats-hardening.conf
sudo cp deploy/apache-host/stats.catasterism.xyz.conf /etc/apache2/sites-available/
sudo mkdir -p /var/www/acme
# certificate first (webroot /var/www/acme), then:
sudo a2enconf stats-hardening && sudo a2ensite stats.catasterism.xyz
sudo apachectl configtest && sudo systemctl reload apache2
```

Set `SERVER_NAME=stats.catasterism.xyz`, `BIND_ADDR=127.0.0.1` and
`HTTP_PORT=8088` in `.env` (the compose proxy rejects any other Host header).
