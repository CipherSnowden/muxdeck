# MuxDeck — Transport Decision

**MuxDeck speaks Wi-Fi/LAN and nothing else.** This document exists because that is not the
obvious choice, the project's own earlier design said otherwise, and the question has now been
asked more than once. Everything needed to re-open it is here, so nobody has to reconstruct the
reasoning a fourth time.

`CLAUDE.md` constraint #1 is the enforceable rule. This file is the argument behind it.

---

## 1. What was considered

| Transport | Verdict | Why |
| --- | --- | --- |
| Wi-Fi / LAN (WSS) | **Shipped** | Fast enough, works on every target, no native tooling |
| USB — Android | Declined for v1 | Technically sound, but a bad end-user story |
| USB — iOS/iPadOS | Declined | Requires inverting the architecture; see §3 |
| Bluetooth LE | Declined | Slower than Wi-Fi at the one thing that matters |
| Bluetooth Classic (SPP) | Impossible on iOS | Blocked without MFi certification |

BLE is dismissed on numbers: a 7.5 ms minimum connection interval, real-world round trips of
20–100 ms, and periodic stalls. Wi-Fi's 3–12 ms LAN round trip beats it, so BLE would be adding a
native transport to make the product worse.

---

## 2. The prior design was not merely rejected — it did not work

The pre-rewrite specification (`F:\reference\muxdeck-legacy\MUXDECK_ARCHITECTURE.md`, §1 and §3)
made USB-C the **primary** transport and Wi-Fi the fallback:

> The primary connection medium is **USB-C direct tunneling** using Apple's native `usbmuxd`
> protocol (`iproxy`), providing an air-gapped, zero-latency physical communication channel.

Its handshake example has the iPad connecting to `127.0.0.1:8080` and reaching the desktop daemon.
**That cannot happen.** `usbmuxd` and its `iproxy` front-end forward in one direction only:

```
iproxy <local_port> <device_port>
   ^ listens on the DESKTOP        ^ connects to a listener ON THE iPHONE/iPAD
```

The desktop is always the client and the iOS device is always the server. There is no
reverse-forward mode, no third-party equivalent of `adb reverse`, and no public API that would let
an iOS app dial out through the cable to a host-side listener. The legacy design would have failed
the moment it was wired up.

This matters beyond history: it is the reason "just add USB back" is not a small task. The feature
was never working code that got deleted.

---

## 3. What iOS USB would actually cost

To make USB work on iPadOS the roles have to invert for that platform alone:

1. The **client** becomes the WebSocket server, listening on `127.0.0.1:<port>` on the iPad.
2. The **daemon** becomes the WebSocket client, reaching it through `usbmuxd`.
3. Certificate pinning inverts with it. Today the client pins the host's self-signed certificate
   (`docs/CLIENT.md` §3). In USB mode the host would have to authenticate the *device's*
   certificate, which the device would have to generate and the host would have to pin at pairing
   time — a second trust path, with its own pairing flow.
4. Session authentication inverts too. `docs/PROTOCOL.md` §3 has the client proving possession of
   its device key against a host-issued challenge. Reversed, the host proves itself to the device,
   which means a second challenge direction and a second signing domain string.
5. `libimobiledevice`/`usbmuxd` has to ship inside the Windows, macOS **and** Linux installers. On
   Windows that historically means depending on Apple Mobile Device Support, i.e. asking users to
   install iTunes.
6. iOS suspends backgrounded apps within seconds, closing listening sockets. A deck is usually
   foreground, so this is survivable — but it turns every app switch into a reconnect.

That is a milestone-sized change to the security model, not a transport plugin. Against it: a few
milliseconds saved versus a LAN round trip that is already inside the 25 ms budget
(`docs/ARCHITECTURE.md` §7).

---

## 4. Why Android USB was declined even though it works

Android is the honest case. `adb reverse` does real reverse forwarding:

```powershell
adb reverse tcp:47654 tcp:47654
```

After that, `127.0.0.1:47654` **on the phone** reaches port 47654 on the desktop. The existing
client works unmodified — same direction, same pinning, same handshake — because the engine's
certificate already carries a `127.0.0.1` SAN (`docs/ARCHITECTURE.md` §5.1). This is roughly a day
of work behind the `Transport` seam.

It is still out for v1:

- It requires the user to enable Developer Options and USB debugging, and to accept an RSA
  fingerprint prompt. That is a developer workflow, not a consumer one.
- The desktop side has to ship `adb` (~5 MB) and manage its server lifecycle, or depend on a
  system `adb` that most users do not have.
- `adb reverse` is torn down on every cable re-plug and every `adb` server restart, so it needs a
  watchdog to be reliable.
- It buys single-digit milliseconds against a transport that already meets budget.

**If USB is ever revisited, do this one and only this one.** It is the entire benefit at a
fraction of the cost, and it does not touch the security model.

---

## 5. Where the seam is

`docs/CLIENT.md` §5 keeps `Transport` as an interface with a single implementation, deliberately:

```
apps/client/lib/data/transport/
├── transport.dart        abstract Transport { connect, send, stream, close }
├── lan_transport.dart    the only implementation today
└── connection.dart       reconnect/backoff wrapper around Transport
```

A `UsbTransport` implementing the same interface is where Android USB would land. Nothing above
`Transport` — session, pairing, profile, deck UI — should need to know which one is in use. If a
transport change requires edits above that layer, the seam has been violated.

The engine needs no seam at all for the Android case: over `adb reverse` it sees an ordinary
loopback TCP connection. Note that such a connection **is** loopback, so it would satisfy the
address half of the `admin` role check — the admin token (`docs/ARCHITECTURE.md` §5.4) is what
still stops a USB-attached phone from obtaining `admin`. That token is doing real work here, which
is one more reason not to "simplify" it away.

---

## 6. The consequence to design around

Wi-Fi-only means a phone on a guest VLAN, on cellular, or on a network with AP isolation cannot
reach the host, and there is no cable to fall back to. That failure has to be **legible**:

- Discovery must distinguish "no hosts found" from "host found but not responding" from
  "fingerprint mismatch" (`docs/CLIENT.md` §6). A spinner that never resolves is a bug.
- Manual `host:port` entry must always remain available, because mDNS is the part most likely to
  be blocked by a router.

---

## 7. What would change the decision

Reopen this if — and only if — one of these is true:

- Measurement shows LAN round trip regularly exceeding the 25 ms budget on a normal network, and
  the cause is the network rather than the code.
- The project targets an environment where Wi-Fi is genuinely unavailable or prohibited.
- Apple ships a supported reverse-tunnel path for third-party apps over USB. (As of this writing,
  it has not.)

Convenience is not on that list. "It would be nice to not depend on Wi-Fi" was the original
motivation and it is what produced a design that could not work.
