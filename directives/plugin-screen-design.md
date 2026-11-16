# Plugin screen design rules

How to give a plugin a screen of its own, and how official MPC plugins look and behave so that screen still feels at home on the hardware. Rules marked **SDK** are stated in Akai's SDK docs. Rules marked **Observed** come from looking at a few official screens.

## Order of priority

1. Safety and SDK limits always win.
2. What the user asked for wins over these rules. If they describe a look, build that look, and keep the rules that are about use and not taste: blocks of four, `MAIN` first, values with units. If they ask for something plain, keep it plain: the kit's controls in the grid on a calm ground.
3. When the user says nothing about looks, design a screen that is this plugin's own, as the next two sections describe.
4. The house style further down is about how a screen is used. Keep to it while you design. Where it only describes how the generated grid looks by default, it is a starting point and not a limit.

## Make the screen this plugin's own

A screen that only fills the grid is correct and forgettable: stock knobs in rows on a flat dark page look like every other generated plugin. The musician came here to make something of their own. So design the screen. Do not just lay it out.

- **Start from the subject, the same one the sound came from.** Before any layout, say in one sentence what this plugin is as an object or a place: a worn tape machine, a sonar room under the sea, a neon arcade cabinet, a brass laboratory instrument, a rain-streaked window. Take the colours, materials, shapes and words from that world, not from the default kit. The sound and the screen are one idea: a plugin that sounds like a tape left in the sun should look like one.
- **Plan the look, and write the plan down.** Put it in `DESIGN.md` before you style anything, under a heading `The look`: the idea in one sentence; four to six colours, each with a name, a hex value and its job (the ground, the panels, one loud accent, a quieter second accent, the lettering); the fonts and what each is for; how the knobs, faders and switches are treated; what the backdrop shows; what moves; and the one thing that will make this screen memorable. Read `DESIGN.md` at the start of every later request so the look stays whole, and bring it up to date when the look changes.
- **Make it unlike the musician's other plugins.** The message that starts a new plugin says how their other plugins look: the idea, the kit, the ground colour and the fonts of each. Choose another world, another palette, another font and another arrangement from every one of them. A musician with five plugins should be able to tell them apart from across the room.
- **Spend your boldness in one place.** One strong idea carried all the way through, such as a large moving centrepiece, a backdrop with real depth, or an unusual arrangement of the controls, does more than five small decorations. Keep the rest calm so that the idea reads.
- **Use everything you have.** "The parts of a look" below goes through it: lettering, knobs, faders and switches, backgrounds, arrangement, colour and movement. Each is a choice. Make each one on purpose.
- **Know the signs of a default.** You have not designed anything yet if: the grid is untouched; the background is one flat colour; every control is the same size; every name is in the kit's own lettering; the kit's colour is the only colour; pictures sit in whatever slots were left over; the kit is amber or the page is near-black only because they usually are; the screen could be swapped with another plugin's and nobody would notice.
- **Look, judge, change, and go round again.** `look_at_screen` takes seconds. After every round of styling, look at the whole page, then look closely at any lettering, edge or small detail you are unsure of. Judge the page as a stranger would, on four things, the hardest first:
  1. *Character.* Would someone know this plugin from its screen alone?
  2. *Wholeness.* Do the colours, shapes and lettering belong to one idea?
  3. *Craft.* Is everything lined up and evenly spaced, with nothing cut off and all lettering readable over what is behind it?
  4. *Use.* Can the hands find the main controls at once, and does each block of four still hold together?
  Be hard on your own work: the first version is rarely the good one. Expect to go round at least three times on a new screen, and stop when another round would change only taste.

## The parts of a look

### Lettering

- Call `list_fonts` before you choose, and choose for the subject: a stencil for a field radio, a typewriter face for a lab notebook, a fat rounded face for a toy, a thin geometric one for something made of glass, a script for something from a dressing table. Never take Helvetica, Arial or the first plain face on the list because it is there.
- Use two families at most: one with character for the plugin's name and the titles of sections, and a quieter one for the names of controls.
- Give the plugin a name plate: its own name lettered as artwork (a `words` layer), sized and placed as part of the picture, like the badge on a real machine. It is not a caption in a corner.
- Letter the names of the controls too. Hide a control's own name (`"hidden": true` in its `name` part) and put the name beside its dial as artwork in the plugin's font. On a new screen do this for every control, unless the musician asked for plain.
- A control's value changes as it is turned, so it stays in the MPC's own lettering. Pick the one of Titillium Web, Roboto and Nexa nearest in feel to your fonts, and give it a size and colour that belong with them.
- Keep one way of writing names, all capitals or not, across every page. Nothing the musician has to read is under 20 pixels high, and all of it stands clear of what is behind it.

