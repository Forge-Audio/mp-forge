---
title: Plugins that go online
description: Let a plugin bring in sound from the internet, such as a radio station. Experimental, and off until you switch it on.
---

A plugin can bring in sound from the internet while it plays, and do with it whatever you ask for: play a radio
station, run one through echoes and filters, or chop what it hears in time with your song.

**This is experimental.** It is off until you switch it on, and it may change. It has been tried on an MPC Key 37.

## Switch it on

1. Open **Settings**.
2. Find **Online plugins**, marked EXPERIMENTAL, and choose **ON**.

Then ask for what you want, and give the address of the stream if you have it:

> A radio that plays `http://stream.example.com/jazz.mp3`, with a volume knob and a filter I can sweep.

While the setting is off, a plugin that goes online is not built and cannot be sent, and one you built earlier
stays silent when you play it in MP Forge. A plugin that is already on your MPC is not changed by the setting.

## What can be brought in

- A stream of **MP3** sound whose address starts with `http://`. Many internet radio stations offer one.
- Up to 16 streams in a plugin, each with a name. The plugin can switch between them.

What cannot:

- An address that starts with `https://`, or a stream sent as AAC, Ogg or Opus.
- A playlist file (`.m3u`, `.pls`, `.m3u8`), or a web page with a player on it. The address has to be the
  stream itself.
- A computer on your own network, or an address written as numbers.
- Anything but sound coming in. A plugin cannot send anything out, fetch files or presets, or read web pages.

The MPC has to be on a network with a way out to the internet. Sound starts a second or two after a stream is
opened, and a stream that drops is tried again by itself.

On a Windows computer a stream is silent when you play the plugin in MP Forge. It is built and checked as
usual, and it plays on the MPC.

## How MP Forge keeps it narrow

- **A plugin's own code still never goes online.** The safety check blocks code that uses the network or
  starts a thread, exactly as before. A plugin only lists the addresses it wants, in a file called
  `online.json`.
- **One helper does the fetching, and it is MP Forge's own.** MP Forge writes it into the plugin afresh before
  every check, with the listed addresses built in. A changed copy is written over. The AI can neither read nor
  change it, and a plugin's own code may not reach into it.
- **Only the listed addresses can be reached.** There is no way for a plugin to hand the helper another
  address while it runs.
- **The helper asks for the stream and sends nothing else.** One plain request to each address, and what comes
  back is treated only as sound.
- **Addresses on your own network are refused**, both when the plugin is checked and again by the helper when
  it looks an address up.
- **Nothing goes online while a plugin is checked.** During the safety check and the simulator the helper
  stays off the network.
- **You can see it.** A plugin that goes online is marked **ONLINE** under its name. Rest the pointer on that
  line to see its addresses. They are shown again before you send the plugin to your MPC, and when you bring
  in a plugin somebody handed you.

## What this cannot promise

- **The stream is not encrypted.** Anyone on the networks between your MPC and the station can see which
  station is playing, and could change what arrives.
- **Sound from the internet is decoded inside the MPC's app.** The decoder is small and widely used, but a
  plugin that takes in outside data can never be as safe as one that does not. A stream that sets out to
  break it could crash the MPC's app. That is why this is off until you switch it on.
- **Whoever runs the station sees your network's address**, as any website you visit does.
- **A plugin somebody hands you can list any public address.** Look at the addresses before you run its
  safety check, and only bring in plugins from people you trust.
- **What a station plays is the station's.** Listening is one thing. Sampling it into a track you release is
  another, and the rights to that are yours to sort out.
