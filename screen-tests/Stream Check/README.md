# Stream Check

A test of MP Forge's own stream helper on a real MPC, by Workbench Tests. It is written the way MP Forge's guide
`mp-forge/online.md` tells the AI to write a plugin that brings in sound from the internet, and it is built by
MP Forge's own safety check with online plugins switched on, so the helper in it is the one MP Forge writes.

It plays one of two stations on top of whatever passes through it.

No station is kept with it. Copy `online.example.json` to `online.json` and put in the addresses of two stations of
your own choosing. Each has to be a stream of MP3 sound whose address starts with `http://`.

| Control | What it does |
|---|---|
| Volume | How loud the station is |
| Playing | Off closes the stream |
| Station | The first or the second stream in `online.json` |

The MPC has to be on a network with a way out to the internet.