### Knobs

- The dial's shape comes from the kit and cannot be redrawn. Its colour is yours (see "Colour"), and so is everything around it.
- **Size.** A dial is drawn sharp at any size. Make the control that matters most large, 160 to 220 pixels across, and fine trims small, 60 to 70. Use three sizes at most, and let size say how much a control matters.
- **Surround.** Draw what the dial sits in as part of the backdrop: a bezel or a sunk well, a ring of tick marks, a scale with words or numbers at its ends, a collar in the accent colour, a shadow under it, a plate with screws. Move and size the controls first, read where each dial is from `style_screen` with an empty list of changes, and draw each surround to those numbers.
- **A ring that moves.** Draw a picture that follows a control. With `style_screen`, place it over that control, size it larger than the dial and set its `depth` to `"behind"`. Then a lit arc, a row of lamps or a set of teeth shows all round the dial and fills as it is turned.
- **Name and value.** Under the dial and centred reads as hardware. Beside it reads as a console. Choose one arrangement and keep it for every knob of the same size.

### Faders, switches and choices

- A fader can be any length and thickness: long and thin like a mixing desk, short and fat like a toy. Draw its slot, its scale marks and the words at its ends in the backdrop. Faders lie flat; the MPC has no upright ones here.
- A switch keeps the kit's size. Give it a plate, a drawn lamp beside it that follows it, and its name lettered like the others.
- A row of choice buttons sits well on a strip of its own, drawn behind it, with the section's title at one end.
- The kind of control still follows its job (see "Choosing a control"). What you vary is size, proportion, surround and lettering.

### Backgrounds

- Every page has a backdrop you draw, unless the musician asked for plain. One flat colour is not a backdrop.
- A backdrop is an object or a place, and it has depth. Light comes from one direction: a soft fade across each surface, a bright line on top edges and a dark one under them. Panels are raised or sunk. Surfaces have a texture built from many small shapes: grain, weave, perforation, rivets, scratches, dust, scan lines. Old things show wear. A darker rim draws the eye to the middle.
- It can be a scene and not only a panel: a horizon, a night sky, a sea floor, a workbench, a window, with the controls set on something in it.
- Keep it quiet and even behind lettering and dials. Put the detail in the gaps and the margins.
- The pages share one world and need not share one picture. `MAIN` can be the front of the machine and `MORE` its back, its inside or its manual.
- A light screen is allowed: cream, steel, chalk, faded colour. Dark is Akai's habit, not a rule. Choose the ground from the subject, then check that the kit's dials and all lettering read on it.
- Simple panels, bands and fades can be colour blocks laid with `style_screen`. Anything richer is a backdrop drawn as SVG.

### Arrangement

- The grid is where the controls start, not where they stay. Compose the page: one hero with room round it; sections as clusters, with wider gaps between clusters than inside them; edges and centres on shared lines; a lopsided balance when the subject wants one. Controls can follow an arc, a slope or the outline of what is drawn behind them.
- Keep what the hands need: the order of the controls, each set of four reading as a set, and dials at least 60 pixels across with clear space between them.
- Empty space is part of the picture. Leave it on purpose, or give it to the centrepiece. Never leave it by accident.

### Colour

- Take four to six colours from the subject. One is loud and used sparingly: on the hero, the lit state, the name plate. The rest are quiet.
- The controls come from one kit for the whole plugin: `mpc-red`, `amber`, `cyan`, `lime`, `violet` or `mono`. Its colour can be any you choose: write `"kit": { "theme": "mono", "accent": "ffd08a2e" }` in `ui.json`, where the eight digits are `ff` and then the colour. Choose the kit and its colour last, to fit the palette, not first to set it.

### Movement

- Give the screen one thing that moves with the sound or with a control and shows what the plugin is doing. Make it large and part of the idea, not an ornament in a spare slot.

## The screen you are designing for

- **SDK** The screen is 1280 × 800. Your page content gets 1280 × 628, starting 110 px from the top.
- **SDK** The MPC draws the top bar (plugin name, preset, load, save). You do not design it.
- **SDK** The MPC draws your pages as tabs along the bottom, one per function key. You only name them.
- While a hardware knob is turned, the MPC slides its own panel, about 270 pixels wide, over the right of the page. At rest nothing is covered. So the right-hand column holds controls and pictures like any other. Only keep a picture that has to be watched while a knob is turned out from under it.
- **SDK** The MPC draws images at their own size and does not scale them. A control's box must be at least as big as its artwork.

