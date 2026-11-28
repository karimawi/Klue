# Klue Server

Klue Server is Klue running on your own VPS, in a Docker container. It checks your lines
around the clock, sends alerts and reports through Telegram, email and webhooks, and keeps
every computer you use Klue on in step. Everything is set up from the app; the server has no
web page and no config file to edit.

## Running it

```sh
docker run -d --name klue-server --restart unless-stopped \
  -p 8443:8443 -v klue-data:/data ghcr.io/karimawi/klue-server:latest
docker logs klue-server
```

On its first start the server makes three things and keeps them in `/data`:

- a **key**, which is what the app asks for;
- a **certificate** for HTTPS, made by the server itself, so no domain or reverse proxy is
  needed;
- a **vault key**, which seals the saved passwords.

`docker logs` shows the key in a box, with the line to paste into the app. The key carries
the certificate's fingerprint, so the app checks it's talking to this server and no other,
even over a bare IP address. Anyone with the key can use the server, so it's treated like a
password: `klue-server new-key` makes a new one, and every device has to be given it again.

## One space, on the server

Once connected, the server is where the lines live. The app looks the same as before (Home,
carriers, dashboards, the Add wizard) but the server signs in and refreshes, and the app
shows what it found. Lines aren't split between "local" and "cloud": every computer shows the
same lines, and they're refreshed once, by the server, however many computers look at them.

The same key works on any number of devices at once. Settings → Server shows how many are
connected right now.

### Connecting

Settings → Server → Connect:

1. The server's address (`203.0.113.7` or `klue.example.com`, with `:8443` unless it's on
   another port) and the key. Klue checks both before going on.
2. A name for the server, shown in the title bar and messages ("Home VPS").
3. What's copied. Everything on this computer goes to the server: every line with its
   password, history, alerts, nickname, refresh interval and archived state, the Home tabs,
   the carrier order and the alert settings for all numbers. If the server already has lines
   (another computer connected first), the two are merged: a line on both keeps the newest
   reading and both histories.

Nothing is deleted from this computer. Its data stays as it was, set aside, so connecting can
be undone exactly (see below).

### The title bar

Connected, the title bar shows a cloud icon before "Synced 2 min ago". Its tooltip:

> You set up Klue Server on **Home VPS**. All refreshes happen over there and only the final
> data comes to this computer.

When the server can't be reached, the icon is crossed out and the text reads "Offline ·
showing 10:42". Its tooltip says so and offers **Refresh here**.

### When the server can't be reached

This computer keeps a full copy of what the server last sent (the mirror), so Klue opens and
shows the last data whether or not the server answers. While it's away:

- **Refresh here** signs in from this computer, as Klue did before the server. It needs the
  passwords, so the server sends them to each connected computer, sealed, and they're kept in
  that computer's credential store. (Settings → Server → "Keep passwords on this computer"
  turns this off; Refresh here then only works for lines added from this computer.)
- Changes (renaming, alerts, Home tabs, adding lines) wait for the server: they're greyed out
  with a note, so two copies never disagree.
- When the server is back, readings taken here go to it. The newest reading of a line wins
  and the history samples are merged, so the graphs have no gap. Alerts shown here are marked
  as sent, so the server doesn't send them again.

### Switching back to this computer

Settings → Server → **Switch back to this computer**, at any time:

- **Bring everything here** (the default): the server's current lines, passwords, history,
  Home tabs and settings are copied to this computer, which refreshes on its own again.
  Lines added from other computers come too.
- **Go back to how this computer was** before it connected (shown with the date), dropping
  what changed on the server since. Offered while that copy is kept.

Then, either way:

- the server's address, key and the mirror are removed from this computer;
- "Also erase everything on the server" (off by default) deletes the lines, passwords and
  Telegram, email and webhook settings from the server. It says how many other devices are
  connected, since they lose them too.

## Settings

Settings gets sections down the side. The first four are always there; the server ones
appear once a server is connected.

| Section | Holds | Kept on |
| --- | --- | --- |
| General | Theme, language, time format, launch at startup, tray | This computer |
| Alerts | Alert settings for all numbers, and which channels each kind of alert goes to | Server when connected |
| Data | Export and import | — |
| Server | Connect, or: status, name, devices, health, uptime, proxy, switch back | Server |
| Telegram | Bot token, linked chats, what it sends | Server |
| Email | SMTP, recipients, alerts, daily report | Server |
| Webhooks | Discord, Slack, Google Chat, Teams, ntfy, any URL | Server |
| Reports | Which lines, the period, the daily report's time, the language | Server |

Refresh interval and carrier order also move to the server, since the server is what
refreshes.

### Server

Not connected, the section explains what the server is and shows the `docker run` line with a
copy button, then the connect form. Connected but unreachable, it shows `docker start
klue-server` (and `docker logs klue-server` for the key) with copy buttons.

Connected, it shows:

- the name (editable), address and version, with an update notice if the server is older
  than the app;
- **Health**: OK, or what's wrong (lines failing to refresh, a channel failing to send, the
  disk filling up, the proxy unreachable);
- **Uptime** since the container started, and the time of the last refresh;
- **Devices**: how many are connected now, and the ones seen in the last 30 days with when;
- **Proxy** for carrier requests (below);
- Keep passwords on this computer; Switch back to this computer.

### Proxy

