# 14 · A Day in the Life (Scenarios)

These scenarios show what the concepts feel like in practice. Each one should become an
end-to-end demo and benchmark.

## Ava: music producer, laptop + home studio desktop
- **08:55:** She opens the laptop lid. The hinge sensor has already started restoring
  last night's mix session, and it's ready by the time the lid is fully open (doc 08).
- **09:10:** She double-clicks a Windows-only VST installer from a sample pack
  site. A capsule appears: Wine with a recipe from the compatibility DB, and the plugin
  bridged into the native audio graph. She didn't make a single choice.
- **11:00:** *"Update the DAW, but don't break my album project."* The update runs in a
  **multiverse fork**: the project is opened there and plugins are scanned.
  Report: *"One plugin fails. Keep the old DAW for this project and use the new one
  everywhere else?"* She taps Yes. Two versions now run side by side, with no install conflicts.
- **13:00:** Video call. The Studio profile pauses, the call session claims the headset,
  and noise suppression runs once on the NPU.
- **17:30:** *"Master all 12 tracks to WAV, MP3 and streaming loudness by tomorrow
  morning, quietly."* The outcome contract moves the job to the studio desktop
  overnight, and the laptop goes to sleep.

## Sam: office worker, company laptop
- A new laptop arrives. Sam signs in once, the fleet Manifest applies, and their
  files stream in on demand. Ready in four minutes.
- The CRM is a 2012 Windows app. IT wrapped it as **verbs** using its
  UI tree, so Sam says *"log this email as a follow-up for Acme"* and it's done.
  There's no screen-scraping script to maintain.
- Sam deletes a folder by mistake and says *"bring back the Acme folder as it was this
  morning."* It's restored in one step.

## Priya: IT lead, 800 devices
- She writes a policy change. The **SMT check** rejects it: *"This firewall rule
  removes the recovery path for devices on the guest VLAN."* She fixes it, and it goes
  out to the canary ring. Attestation confirms 800 out of 800 machines are on the new
  configuration generation within the hour.
- A user reports a crash. The **deterministic replay** shows the exact
  sequence, and a bisect across forks identifies a driver update. That update is rolled back across the fleet in
  one transaction.

## Leo: maker, Pi Zero 2 W in the shed
- He flashes the Seed image, and the Pi joins his personal mesh by QR code.
- *"Log the shed temperature and warn me if it drops below 2 °C."* The intent is
  resolved by deterministic matchers on the Pi, with no model needed.
- A harder question, *"why did it spike last Thursday?"*, is offloaded to his desktop's
  GPU through the mesh. If the desktop is off, he gets the raw graph instead.
- The power cuts out mid-write. With a read-only root and a log-structured Strata, it
  boots cleanly anyway.

## Margaret: 78, a tablet-style laptop
- She never sees an "app". She sees *"Photos of the grandkids", "Call Jenny", and
  "That letter from the council"*. These are sessions and smart queries, organised by
  people and time.
- *"Make the writing bigger everywhere"* uses malleable UI, and it applies even inside the
  old card-game program she loves.
- When a scam web page tells her to "install support software", her
  **constitution** template ("never install remote-access tools without my
  son's approval") blocks it deterministically.
