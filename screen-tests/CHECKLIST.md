# Screen test cards: what to look at on the MPC

Seven small effect plugins that do nothing to the sound. Their screens try things out so we can see what an
MPC plugin screen really supports before building the edit mode.

- **Test Card** holds the checks the edit mode depends on.
- **Draw Path, Draw Plot, Draw Knobs, Draw Guess, Draw Extras** each hold one kind of less tested widget.
  They are kept apart because one entry the MPC dislikes blanks a card's whole screen.
- **Draw Live** tests whether pictures can move by themselves, driven by the plugin.

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
2. Install or refresh all seven (put the MPC's address in place of `ADDRESS`):

   ```bash
   python3 vendor/mpc-plugin-builder-sdk/tools/mpc_remote.py ADDRESS update "screen-tests/Test Card" "screen-tests/Draw Path" "screen-tests/Draw Plot" "screen-tests/Draw Knobs" "screen-tests/Draw Guess" "screen-tests/Draw Extras" "screen-tests/Draw Live"
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