Carriers may one day refuse requests from abroad. The server can send carrier requests
through a proxy: HTTP, HTTPS or SOCKS5, with a username and password if needed, for all
carriers or only the chosen ones. **Test** reaches each chosen carrier through it and shows
how long it took. Only carrier requests use it; Telegram, email and webhooks don't.

## Alerts

The server decides when an alert is due, so it's sent once whatever the number of computers.
Each kind of alert (a bundle running low, a renewal coming up, renewal day) can go to:

- **Desktop**: every connected computer that has the app open shows it, as now;
- **Telegram**, **Email**, and each **webhook**.

## Telegram

1. Make a bot with @BotFather and paste its token. Klue checks it and shows the bot's name.
2. Klue shows a link (`t.me/<bot>?start=<code>`) and a QR code. Opening it and pressing Start
   links that chat. Only linked chats get answers; anyone else is told the bot is private.
   More chats can be linked the same way, and removed from Settings.

The bot sends alerts as set up, and the daily report if it's turned on. It also has a menu:

- **Home**: the first Home tab as an image of its cards, the same cards as the app's Home,
  with a short summary under it, and buttons for the other tabs.
- **WE**, **Vodafone**, **Orange**, **e&** (the carriers with lines): asks which line, with a
  button per line, then shows that line: an image of its numbers and bundles, the renewal
  date, the balance, and buttons for **Usage graph**, **Refresh now** and **Back**.
- **Full report**: the PDF report, as a file.

Commands: `/home`, `/report`, `/refresh`, `/help`. Messages use the server's language and
Telegram's formatting (bold figures, bars drawn with ▰▱, links), so they read well in the
chat list and the notification as well as in full.

If WE asks for a captcha the server can't read, it's sent to the linked chats ("Reply with the
code") and to any open app.

## Email

Email is for alerts and the daily report only. SMTP host, port, security (TLS, STARTTLS or
none), username, password, sender and recipients, with a **Send a test** button.

The daily report goes at the chosen time in the server's time zone (taken from the computer
that connected it, and editable). Its body is a table of every included line: what's left of
the main bundle, the share left, when it renews, and the balance. The PDF report is attached.
Plain text is sent alongside the HTML for clients that don't show HTML.

## Webhooks

Any number, each with a name, a kind, its URL and what it gets (alerts, the daily report):

| Kind | Sends |
| --- | --- |
| Discord | Embeds in the carrier's colour, posted as **Klue** with Klue's picture, the footer "Sent by Klue"; the daily report attaches the PDF |
| Slack | Block Kit sections with fields |
| Google Chat | A card with Klue's logo in the header |
| Microsoft Teams | An Adaptive Card (Workflows webhook) |
| ntfy | Title, tags and a click-through to the line; the daily report attaches the PDF |
| Any URL | A documented JSON body, signed with HMAC-SHA256 in `X-Klue-Signature` if a secret is set |

## Reports

The PDF report has a summary page (every included line, what's left, renewals, balances)
and a page per line: its bundles, balance and renewal, the graph of what was left over the
period, and usage per day, with the average and the day it runs out at that rate. It's in
the server's language, Arabic laid out right to left.

Settings → Reports picks the lines (all by default, and new lines join unless the user picked
a list), the period (this billing cycle, 7 days or 30 days) and the daily report's time.
**Save a report** makes one now and saves it on this computer.

## Branding

Everything the server sends says where it came from, with "Klue" linking to
<https://klue.wkar.im>:

- messages (Telegram, email, webhooks): **Sent by Klue**;
- reports and images: **Generated by Klue**, with the date and the server's name.

## Security

- The key is a long random secret; the app sends it over HTTPS to a certificate it pins.
- Passwords on the server are sealed with XChaCha20-Poly1305 under the vault key. That
  protects copies and backups of the data file, not someone who controls the VPS: the server
  has to be able to read the passwords to sign in by itself.
- Telegram, SMTP and webhook secrets are kept in the same vault.
- The server only makes requests to carriers, and to Telegram, mail servers and webhooks the
  user set up. It never contacts klue.wkar.im or anything else.

## How it's built

All in this repository, one Cargo workspace, one version number:

| Crate | Holds |
| --- | --- |
| `klue-core` | Carriers, the data model, history, alerts, export files |
| `klue-engine` | Lines, passwords and refreshing: what `Shared` in the app did, for the app and the server alike. Changes to the data are `Op`s, applied the same way on both |
| `klue-report` | The images and the PDF report, drawn as SVG |
| `klue-server` | The server: HTTPS API, sync, channels, scheduler; and its Dockerfile |
| `klue-app` | The app, with the local engine or the server behind the same `Snapshot` |

### Sync

The app reads a `Snapshot` and redraws when a revision counter moves, local or not. Connected:

- `GET /v1/events` is a server-sent event stream: new revisions, lines refreshing, alerts for
  the desktop, captchas to solve.
- On a new revision, `GET /v1/sync?since=<rev>` returns what changed: the lines changed since
  then without their history, the history samples recorded since, removed lines, and the Home
  tabs and settings if they changed. The first sync, or one after a long gap, sends
  everything. History samples are merged by time, so sending one twice does no harm.
- Changes go up as `POST /v1/ops`; the app applies them to the mirror at once, and the next
  revision confirms them.

Other calls: verify and add a line, refresh, other lines on an account, import and export
(used to copy everything up and down), readings taken offline, server settings, test
messages, a report, health, and erase.
