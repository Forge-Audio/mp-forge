---
title: Share a plugin
description: Hand a plugin you made to somebody else, and bring one in that somebody handed you.
---

A plugin made in MP Forge is a project: its code, its screen, its pictures and its sounds. You can save that
project as one file and hand it to another musician, who brings it into their own MP Forge and carries on from
there: plays it, changes it by asking, edits its look, and sends it to their MPC.

## Hand a plugin to somebody

1. Open the plugin.
2. Press the **⋯** button above its screen and choose **Export…**.
3. Choose where to save it. MP Forge writes one file, named after the plugin and ending in `.mpforge`.
4. Give that file to whoever you like.

What the file holds:

- the plugin's code
- its screen and everything you arranged by hand, with the artwork you added
- its pictures
- the sound files you added to it

What stays with you:

- your conversation with the AI
- the plugin's earlier versions
- anything built from it

## Bring a plugin in

1. Go to **+ New plugin**.
2. Press **IMPORT A PLUGIN** under the message box.
3. Choose **A PLUGIN FILE** for a `.mpforge` file, or **A PLUGIN'S FOLDER** for the folder of a plugin's
   project, such as one written by hand with Akai's MPC Plugin SDK. Then pick it.
4. The plugin opens as a new plugin of yours, marked NOT CHECKED YET. Run its safety check to build it. After
   that you can see its screen, play it, change it and send it.

If you already have a plugin of the same name, MP Forge asks you for another name. It never replaces a plugin
you have.

> **Only bring in plugins from people you trust.** A plugin is a program. Its safety check builds its code and
> runs it on your computer. The check reads the code first and stops what it knows to be harmful, such as
> starting other programs or using the network, but it cannot catch everything. Treat a plugin from a stranger
> the way you would treat any program from a stranger.

## What MP Forge does to keep an import safe

- **Nothing runs when a plugin is brought in.** Its files are only copied. The code is first built when you
  run the safety check, and the check reads it before building it.
- **Only a plugin's own files come in.** Code, the screen's files, drawings, artwork and sound files, each
  from its own place and each up to a size limit. Anything else in the file or the folder is left out.
- **The plugin's settings file is written afresh**, so it cannot point MP Forge's build tools at anything
  outside the plugin.
- **A finished plugin cannot be brought in.** The folder that goes on an MPC holds no code, so MP Forge
  could not check it. Ask its maker for the project.

## Two things to know

- **Two copies of one plugin are the same plugin to an MPC.** An MPC knows a plugin by a hidden four-letter
  id, not by its name. If you bring in a friend's version of a plugin you also have, and send both to your MPC,
  the MPC takes them for one plugin.
- **A plugin with a hand-made screen loses it.** MP Forge builds every screen from the plugin's layout file. A
  project whose screen files were written by hand comes in with the screen its layout file describes.
