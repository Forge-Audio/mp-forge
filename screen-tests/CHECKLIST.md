# Screen test cards: what to look at on the MPC

Ten small effect plugins. Nine do nothing to the sound, and Radio Check plays a station on top of it. Their screens try things out so we can see what an
MPC plugin screen really supports before building the edit mode.

- **Test Card** holds the checks the edit mode depends on.
- **Draw Path, Draw Plot, Draw Knobs, Draw Guess, Draw Extras** each hold one kind of less tested widget.
  They are kept apart because one entry the MPC dislikes blanks a card's whole screen.
- **Draw Live** tests whether pictures can move by themselves, driven by the plugin.
- **Draw Fast** tests how often a second a picture can be changed before more stops looking smoother.
- **Net Check** tests whether a plugin can keep a thread of its own and reach the internet.
- **Radio Check** tests whether a plugin can play an internet radio station.

Every item has a number and a caption drawn into the page. A photo of each page is the easiest report. For a
Draw card, "blank" or "shows its page" is already an answer.

## What we know so far (MPC Key 37, MPC version 3.10.1.12)

| Question | Answer | Seen where |
|---|---|---|
| Can controls and pictures go anywhere on the page? | Yes | PLACE |
| Does a full-page background picture work? | Yes, all 1280 x 628 pixels, edge to edge | PLACE |
| Is the right edge covered by a knob sidebar? | **Only while a hardware knob is being turned.** Then a panel about 270 pixels wide covers the page from roughly 1010 across to the right edge, listing the four knobs' controls. At rest nothing covers the page | PLACE, item 5; MORE (photo of the panel) |
| What name does that knob panel show? | The plugin's own parameter name, as "Plugin 001 (Knob 5)", even for a control whose link was renamed to SPEED. A screen file cannot change it | MORE, item 7 |
| Does a control still work when its link is renamed? | Yes. The renamed knob was turned from 50 to 31 | MORE, item 7 |
| Does renaming a control through its link change the name drawn on the page? | **Yes.** The knob showed SPEED under it and still followed its hardware knob (45, then 43, then 31). This is how the edit mode renames | MORE, item 7 (video) |
| Does swapping a control's name for fixed words work too? | Yes. It also showed SPEED and followed its hardware knob (38, then 57, then 52) | MORE, item 8 (video) |
| Do the four hardware knobs follow the group that was touched? | Yes, they "work perfectly" (reported, not filmed) | GROUPS |
| Do see-through colours and see-through pictures work? | Yes | PLACE, items 8 and 9 |
| Do baked-in shadows and colour fades survive? | Yes | PLACE, item 6 |
| Does the MPC resize a knob picture to its space? | **No.** It draws it at its own size from the top-left corner and cuts off what does not fit | PLACE, items 1, 3, 4 |
| Does the MPC stretch a picture to its space? | **No.** Same: own size, cut off. Akai's reference says otherwise | PLACE, item 7 |
| Does fixed text written the reference's way show? | **No.** None of the captions appeared | PLACE |
| Do captions drawn into the background picture show? | Yes, all nine on PLACE are readable | PLACE (second round) |
| Do six pages fit along the bottom without a "more" button? | Yes | Test Card |
| Does fixed text show when it is tied to a real parameter? | **Yes.** "Hello 1", "Hello 3" and "Hello 4" all showed: tied through the entry's own mapping, tied directly, or placed inside a component. Only the one tied to nothing ("Hello 2") was missing | TEXT, rows 1 to 4 |
| Can a parameter's name be shown, and replaced with other words? | Yes. "Knob 2" showed, and so did "Renamed" in its place, with no change to the plugin | TEXT, rows 5 and 6 |
| Do text sizes work? | Yes. Size 20 is small and size 40 about twice as tall | TEXT, rows 7 and 8 |
| Can live text be coloured and aligned? | Yes. The value showed in orange at the right end of its panel | TEXT, row 9 |
| Does bold work? | Yes. Visibly thicker letters than the regular value at the same size | TEXT, row 10 |
| Does a value show its unit? | No. The value showed as "50", not "50 %" | TEXT and FONTS |
| Which fonts can be used? | Three: Titillium Web, Roboto and Nexa each looked different. Nexa works here although the open-mpc notes say it does not | FONTS, rows 1 to 3 |
| Can a plugin ship its own font? | **No.** The shipped font fell back to Titillium Web under all three ways of naming it | FONTS, rows 4 to 6 |
| Can a changing value be shown in a font the MPC does not have? | **Yes**, as a strip of pictures drawn on the computer, one per reading. A 0 to 100 number in JetBrains Mono followed the knob and matched the MPC's own number in every video frame where both were readable (16 frames, values 24 to 42, and later at 61, 68 and 70). The rest of the range was not filmed | MORE, items 3 and 4 |
| Can fixed words be in any font? | Yes, as a picture: "Drawn on the computer" showed in JetBrains Mono | MORE, item 5 |
| Can other fonts already on the MPC be used? | Not the two tried (DejaVu Sans Mono, Courier): both fell back to Titillium Web | FONTS, rows 7 and 8 |
| Can the focus box be any shape and place? | Yes for the one seen: group 1's box was drawn exactly where it was asked for (30 to 650 across, 20 to 150 down). The MPC adds a small "Q" badge in the box's top-left corner, which covers about 40 x 30 pixels of whatever is there | GROUPS |
| Can the focus box be hidden? | Yes. With no boxes given, no box is drawn at all | NO BOX |
| Do JPEG pictures work? | Yes | MORE, item 1 |
| Can things be placed as a share of the page? | Yes. The green block sat at 640 to 960 across and 314 to 471 down, as asked | MORE, item 2 |
| Does one bad entry blank a whole screen? | Yes. The first Draw card showed no pages | first Test Card Draw |
| Do settings the MPC does not know blank a screen? | No. They are ignored | Draw Guess |
| Can a plain picture be rotated or faded by a setting? | No. All three arrows looked the same | Draw Guess |
| Is anything tied to no parameter (`Invalid`) drawn? | **No.** Text and shapes tied to nothing did not appear; the one shape tied to a knob did | PLACE (first round), Draw Path |
| Can the MPC draw a shape without a picture? | Yes, as an outline only, and it grows with the knob it is tied to (half size at 50) | Draw Path, item 6 |
| Does the picture knob with a turning pointer draw? | Yes. At twice the size the pointer doubled but the round body came out as a plain square | Draw Knobs, items 1 and 2 |
| Does the MPC's own knob draw without a picture? | Yes | Draw Knobs, item 3 |
| Does the draggable number show? | No number appeared | Draw Knobs, item 4 |
| Does the plot widget draw a line or a colour fade? | Not in a still photo: only its background panel showed. Not yet seen while a knob is turned | Draw Plot |
| Do the halo, lamp, touch pad and meter widgets load? | At least one of them is refused: the card was blank | Draw Extras |
| Can a plugin move a picture by itself? | **Yes.** At 15 pictures a second the bar, the sliding dot, the 24-column wave and the counting number all moved, with nothing playing and nothing touched | Draw Live, items 2, 4, 6, 7 |
| Do the workbench's own picture strips follow the plugin too? | Yes. Both ways of drawing a strip moved together | Draw Live, items 2 and 3 |
| Can a picture loop by itself with no help from the plugin? | Yes. The spoke kept turning | Draw Live, item 8 |
| Can a bar follow the real sound on the track? | Yes. With something playing, the level bar showed the sound level (reported, not filmed) | Draw Live, item 5 |
| Does 30 pictures a second cost anything you can hear or see? | No. Smooth, with no crackles (reported, by ear and eye). The MPC's Resources screen read 2% CPU across 8 cores, busiest core 8%, memory 2.3 of 3.8 GB, about 47 °C | Draw Live, item 1 |
| Does the MPC follow hardware knobs that were given other controls? | Yes. A plugin whose knobs were reassigned in MP Forge worked on the MPC (reported) | a plugin sent from MP Forge |
| Does the MPC show more than 30 pictures a second? | **No. 30 is the cap.** Rows changed 45, 60 and up to 240 times a second looked no smoother than the row changed 30 times a second (reported) | Draw Fast |
| Can a plugin keep a thread of its own? | **Yes.** The card's thread counted up once a second (33 after about half a minute) | Net Check, row 6 (photo) |
| Can a plugin reach the internet? | **Yes.** From its own thread it asked a public name server for example.com's address, connected and was answered with 200, four tries out of four, the last in 29 ms. The MPC was on the home network | Net Check, rows 1 to 5 (photo) |
| Can a plugin play an internet radio station? | **Yes, and it sounds good**, At rest the MPC asked the plugin for 44,160 frames a second in 345 calls of 128, the same as a computer does (photo). The first card skipped, then went jumbled and silent by turns: that was the card's own fault, not the MPC's. It gave the decoder the very end of each piece that arrived, where a cut-off piece of sound is thrown away. Fed a saved sample in pieces of 1448 bytes, it got 3 seconds of sound out of 22. Keeping the last 4096 bytes back until more arrives gets all of it, at any piece size | Radio Check, by ear and rows 7 to 10 |
| How much memory does a session of loading and restarting leave behind? | Memory read 2.3 of 3.8 GB after a morning of installs and app restarts, and 1.3 GB after a full reboot. Not a clean comparison: the cards were loaded in the first reading and may not have been in the second | MPC Resources screen |
| Does the MPC keep running an effect when nothing is playing? | Yes, so displays keep moving while the song is stopped | Draw Live |