## What a generated page really looks like

**SDK** These are the measurements the generator writes, in pixels of the 1280 × 628 page. Design with them in mind, because the result is sparser than a 4 × 4 grid sounds.

- A slot is 278 wide and only 107 tall. Slots start at x = 23, 343, 663, 983 and y = 45, 196, 346, 497, so there is a 42-pixel gap between columns and about 44 between rows.
- A knob is a 90-pixel dial at the left of its slot. Its name (grey, small) and its value (white, a little larger) sit to the right of the dial, not above or below it.
- A fader is its name at the top left, its value at the top right, and a 248-pixel bar under them.
- An on/off switch is its name and value at the left and a small pill at the right. A choice of 2 to 4 is its name above a row of buttons.
- A `text` control is one line of small white lettering, 30 pixels high, at the left of its slot. It reads as a caption. It is not a title strip and it does not centre over a column.
- Controls have no panel or box of their own. They sit straight on the page's background colour, or on a backdrop picture when there is one.
- Two rows of controls fill only the top half of the page. The bottom half stays the plain background unless you put controls or pictures there.

So a page looks good when its slots are used in whole rows, when what belongs together is in one row or one column, and when the empty half is either left calm on purpose or given to one or two large pictures. It looks bad when captions float alone in a row, when small pictures are scattered one per slot, or when a row holds one stray control.

## The house style

1. **SDK** Start with a page called `MAIN`. Its top row holds the controls people reach for first, as knobs or faders, never switches.
2. **SDK** Think in blocks of four. The 16 hardware knobs work in groups of four, so each row or column of four is one related set: the four envelope stages, or cutoff, resonance, drive, and key tracking.
3. **SDK** Fill a page left to right, then top to bottom. Leave a slot empty or use a text heading before you add a stray control.
4. **Observed** Follow the signal from left to right. Input and source on the left, shaping in the middle, mix and output on the right.
5. **Observed** Official screens box each section and title it with a short uppercase strip, such as `ENV 1`, `CHORUS`, `ECHO`, `TONE`. The generated grid has neither, so draw a panel behind each group in the backdrop and letter its title on it.
6. **SDK** Show values with units, such as `1.2 kHz`, `358 ms`, `-7.0 dB`, `40 %`. Never rely on colour alone to carry meaning.
7. **SDK** Keep a section's mode switch in the same row as the knobs it changes.
8. **SDK** Use the same kind of control for the same kind of job on every page. `docs/ui-guide.md` has the table of which kind suits which job.
9. **SDK** Put rarely used settings on a last page called `MORE`.

## Pages

- **SDK** Page names are 8 characters or fewer.
- **Observed** Page names are uppercase and name a section, often two joined by a slash: `FILTER/ENV`, `CHORUS/FX`, `VOICE/ARP`.
- **Observed** Effects usually need one or two pages. Synths usually need four to six.
- **SDK** The limit is 32 pages, but a plugin that needs more than six is probably doing too much.
- **Observed** A page holds roughly 8 to 16 controls. If `MAIN` needs more, move the extras to a section page.

## Words on screen

- **SDK** Parameter names: about 12 characters. Value text: about 8 characters.
- **Observed** Use plain musician words: `Repeats`, `Wobble`, `Tone`, `Mix`. Avoid engineering terms unless the user used them.
- **Observed** Section titles and page names are uppercase. Parameter names are either all uppercase or all title case. Pick one and keep it.

## Never do this

- Never copy artwork, logos, panel layouts, or names from Akai, AIR, or any other maker's plugin. Follow the conventions, not a product.
- Never take files from MPC firmware.
- Never use the words "Akai", "MPC", or "AIR" in a plugin's name or on its screen.
- Never imitate a specific vintage instrument's branding. "Looks like an 80s polysynth" is fine. A named model's logo or exact panel is not.

## Before you say the screen is done

- You have looked at every page as it stands now, and closely at its lettering, and it has a character of its own (or the musician asked for plain).
- `DESIGN.md` says what the look is, and the screen is what it says.
- On a new screen that was not asked to be plain: every page has a backdrop with depth, the plugin has a name plate in a font chosen for it, the controls' names are lettered in the plugin's font, the controls are not all one size, and they sit where the picture wants them and not where the grid left them.
- It looks unlike each of the musician's other plugins.
- `MAIN` exists, is first, and its top row is knobs or faders.
- Every row or column of four is one related set.
- Every page name is 8 characters or fewer.
- Every value shows a unit.
- One kit, and one palette carried through every page.
