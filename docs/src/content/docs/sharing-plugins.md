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
- the words of its guide, if you made one

What stays with you:

- your conversation with the AI
- the plugin's earlier versions
- anything built from it

## Make a guide to go with it

MP Forge can write a one-page guide to a plugin: how to use it, for you to keep or to hand on with the plugin.

1. Open the plugin. It needs to have been built, so run its safety check first if it is marked NOT CHECKED YET.
2. Press the **⋯** button above its screen and choose **Guide…**.
3. The first time, press **WRITE IT**. Your AI is asked once for the guide's words, which uses a little of your
   AI credit and takes a minute or so.
4. The guide opens in your browser. Print it or save it as a PDF from there. It is also saved in the plugin's
   folder as `guide.html`.

What the guide holds:

- a picture of every page of the plugin's screen
- every control: what it does, where it starts, and which hardware knob turns it
- the presets, with a few words on each
- a few things to try
- how to load the plugin on an MPC

The facts in it come from the plugin itself, so they are always right for the plugin as it is. Only the
explaining is the AI's. Read it through before you hand it on: the AI reads the plugin's code to write it, but it
can still describe a control less well than you would.

The words are kept with the plugin and go along when you export it. Pressing **Guide…** again opens the guide
at once, with no AI. MP Forge asks the AI again only when the plugin's controls or presets have changed, or when
you tick the box to have the words written again.

The guide's words cannot be written through the ChatGPT (Codex sign-in) choice yet. Pick another model from
the menu at the top for the guide.

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

## List it on Open MPC

[Open MPC](https://open-mpc.github.io/) is the community's catalog of open plugins for the MPC. MPC owners
browse it and install from it, and it is where **REMIX A PLUGIN** in MP Forge finds its plugins. A plugin you
made can be listed there too.

The catalog lists open plugins only. Listing a plugin makes its code public, for anyone to read, change and
share, under a licence you choose.

MP Forge gets everything ready. The steps on GitHub are yours.

1. Open the plugin. For a fuller page, make its guide first: **⋯**, then **Guide…**.
2. Press **⋯** and choose **List on Open MPC…**.
3. Fill in the sheet:
   - **Your repository on GitHub**, written as `yourname/plugin-name`. It does not have to exist yet.
   - **One line for its card**: what the plugin sounds like or does. Plain letters and punctuation only.
   - **Licence**: MIT lets anyone do anything with it, as long as your name stays on it. GPL lets anyone
     change and share it, as long as what they share stays open the same way.
4. Choose where the folder goes. MP Forge runs the safety check once more, and only a plugin that passes is
   got ready.

MP Forge makes a folder named after the plugin, ending in "for Open MPC", and opens it. Nothing is sent
anywhere. In it:

| | What it is | Where it goes |
| --- | --- | --- |
| **START HERE** | The three steps, with your repository's name and the file names filled in. | Read it first. |
| **1 - put in your repository** | The plugin's code, screen and pictures, the licence, and a README made from its guide, with pictures of its screen and how to put the plugin on an MPC. | A public repository of yours on GitHub. |
| **2 - attach to your release** | The built plugin as one zip. | A release in that repository. |
| **3 - add to the catalog** | The small file that asks for the listing. | A pull request to Open MPC's catalog. |

The catalog is not your repository, so GitHub will not let you put a file straight into it. Press **Fork** on
the catalog's page to make a copy of your own, upload the file into `catalog/plugins` in that copy on a new
branch, and GitHub turns it into a pull request. START HERE has the steps one by one, with the checksum the
pull request's form asks for.

One of Open MPC's maintainers reads the pull request and merges it. The catalog then fetches your release,
checks the zip, and lists the plugin.

Your plugin does not have to wait for that. Once the release is published, anyone can download the zip from
your repository and copy it onto a drive for their MPC. The README says how.

Things to know first:

- **It has to be yours to share.** A remix of somebody else's plugin has to keep that plugin's licence and
  name its author. Sound files and pictures you added have to be yours to share as well.
- **The name on the card is your maker name.** It is public.
- **It is listed for MPC OS 3.** MP Forge's screens use parts that MPC OS 2 does not have.
- **A plugin that goes online is looked at by a person.** Open MPC's check marks plugins that use the
  network, so say in your pull request what it fetches. The repository gets the whole of MP Forge's stream
  helper with it, so a reviewer can read every line that touches the network.
- **A later version** needs a higher version number and a new release. MP Forge does not change the number
  for you yet.

Open MPC's own instructions are on its [workflow page](https://open-mpc.github.io/workflow.html#list-it-in-the-catalog).