The yellow line around the PLACE and MORE pages is part of the test ruler picture, not something the MPC adds.

Still to see: what the pop-up that appears when a renamed knob is touched calls it. On NO BOX, whether the
hardware knobs follow the touched group as they do on GROUPS. Also Draw Plot, Draw Knobs and Draw Path while Knob 2
is being turned.
Seen on the Key 37 from the editor's steps 2 to 4, with the Look Test plugin (photo, 2026-11-14): the page
background the editor made (fade, see-through panel with its edge line, words in a computer font, see-through
ellipse) sits behind everything; a knob redrawn at 180 px with 89 frames and one at 120 px draw sharp and show
their settings; a control shown under another name (LOUDNESS) with its name and value centred underneath in
Roboto Bold and Titillium Web; a control with its name beside it in Nexa and its value hidden; a fader redrawn
at 420 x 44 with a bold, right-aligned value; and the focus box with its "Q" badge drawn round the touched
control's own new bounds.
**Not drawn: the heading.** Akai's generator writes a heading as fixed words tied to `Data` with nothing
mapped to it, and the MPC draws no words tied to nothing (the same as "Hello 2" on the TEXT page). The engine now
ties every heading to `Parameter 0` when it finishes the built screen. **Seen working (second photo, 2026-11-14):**
RESTYLED then showed on STYLED in Roboto Bold, in its colour, right-aligned in its own box, and MPC TEXT showed
on PLAIN in the generator's own lettering, in its slot (third photo). Still not known: whether the captions on a choice switch, which the generator writes the same
way, are drawn. No plugin with a choice switch has been looked at on the MPC.
On Draw Live: the Resources reading was not compared with the card removed, so the card's own share of that 2% is not known.

