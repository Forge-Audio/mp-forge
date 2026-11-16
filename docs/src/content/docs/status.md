---
title: What is and is not finished
description: Where MP Forge stands today, said plainly.
---

MP Forge is an early build. This page says what works, what is still being tested, and what is not there
yet.

It is free and open source, under the GNU General Public License, version 3 or later.

## What works

- Describing a plugin and having an AI build it, as an effect or an instrument.
- The twelve safety checks, run on every plugin before it can be sent.
- Seeing the plugin's screen as it will look on the MPC, and playing the real plugin on your computer.
- Asking for changes, going back to an earlier version, and editing the look by hand.
- Sending a plugin over the network to a USB drive or SD card in an MPC.

All of this is written and tested by hand on a Mac.

## Still being tested

- **The installers.** There are installers for macOS, Windows and Linux, and the
  [download page](/mp-forge/download/) fills when the first release is out. They are not signed yet, so
  macOS and Windows each warn the first time MP Forge is opened.
  [Getting started](./getting-started.md#the-first-time-you-open-it) says what to do.
- **Windows and Linux.** MP Forge is made to work on both. So far they are tested by automated tests, which
  build the app and run on both every time the code changes. They have not yet been tested by hand, so
  expect rough edges there.

## Not there yet

- **Taking plugins off an MPC.** MP Forge sends plugins. Removing one is done on the drive itself.

## Who makes it

MP Forge is made by Forge Audio. It is an independent tool for making plugins for Akai MPC hardware. It is
not affiliated with, sponsored by or endorsed by Akai Professional or inMusic Brands. MPC and Akai are
trademarks of their owners.

Developers who want to build MP Forge themselves will find how in
[Building from source](./building-from-source.md).
