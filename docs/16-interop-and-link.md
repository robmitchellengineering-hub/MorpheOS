# 16 · Interop & Morph Link: Everything Talks to Everything

"No more Mac vs PC vs Linux vs Android" can't depend on everyone switching
OS. MorpheOS has to **speak everyone else's language natively**, and eventually
put a small agent on devices it doesn't run on.

## Part A: Speaking existing protocols (built into the OS)

| Need | Protocols to support | Notes |
|---|---|---|
| File sharing (LAN) | SMB 2/3 (client and server), NFS, WebDAV, SFTP | Windows and Mac shares just appear, and MorpheOS shares appear to them |
| Nearby sharing | Google Quick Share (open implementations exist), an AirDrop-compatible protocol (research: OpenDrop needs specific Wi-Fi hardware) | AirDrop support is best effort and hardware-dependent. Be honest about it |
| Screen casting | Chromecast (Cast) sender, Miracast, AirPlay receiver/sender (open implementations exist), DLNA | Casting to TVs and projectors with one tap |
| Printing/scanning | IPP Everywhere, AirPrint-compatible, eSCL scanning | Driverless printing, as on modern macOS and Linux |
| Smart home | Matter and Thread (with a Thread border router on supported hardware) | A Pi Seed profile can *be* your Matter hub |
| Phones | MTP/PTP, ADB integration for Android, KDE Connect protocol | Notifications, SMS and photos from your existing phone |
| Accounts and cloud | Microsoft 365/OneDrive, Google Drive, iCloud Drive (via web APIs), Dropbox, S3 | Mounted as Strata tiers, so they're searchable and versioned locally |
| Enterprise | Active Directory/Entra ID join, Kerberos, LDAP, SCEP/802.1X, common VPNs (WireGuard, IPsec, OpenVPN, vendor SSL VPNs via capsule) | Fleet profile requirement |
| Mail/calendar/contacts | IMAP/SMTP, JMAP, CalDAV/CardDAV, Exchange (EWS/Graph) | System-level services, available to any app through verbs |
| Peripherals | Bluetooth LE Audio, USB4/Thunderbolt, DisplayLink (vendor driver) | Through the Device Broker |

**Rule:** a protocol is integrated *once*, at the OS level, as a Strata tier,
a Device Broker device, or a verb. Every app then gets it for free.

## Part B: Morph Link (phase 3)

**Morph Link** is a small app or agent built from the *same open-source core*
(mesh, Strata sync, heal engine, intent client) for operating systems we
don't replace.

| Platform | Form | What it can do there |
|---|---|---|
| Windows 10/11 | Service and tray app | Joins the mesh, two-way file sync, clipboard, receives handoff, compute offload in both directions, **heals Windows** where it can (restore points, driver rollback, app repair), Phoenix-backup *of* the Windows machine |
| macOS | Menu-bar app with a system extension | Mesh, sync, clipboard, handoff, Time Machine-aware backup into the mesh |
| Android | App (no root needed) | Mesh, photo and file sync, notifications mirrored to MorpheOS, the phone as a camera, mic or remote, and as a **Phoenix key** |
| iOS/iPadOS | App, within Apple's sandbox limits | Mesh, file sync through the Files provider, photo import, remote control, Phoenix key |
| Linux (other distros) | Daemon | Full mesh, Strata as a FUSE mount, healing limited to what the distro allows |
| Web | Browser client | Get at your mesh files and sessions from any computer, with end-to-end encryption |

### Why Link matters even with "OS first"
- **It's a migration path.** Install Link on your current Windows PC, and your files, apps and
  settings are already in the mesh. Switching to MorpheOS is then just a Phoenix restore with
  a new OS underneath.
- **It's the phone story.** For years, most people's phones will stay iOS or stock
  Android. Link makes them full members of the mesh anyway.
- **It drives adoption.** Every Link install is a device that already "speaks MorpheOS".

## Part C: Phones running MorpheOS (phase 4)

- **Targets:** devices with unlocked bootloaders and mainline Linux support (the
  postmarketOS/GrapheneOS-class device lists), plus partner hardware.
- **Hard parts:** the modem (calls and SMS), camera pipelines, power management
  that rivals iOS and Android, carrier certification, and banking apps that require Play
  Integrity.
- **Android apps** run through the Android capsule (rung 4). Apps that need Play
  Integrity may refuse to run. That's a known and published limit.
- **Convergence:** the phone docks to a monitor and becomes the desktop. The same
  sessions continue at full size.

## Open questions
- Which of Quick Share or KDE Connect gives the best "it just works" Android path without root?
- Should Morph Link on Windows include Wine-in-reverse, running MorpheOS capsules on Windows through WSL2?
- How much Windows healing is possible without kernel drivers? (VSS, restore points, DISM/SFC, pnputil)