How other people's MPC plugins show moving things (from the open-mpc project's own notes, not tested here): a
picture strip tied to a parameter that the plugin itself keeps changing. The MPC redraws the strip whenever the
plugin reports the change. Akai's framework has the call for this (`notifyHostParameterChanged`).

What this means for the edit mode: resizing a knob or a picture has to be done on the computer, by redrawing
it at the new size before it goes to the MPC. The MPC will not do it.

Words have two routes. Text the MPC draws itself is limited to three fonts (Titillium Web, Roboto, Nexa), in any
size, colour, weight and alignment, and it must be tied to a parameter. Any other font has to be drawn into the
page background on the computer. Renaming a control needs no change to the plugin.

## Putting the cards on the MPC

1. Save any open project on the MPC. The MPC app has to restart to see new plugins.
2. Install or refresh all ten (put the MPC's address in place of `ADDRESS`):

   ```bash
   python3 vendor/mpc-plugin-builder-sdk/tools/mpc_remote.py ADDRESS update "screen-tests/Test Card" "screen-tests/Draw Path" "screen-tests/Draw Plot" "screen-tests/Draw Knobs" "screen-tests/Draw Guess" "screen-tests/Draw Extras" "screen-tests/Draw Live" "screen-tests/Draw Fast" "screen-tests/Net Check" "screen-tests/Radio Check"
   ```

3. Restart the MPC app:

   ```bash
   python3 vendor/mpc-plugin-builder-sdk/tools/mpc_remote.py ADDRESS restart
   ```

4. On the MPC, add each card as an insert effect on a track and open its screen.

## Test Card

### PLACE

Already answered above. The captions should now be readable, because they are part of the background picture.

### TEXT

Ten ways of putting words on the screen. Each caption on the left says what should appear in the grey panel
on its right. The panel marks the space the words are allowed to use.

| Rows | What to look for | What it tells us |
|---|---|---|
| 1, 2, 3, 4 | Which of "Hello 1" to "Hello 4" appear in their panels? | Which way of writing fixed text the MPC accepts |
| 5 | "Knob 2" | A parameter's name can be shown |
| 6 | "Renamed" instead of "Knob 2" | A label can be renamed without changing the plugin |
| 7, 8 | A value such as "50 %", small and then large | How big text sizes 20 and 40 really are |
| 9 | The value in orange, pushed to the right end of its panel | Live text can be coloured and aligned |
| 10 | The value in white at the left. Thicker letters than row 9? | Bold works |

### FONTS

Eight rows. Each row has three samples in that row's font: the words "Illicit WAVE 0123", then "Knob 2", then
a value such as "50 %". The first sample may be missing everywhere (that depends on rows 1 to 4 of TEXT); the
other two are enough to judge by.

| Rows | What to look for | What it tells us |
|---|---|---|
| 1, 2, 3 | Three different-looking fonts | The three built-in fonts |
| 4, 5, 6 | Typewriter-style letters, and a zero with a dot or slash in it | A font file shipped inside the plugin can be used |
| 7, 8 | Different from row 1? | Other fonts already on the MPC can be used |

If rows 4 to 8 all look like row 1, custom fonts are not possible.

### GROUPS

Sixteen small knobs in four groups. Group 1 is a tidy row, group 2 a square, group 3 a column, group 4 is
scattered. Touch one knob in each group.

| What to look for | What it tells us |
|---|---|
| Does a box light up around the group you touched? Does it match the group's shape (wide, square, tall, large)? | The focus box can be any shape and place we choose |
| Do the MPC's four hardware knobs now turn that group's knobs? | Groups of four still work when placed freely |
| How does the box look around the scattered group 4? | Whether scattered controls are acceptable |

### NO BOX

The same sixteen knobs with the box switched off.

| What to look for | What it tells us |
|---|---|
| Touch a knob in each group. Is there no box at all? | The focus box can be hidden |
| Do the hardware knobs still follow the group you touched? | Hiding the box does not break the knobs |

### MORE

| Item | What to look for | What it tells us |
|---|---|---|
| 1 | The round orange picture on a grey square | JPEG pictures work |
| 2 | A green block from 640 to 960 across and 314 to 471 down on the ruler | Things can be placed as a share of the page |
| 3 | A number in typewriter-style letters, with a dot in its zero. Turn the small knob: does it count along, and does it always match item 4? | A changing value can be shown in a font the MPC does not have, as a strip of pictures drawn on the computer |
| 4 | The same value in the MPC's own font | The comparison for item 3. If the two ever differ by one, note at which numbers |
| 5 | "Drawn on the computer" in orange typewriter-style letters | Fixed words can be in any font |
| 6 | A knob with "Knob 4" under it | The comparison for items 7 and 8 |
| 7 | A knob renamed through its link. Does the word under it say SPEED or Knob 5? | Whether renaming a whole control this way changes the name drawn on the page |
| 8 | A knob whose name was swapped for the fixed words SPEED. Does it say SPEED, and does it still turn? | The other way to rename, if item 7 still says Knob 5 |

Items 6 to 8 sit in the middle of the page, clear of the panel that slides in from the right while a hardware knob
is turned. For each, also touch it and note what its pop-up calls it.

## The Draw cards

For each: does the card show its page, or is it blank? If it shows, turn the small knob marked "turn Knob 2"
and watch the numbered items.

### Draw Path

| Item | What to look for | What it tells us |
|---|---|---|
| 1 | A grey square. Outline or filled? Does it fill its space or sit small in the corner? | The MPC can draw shapes without a picture |
| 2, 3 | A red triangle, then the same one much fainter | Shapes can be see-through |
| 4 | A blue curved line | Curves work |
| 5 | A triangle with drawing turned off. Anything there? | What that setting does |
| 6 | A yellow triangle. Does it change when you turn Knob 2? | Shapes can follow a knob |

### Draw Plot

| Item | What to look for | What it tells us |
|---|---|---|
| 1 | A red line that moves with Knob 2 | The widget works at all |
| 2 | Is the area under the line a fade from blue to pink, top to bottom? | The MPC can draw a colour fade by itself |
| 3 | The same fade running left to right, and a line fading yellow to red | Fades in both directions |

### Draw Knobs

| Item | What to look for | What it tells us |
|---|---|---|
| 1 | A grey disc with an orange pointer. Does the pointer turn with Knob 2? | The MPC can rotate a picture |
| 2 | The same at twice the size. Bigger, and sharp or blurry? | Rotated pictures can be scaled |
| 3 | The MPC's own standard knob | A knob that needs no picture |
| 4 | A number. Can you drag it up and down to change it? | A value-only control |

### Draw Guess

| Item | What to look for | What it tells us |
|---|---|---|
| Whole card | Does it show at all? | Whether settings the MPC does not know blank a screen |
| 1 | An orange arrow pointing straight up | The comparison for 2 and 3 |
| 2 | Is this arrow leaning to one side? | A guessed "rotation" setting does something. Most likely not |
| 3 | Is this arrow paler than item 1? | A guessed "opacity" setting does something. Most likely not |

### Draw Extras

Three pages: HALO, XY and METER.

| Page | What to look for | What it tells us |
|---|---|---|
| HALO, items 1 to 3 | A blue arc and two blue bars that grow with Knob 2 | Value rings and bars drawn by the MPC |
| HALO, items 4 and 5 | Two lamps. Does either light when Knob 2 goes past halfway? | Lamps that follow a value |
| XY | An orange dot in a grey panel. Can you drag it? Do Knob 2 and Knob 3 change? | A two-way touch pad |
| METER | A tall bar and a wide bar. Green follows Knob 2, yellow follows Knob 3 | Level meters that follow a value |

### Draw Live

One page, LIVE. Put the card on a track that has sound playing through it, and do not touch anything at first:
the point is what moves by itself. A short video is better than a photo here.

| Item | What to look for | What it tells us |
|---|---|---|
| 2 | A green bar that rises and falls about every four seconds | A plugin can move a picture by itself (the way other plugins do meters) |
| 3 | The same bar, drawn the way the workbench draws its pictures today | Whether the workbench's own picture strips follow the plugin too |
| 4 | A number counting up to 100 and back | Text can change by itself |
| 5 | A bar that jumps with the sound on the track | A real level meter is possible |
| 6 | An orange dot sliding left and right | A moving grain or playhead is possible |
| 7 | 24 blue marks forming a wave that travels sideways | A waveform or curve is possible |
| 8 | A spoke turning in a ring, without stopping | A picture can loop by itself with no help from the plugin |
| 1 | Turn the small knob (1 to 30 pictures a second). Does the movement get smoother? Does the sound crackle or the MPC slow down at 30? | How fast the screen can be driven, and what it costs |

**This card fails one of Akai's simulator checks, on purpose.** The simulator saves the plugin's settings, loads
them back and expects every value to return. Draw Live keeps changing its own display values, so 26 of them do
not come back. Nothing a musician sets is lost, and all the sound and safety checks pass. It does mean the
workbench's safety check would block a plugin like this today, until it knows which values are display-only.

### Draw Fast

Four rows show the same movement: a dot that slides across in one second and back in the next, and a spoke that
turns once round and back with it. Every row is at the same place in the movement at every moment. The rows
differ only in how often a second the plugin gives the MPC a new picture.

The pictures have 121 frames, so a row changed 120 times a second lands on a new frame every time. Draw Live's
dot has 61 frames and takes two seconds to cross, so it can show about 30 places a second however fast it is driven.

| Item | What to look for | What it tells us |
|---|---|---|
| 1, 2, 3 | Rows changed 30, 45 and 60 times a second. Is 2 smoother than 1? Is 3 smoother than 2? | Whether the MPC shows more than 30 pictures a second |
| 4 | Turn the knob (1 to 240 a second). At 120, is row 4 smoother than row 3? Above what number does it stop getting smoother? | Where the MPC's own screen stops keeping up |
| all | Does any row stutter or fall behind the others? Does the sound crackle? What does the MPC's Resources screen read with the knob at 30, 120 and 240? | What going faster costs |

A phone's slow-motion video of the whole page is the surest report: by eye, 45 and 60 are hard to tell apart.

Like Draw Live, this card fails the simulator's check that saved settings come back, for the same reason.

### Net Check

Every ten seconds a thread the plugin starts itself makes one try: it asks a public name server (1.1.1.1) for the
address of example.com, connects to that address, asks for the front page and reads the first line of the answer.
It sends nothing else, and keeps only the number the web server answered with. The MPC needs to be on a network
that reaches the internet.

| Row | What to look for | What it tells us |
|---|---|---|
| 1 | Answered, No name, No connection, No answer, or still Waiting | How far a plugin gets towards the internet |
| 2 | 200 | The web server answered properly |
| 3, 4 | Tries made, and how many were answered | Whether it works every time |
| 5 | A number of milliseconds | How long one try takes |
| 6 | A number that goes up once a second | Whether a plugin's own thread runs at all |

The thread never waits longer than a tenth of a second without looking whether the plugin is going away, so
removing the card does not hold the MPC up.

### Radio Check

A thread the plugin starts itself connects to one station, which has to stream plain MP3 without encryption. No
station is kept with the card: copy `shared/station_here.example.h` to `shared/station_here.h`, put a station of
your own choosing in it, and build the card. Without that the card builds, and shows No name. It turns what arrives into sound with minimp3 and leaves it waiting in memory, and the audio thread
plays what is waiting on top of whatever passes through the card. The plugin sends the station one request and
nothing else. The MPC needs to be on a network that reaches the internet. Volume turns the station down.

| Row | What to look for | What it tells us |
|---|---|---|
| by ear | Music, a second or two after the card is added. Is it clean? Does it ever stutter, and does anything else on the MPC stutter with it? | Whether a plugin can play a station at all |
| 1 | Playing. Otherwise No name, No connection, Refused, Tuning in or Lost | How far it got |
| 2 | 200 | The station answered properly |
| 3 | Seconds played, counting up | That it keeps playing |
| 4 | 0, or a small number that stays put | How often the sound ran out. A number that keeps climbing means the network cannot keep up |
| 5 | Close to 100 % | How much sound is waiting. It should fill within seconds and stay full |
| 6 | 44100 | The station's rate. If the MPC's project runs at another rate, the station plays at the wrong speed |
| 7 | About 44100, whatever the tempo | How much sound the MPC asks a plugin for each second. More than the station sends means the sound must run out |
| 8 | The number of calls that second | Whether the MPC asks in more, smaller calls as the tempo rises |
| 9, 10 | The fewest and the most frames in one call | How the MPC cuts its calls up while a sequence plays |

Things worth trying: leave it playing for ten minutes; play pads and a sequence over it; pull the network cable or
switch Wi-Fi off and on again (the sound should stop within seconds and come back by itself, with row 4 one higher);
remove the card (the MPC should not hang).

The decoder, `engine/helpers/online/minimp3.h`, is lieff/minimp3 at commit 142823796ab8 (CC0, its licence is beside it). It was
checked to hold nothing but decoding: it includes only the standard memory and integer headers and the
processor's own, and calls nothing that touches files, the network or other programs.

### Stream Check

Not a test card but a small plugin of MP Forge's own kind: it is written the way the AI's guide
(`directives/reference/online.md`) says to, it lists two stations in `online.json`, and it is built by MP
Forge's safety check with online plugins switched on. So the helper in it is the one MP Forge writes into every
plugin that goes online, which Radio Check never ran. No station is kept with it: copy `online.example.json` to
`online.json` and put in the addresses of two stations of your own choosing. Then build it and send it:

```bash
cargo run --manifest-path engine/Cargo.toml -- gate "screen-tests/Stream Check" --online-plugins
```

```bash
python3 vendor/mpc-plugin-builder-sdk/tools/mpc_remote.py ADDRESS update "screen-tests/Stream Check"
```

Without `--online-plugins` the check must block it, saying online plugins are switched off.

Heard on the Key 37 (2026-11-18): it plays. MP Forge's own helper brings a station in on a real MPC.

| Try | What to look for | What it tells us |
|---|---|---|
| Add it to a track | The first station fades in a second or two later, clean, at the right speed | MP Forge's own helper plays a station on the MPC |
| Station to Second | The first station stops, and the other fades in a second or two later | A plugin can change streams |
| Playing off, then on | Silence, then the station again | A stream can be closed and opened |
| Volume | The station gets quieter and louder, and what passes through the plugin does not | The sound is the plugin's to shape |
| Leave it for ten minutes, with a sequence playing | No stutter, no drift | It keeps up |
| Remove it | The MPC does not hang | The helper's thread ends with the plugin |

## Changing the cards

`make_test_cards.py` writes every card's source file, README and screen. After changing it, run it and then
build the cards that changed:

```bash
python3 screen-tests/make_test_cards.py
```

```bash
python3 vendor/mpc-plugin-builder-sdk/tools/build.py "screen-tests/Test Card"
```

The font test ships JetBrains Mono (SIL Open Font Licence, included beside it) copied from this computer's
own fonts folder.
