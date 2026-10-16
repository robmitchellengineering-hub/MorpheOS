# 2026-10-16 · DeepSeek · trackpad pointer-acceleration speed

On the 2012 MacBook Pro the trackpad "works but not very well". The shell reads input through
libinput and never tuned pointer acceleration, so it used libinput's raw default. Set a
pointer-acceleration speed (0.2, neutral-ish) on pointer devices when they appear, keeping the
adaptive profile. Applied at both places a device is first seen, so a trackpad plugged after
boot is covered too.
