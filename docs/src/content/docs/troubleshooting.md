---
title: Troubleshooting
description: The messages MP Forge shows when something is in the way, and what to do about each.
---

If your trouble is not on this page, ask in the **help** channel of the
[Forge Audio Discord](https://discord.gg/EWkFTAEgJr). Say what you did, what you expected and what happened,
and whether you are on macOS, Windows or Linux.

## Starting MP Forge

### macOS will not open MP Forge, or Windows says it protected your PC

MP Forge's installers are not signed yet, so each system stops it the first time it is opened. This
happens once. [The first time you open it](./getting-started.md#the-first-time-you-open-it) has the steps
for each system.

### "This computer cannot build a plugin yet."

Something MP Forge cannot work without is missing: Python 3, Akai's build tools, or both. MP Forge fetches
these itself, so this means the fetch did not work, most often because the network was not there. The
message says what went wrong. Check the connection, open **Settings**, and press **GET WHAT IS MISSING**
under **What MP Forge needs on this computer**.

### My computer has Python, and MP Forge fetched its own anyway

Akai's build script needs two parts of Python, `venv` and `ensurepip`, that some systems leave out, and on
Windows a Python may be installed without being findable by the name `python`. When the Python on your
computer will not do, MP Forge fetches one of its own and uses that. Yours is left as it is.

### "The build tools could not be fetched" or "Fetching the build tools took too long"

Fetching Akai's compiler needs the network, once, for a download of about 50 MB. Check the connection and
press **GET WHAT IS MISSING** in Settings.

### "Python could not be downloaded"

MP Forge fetches a Python of its own when your computer has none it can use, a download of about 30 MB.
Check the connection and press **GET WHAT IS MISSING** in Settings.

## The AI

### The model list of a service will not open

A service's models are asked of the service itself, so this needs the network and, for most services, your
key or sign-in. Check both under **Settings → AI accounts**.

### "Ollama is not running on this computer."

Open Ollama, then open the list of models again.

### "No model here can use tools yet."

None of the models you have in Ollama can do MP Forge's work. Get one in Ollama that can use tools, then
open the list again.

### A model on your computer loses track of what it was asked

In Ollama's settings, give models a context length of 64000 or more.

### "Codex was not found on this computer yet." or "Codex is installed but not signed in yet."

The **ChatGPT (Codex sign-in)** connection uses Codex, OpenAI's own tool. Install it, run `codex login` in
a terminal, sign in in the browser window it opens, then press **CHECK AGAIN**.

### Claude does not answer through Claude Code

Claude Code has to be installed, up to date and signed in. MP Forge asks it each time you open the model
menu. When something is missing, the menu says so in red at the top of Claude's models, with **UPDATE
NEEDED**, **SIGN IN NEEDED** or **NOT INSTALLED** beside Anthropic, and the menu's button shows a red mark.
Settings also lists Claude Code under **What MP Forge needs on this computer**, and marks it **NOT FOUND**
when it is not installed. Or switch the menu's **Connection** to
**Claude API** and add a key, or pick a model from another service.

### "Claude Code stopped without finishing."

Claude Code started and then gave up. MP Forge passes on what Claude Code said as it did, after **It said:**.

When it says your Claude Code is older than MP Forge needs, run `claude update` in a terminal and try again.
MP Forge starts Claude Code with its own tools switched off, and an older Claude Code does not know how to
be started that way. `claude --version` should then say 2.1.259 or newer.

### "The AI is already building five plugins."

The AI works on up to five plugins at once. Build this one once one of them has finished, or open one of
them and press **STOP** there. Each plugin being built keeps an AI, a compiler and a safety check busy on
your computer, so several at once take longer each, and use your AI plan or credit faster.

## Building and checking

### The status says BLOCKED

One safety check failed. The conversation says which, and why. Ask for a fix there, in your own words:
"fix what the safety check found" is enough. [The safety checks](./safety-checks.md) explains each check.

### "The live player stopped on its own."

The plugin most likely crashed it. Run the safety check to find out why.

### A change made things worse

Press **VERSIONS** at the top of the conversation and go back to a version from before the change.

## Sending to your MPC

### "The MPC was not reached, or it said no."

The message ends with what Akai's install tool reported. Things to check:

- the MPC is not on the same network as your computer,
- the address is not the MPC's own, which it shows under its Wi-Fi settings,
- developer mode is off on the MPC,
- the MPC asks for a password, and none is set under **Settings → Your MPC**.

### "The MPC has no USB drive or SD card in it that can be written to."

MP Forge only sends to a drive that can be pulled out. Put a USB drive or an SD card in the MPC and send
again.

### "The MPC has several drives that can be pulled out"

Say which one to use under **Settings → Your MPC**, by its name or its number.

### "… is built into the MPC."

The drive named in Settings is the MPC's own storage, which MP Forge never writes to. Name a removable
drive there, or leave the drive empty.

### "Akai's tool for reaching the MPC could not be run"

Sending needs Python 3 and SSH on your computer. Check both under **Settings → What MP Forge needs on this
computer**.

### The plugin was sent but is not in the MPC's list

- The MPC's app has to be restarted before a new plugin shows. Send again with **Restart the MPC's app
  afterwards** ticked, or restart the MPC yourself.
- **Enable Enhancements** and **External Plugin Support** both have to be on, under Preferences →
  Enhancements (Beta).
- The MPC has to run MPC OS 3.10.1 or later.
- Look under **External Plugins** in the plugin browser.

## If you built MP Forge yourself

### "Akai's SDK was not found at … Run scripts/check-sdk.sh to fetch it."

Akai's SDK is not in the `vendor` folder inside your `mp-forge` folder. Fetch it as in
[Building from source](./building-from-source.md#fetch-akais-sdk). If you moved the `mp-forge` folder after
building, start MP Forge again with `cargo run` from its `app` folder.

## Something else

### Nothing in MP Forge moves

The conversation does not slide aside and the blacksmith stands still. Your computer has told MP Forge that
you want less motion. On Windows it says so whenever **Animation effects** is off under Settings,
Accessibility, Visual effects. Either switch that on, or open MP Forge's Settings and set **Animations** to
**ALWAYS**.

[Where MP Forge stands](./status.md) says what works and where it has been tested.
