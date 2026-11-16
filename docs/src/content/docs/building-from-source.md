---
title: Building from source
description: For developers. Fetching the code and Akai's SDK, building and starting MP Forge, and running its tests.
---

This page is for developers who want to build MP Forge themselves or work on it. To make plugins, you do
not need any of this: [Getting started](./getting-started.md) is the page for you.

## What you need

- **Git**, to fetch MP Forge and Akai's SDK.
- **Rust**, from [rustup.rs](https://rustup.rs). MP Forge is written for Rust's 2024 edition, so it needs
  Rust 1.85 or newer. A fresh install is new enough.
- **Python 3**, which runs Akai's tools. [What you need](./what-you-need.md) says where to get it.
- **What Tauri needs on your system.** MP Forge's window is built with Tauri 2. Tauri's own
  [list of prerequisites](https://tauri.app/start/prerequisites/) says what that is on each system.

On Ubuntu 24.04, MP Forge's automatic build installs these before it builds the app:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev libasound2-dev
```

## Get the code

```sh
git clone https://github.com/Forge-Audio/mp-forge.git
cd mp-forge
```

## Fetch Akai's SDK

MP Forge builds every plugin with Akai's own MPC Plugin SDK. It is not kept in MP Forge's repository. It
is fetched into `vendor/`, at the exact version MP Forge is tested against, which is written in the file
`sdk.pin`.

On macOS and Linux, run this from the `mp-forge` folder:

```sh
./scripts/check-sdk.sh
```

It fetches the SDK, then builds Akai's three example plugins and plays each through Akai's simulator, to
prove that Akai's tools work on your computer. The first run also downloads Akai's compiler, about 50 MB,
so it takes a few minutes. It ends with `All 3 examples built and passed the host simulator.`

On Windows that script has not been confirmed yet. Fetch the SDK with these three commands in Git Bash
instead. They are what MP Forge's own automatic Windows build runs, and they work on macOS and Linux too:

```sh
source sdk.pin
git clone "$SDK_REPOSITORY" vendor/mpc-plugin-builder-sdk
git -C vendor/mpc-plugin-builder-sdk checkout "$SDK_COMMIT"
```

## Build and start MP Forge

```sh
cd app
cargo run
```

The first time, this builds the whole app, which takes several minutes. After that it starts in seconds.

Built this way, MP Forge looks for Akai's SDK in the `mp-forge` folder it was built in, so keep that folder
where it is. To have it look in another folder, set `WORKBENCH_HOME` to that folder before you start it.

If Akai's build tools were not set up by `check-sdk.sh`, MP Forge fetches them the first time it opens, as in
[Getting started](./getting-started.md#2-let-mp-forge-check-your-computer).

## Run the tests

The tests build real plugins, so they need Akai's SDK and its build tools in place first. MP Forge's
automatic build sets the tools up by building one of Akai's examples before it runs any test:

```sh
cd vendor/mpc-plugin-builder-sdk
python3 tools/build.py examples/StereoDelay
```

On Windows the program is called `python`. Then, from the `mp-forge` folder:

```sh
cd engine
cargo test --locked
cd ../app
cargo test --locked
```

These run on Linux and Windows every time the code changes, and on a Mac when asked for.

## The safety check from the command line

The engine also builds a command-line program, `workbench`. From the `engine` folder, this runs the safety
check on one plugin's folder:

```sh
cargo run -- gate <plugin folder> --sdk ../vendor/mpc-plugin-builder-sdk
```

It ends with exit code 0 when the plugin is safe to send, 1 when it is not, and 2 when the check could not
be run at all.

## What is where

| Folder | What it holds |
| --- | --- |
| `app/` | The desktop app, built with Tauri 2. Its screen is plain JavaScript, in `app/ui/`. |
| `engine/` | The Rust engine: the safety checks, the AI's work, building, playing and sending. |
| `directives/` | The instructions the AI is given. |
| `screen-tests/` | Test plugins for trying out what an MPC's screen supports. |
| `scripts/` | `check-sdk.sh`, which fetches Akai's SDK and checks it. |
| `docs/` | This site. `npm install` and `npm run dev` in that folder show it on your computer. |

The code calls the app "workbench" in places. The product's name is always MP Forge.
