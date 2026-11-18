# Sound from the internet in a plugin

A plugin can bring sound in from the internet while it plays: a radio station, or any other stream of MP3 sound.
What the plugin does with that sound is up to you. It can play it as a radio, run it through filters and echoes,
keep the last few seconds and chop them in time with the song, or use it to shake another sound.

This is experimental, and the musician has to have switched on "Online plugins" in MP Forge's settings. It is
the one way a plugin may use the network. The rule that a plugin's own code never uses the network, and never
starts a thread, stands exactly as it was.

## How it works

- The plugin lists the addresses it wants in `online.json`, beside `ui.json`.
- The plugin's code reads the sound through `mpforge::AudioStream`, from `mp_forge_stream.h`.
- MP Forge writes `mp_forge_stream.h`, `mp_forge_stream.cpp` and the folder `src/mp-forge-online/` into the
  plugin itself, before every check, with the listed addresses built in. You cannot read or write them, and you
  never need to: everything you can use is on this page. Never write a file of your own under those names, and
  never include anything from `mp-forge-online/`. The check blocks both.
- A stream can only be opened by its place in `online.json`. There is no way to hand it an address at run time,
  so never try to build one from a control, a preset or a file.

## online.json

```json
{ "streams": [
    { "name": "Jazz", "address": "http://stream.example.com/jazz.mp3" },
    { "name": "News", "address": "http://radio.example.org:8000/news" }
] }
```

- Up to 16 streams. Each has a `name` of up to 32 letters, digits and spaces, and no two share a name.
- An address starts with `http://`. An `https://` address cannot be read. Many stations offer both, so look for
  the plain one.
- An address names a computer on the internet, such as `stream.example.com`. A number such as `192.168.1.5`,
  `localhost`, or a computer on the musician's own network is refused.
- The port is 80, which is the usual one and need not be written, or 1024 and above.
- The address has to be the stream of sound itself, sent as MP3. A playlist file (`.m3u`, `.pls`, `.m3u8`) is
  not a stream, and neither is a web page with a player on it. AAC, Ogg and Opus streams cannot be read.
- The helper asks an address once and does not follow it anywhere else, so an address that only points on to
  another one gives `kRefused`. List the address it points to.

## Which addresses to list

- List only streams the musician asked for. Never add one they did not ask for, and never list an address for
  any purpose but bringing in sound.
- Take an address the musician gave you as it stands. When they name a station without an address and web
  lookups are switched on, look for the station's own published MP3 stream. When you cannot find one, say so
  and ask for the address. Never make one up.
- A stream is for listening to as it arrives. Do not write its sound to a file.

## Reading the sound

```cpp
#include "mp_forge_stream.h"

class Radio : public mpc::Plugin
{
public:
    void prepare(double sampleRate, int maxBlockSize) override
    {
        stream_.start(sampleRate);
        fromTheStreamLeft_.assign((size_t)maxBlockSize, 0.0f);
        fromTheStreamRight_.assign((size_t)maxBlockSize, 0.0f);
    }

    void process(mpc::ProcessContext& ctx) override
    {
        stream_.open(streamWanted());
        stream_.read(fromTheStreamLeft_.data(), fromTheStreamRight_.data(), ctx.numFrames);
        // fromTheStreamLeft_ and fromTheStreamRight_ now hold ctx.numFrames of sound, or of silence.
    }

private:
    // The stream the Station control stands on, or none at all while the plugin is switched off.
    int streamWanted() const { return isOn() ? stationChosen() : mpforge::AudioStream::kNoStream; }

    mpforge::AudioStream stream_;
    std::vector<float>   fromTheStreamLeft_, fromTheStreamRight_;
};
```

Everything a stream offers:

| Call | What it does |
|---|---|
| `start(sampleRate)` | Call it in `prepare()`. Nothing is fetched yet. |
| `open(stream)` | Says which stream of `online.json` to bring in, counting from 0. `kNoStream` closes it. It only leaves a number, so call it from `process()` as often as you like. |
| `opened()` | The stream last asked for. |
| `read(left, right, numFrames)` | Fills both with the next sound, and with silence where there is none. Returns how many frames of sound it gave. `right` may be null. It never waits, so it belongs in `process()`. |
| `state()` | `kClosed`, `kOpening`, `kPlaying`, `kNotFound`, `kNotReached`, `kRefused`, `kLost` or `kNotOnThisComputer`. |
| `waiting()` | How full the store of sound waiting to be read is, from 0 to 1. |
| `AudioStream::streamCount()` | How many streams `online.json` lists. |
| `AudioStream::streamName(stream)` | The name of one of them. |

- `read()` gives the sound at the plugin's own sample rate, in stereo, between -1 and 1. A stream sent in mono
  comes out the same on both sides.
- Call `read()` once in every `process()` while a stream is open, for the whole block, even when its sound is
  turned down. A stream that is not read fills up, falls behind what is being sent and is then cut off.
- Sound starts about a second and a half after `open()`, because the stream fills up first so that a short
  hitch on the network is not heard. A stream that stops is tried again by itself every few seconds.
- A stream cannot be paused, rewound or made to run faster. It arrives at the speed it is played. To loop or
  chop it, keep what you read in a buffer you allocated in `prepare()`.
- Keep one `AudioStream` in a plugin. Each one keeps a thread of its own.
- Sound from the internet can be as loud as full scale. Put it through the plugin's volume, and fade it in over a
  few milliseconds when it starts and when the stream changes, so that it does not click.

## Choosing and showing a stream

- Give the musician a control that chooses the stream when there is more than one, with one position for each
  and the streams' names on it, in the order of `online.json`. Give them a way to switch it off.
- A new plugin should start switched off, or on its first stream, as the musician asked. Say which in the README.
- To show what the stream is doing, such as a lamp that is dark when closed, blinks while opening and glows
  while playing, keep a hidden control the plugin sets itself and have a picture follow it, the way
  `mp-forge/pictures.md` shows for a picture of what the plugin is doing. Set it from `state()` only when the
  state changes.

## What the check hears

While MP Forge checks and builds a plugin, its stream stays off the network: `state()` stays `kClosed` and
`read()` gives silence. So:

- Never wait for a stream, and never make anything else in the plugin depend on one having arrived. With
  silence from the stream the plugin has to behave like any other.
- On a Windows computer a stream gives silence and its state is `kNotOnThisComputer`. It plays on the MPC.

## When the check says online plugins are switched off

The check then blocks the plugin before it is built. Do not work round it and do not take the stream out. Tell
the musician, in one sentence, that a plugin which brings in sound from the internet is experimental and that
they switch it on under "Online plugins" in MP Forge's settings, and stop there.

When a musician asks for something else that needs the internet, such as fetching presets, sending anything
out, or reading a web page from the plugin, say plainly that a plugin cannot do it. Sound coming in through a
stream is the only thing there is.

## What to tell the musician

In what you say when the work is done, name each stream the plugin brings in. In the README, list every stream
with its address, and say that the plugin needs the MPC to be on a network with a way out to the internet.
