# Pictures and animation on a plugin's screen

MP Forge's own guide. Read it before you write `pictures.json`, a drawing or the picture clock.

`ui.json` gives pages of knobs, faders, switches and sliders. On top of that you can put pictures you draw yourself on the screen: still artwork, a picture that plays round and round on its own, and a picture that changes as the musician turns a control. You write each drawing as SVG, and the safety check turns it into the image file the MPC plays. Use pictures when the musician asks for animation, artwork or a look of their own, and when an empty part of a page would be better with something on it. Do not add them to a plugin whose musician asked for something plain.

List the pictures in `pictures.json`, and write each drawing to `pictures/<name>.svg`:

```json
{ "pictures": [
  { "name": "reels", "page": "MAIN", "where": { "row": 3, "column": 1, "rows": 2, "columns": 4 }, "motion": "loops", "frames": 12 },
  { "name": "glow", "page": "MAIN", "where": { "row": 2, "column": 4 }, "motion": { "follows": "Drive" }, "frames": 24 },
  { "name": "paper", "page": "MORE", "where": "backdrop" }
] }
```

- `name`: lowercase letters, digits and dashes. It is also the drawing's file name.
- `page`: the page's `name` in `ui.json`.
- `where`: a block of slots on the page's 4 by 4 grid, counted from 1 (`rows` and `columns` are 1 when left out), or `"backdrop"` for the whole page behind the controls. A block may only cover slots with no control in them, so leave those slots out of `ui.json` or mark them `"empty"`.
- `motion`: `"still"` (the default), `"loops"`, or `{ "follows": "<control name>" }`. A picture that follows a control shows its first frame when the control is at its lowest and its last frame at its highest. A `"loops"` picture needs the plugin's `Picture Clock`: see "Pictures that play on their own" below.
- `frames`: needed when the picture moves, from 2 to 64. A moving picture is only as smooth as its frames. An MPC shows up to 30 pictures a second, so a looping picture is at its smoothest with 30 frames for each second its loop takes: 30 for a one-second loop, 60 for two seconds. Give a small picture that many. A large one cannot have them (see the memory rule below), so give it as many as fit. A picture that follows a control is fine with 24, and smoother to turn with more.

A drawing is one SVG file. Anything that should change from frame to frame is written as a sum in curly brackets, which MP Forge works out for every frame:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1238 258">
  <g transform="translate(300 129) rotate({90 * t})">
    <circle r="90" fill="#2e2924" stroke="#d8c9a8" stroke-width="6"/>
    <rect x="-8" y="-80" width="16" height="50" fill="#d8c9a8"/>
  </g>
  <circle cx="900" cy="129" r="{40 + 12 * sin(t)}" fill="#ff5a1f" opacity="{0.4 + 0.6 * t}"/>
</svg>
```

- `t` runs from 0 to 1 across the frames. In a loop it stops one step short of 1, so the last frame leads back into the first. `frame` and `frames` are also there.
- A sum can use numbers, `+ - * /`, brackets, and `sin`, `cos`, `abs`, `round`, `floor`, `min`, `max`. `sin` and `cos` count in whole turns, so `sin(t)` rises, falls and returns exactly once per loop.
- For a loop, every moving thing must end where it began. Turn a shape with four matching arms by `{90 * t}`, not `{360 * t}`: it looks the same and moves four times more smoothly for the same frames.
- Give the `viewBox` the shape of the place the picture goes, or it will be shrunk to fit with empty bands beside it. One slot is 278 wide and 107 tall. A block is `278 + 320 × (columns − 1)` wide and `107 + 151 × (rows − 1)` tall. A backdrop is 1280 by 628.
- The background is see-through unless you draw one. Take colours from the page: the `style` and `kit` in `ui.json`.
- Draw with shapes, paths and gradients only. Lettering, other images, scripts and SVG's own animation tags are not drawn. Put words on the screen with a `text` control in `ui.json`. Set colours and sizes with attributes, not a `<style>` block, because every curly bracket is read as a sum.
- Every frame is held in the MPC's small memory. Frames × width × height may not pass 6 million for one picture, so a large picture gets fewer frames: a backdrop that moves can have 7, a two-row block across the page 18.

`check_plugin` draws the pictures before it builds, and tells you which picture and which sum is wrong when one cannot be drawn. Once they can be drawn, its result shows you each picture: up to four frames from the first to the last, one above the other, on a dark stand-in for the page. Look at every one before you call the plugin finished, and redraw any that is wrong:

- a shape that is cut off at an edge, off-centre, or lost in empty space;
- colours too faint to read against the dark page, or that clash with the page's `kit`;
- frames that all look alike, which means nothing moves, or a loop whose last frame would jump on its way back to the first;
- a follower whose first and last frames do not look like the control at its lowest and highest.

## Pictures that play on their own

An MPC draws a picture again only when one of the plugin's controls changes. It never plays a picture by itself: a `"loops"` picture with nothing turning it stands still on the MPC. So a plugin with a `"loops"` picture keeps one extra control, called `Picture Clock`, which is on no page and which the plugin turns round and round itself. Every looping picture follows it: one turn of the clock is one play of the picture. MP Forge ties the pictures to the clock and plays them at the clock's speed in the preview. The safety check runs the plugin and blocks it when the clock does not turn. All you write is the clock.

Declare it last, after every other control, add `kPictureClock` to the end of your list of controls, and leave it out of `ui.json`:

```cpp
        {
            // On no page: the plugin turns it itself, and the pictures that play on their own follow it.
            mpc::Param pictureClock = mpc::floatParam("pictureclock", "Picture Clock", 0.0f, 1.0f, 0.0f, "", 2);
            pictureClock.automatable = false;
            addParam(pictureClock);
        }
```

Add this to the plugin's class, and call `turnPictureClock(ctx);` once in every `process()`:

```cpp
    // One play of every looping picture takes this long, in this many steps: 30 a second, the most an MPC shows.
    static constexpr float kClockTurnSeconds = 2.0f;
    static constexpr int   kClockSteps       = 60;

    // An MPC draws a picture again only when a control changes, so the plugin turns this one itself.
    void turnPictureClock(const mpc::ProcessContext& ctx)
    {
        const int shown = (int)std::lround(getNormalised(kPictureClock) * (float)(kClockSteps - 1));
        if (shown != clockStep_) { clockStep_ = shown; framesIntoClockStep_ = 0.0f; }   // the MPC set it: go on from there
        framesIntoClockStep_ += (float)ctx.numFrames;
        const float framesInAStep = (float)ctx.sampleRate * kClockTurnSeconds / (float)kClockSteps;
        if (framesIntoClockStep_ < framesInAStep) return;
        framesIntoClockStep_ -= framesInAStep;
        clockStep_ = (clockStep_ + 1) % kClockSteps;
        setNormalised(kPictureClock, (float)clockStep_ / (float)(kClockSteps - 1));
        notifyHostParameterChanged(kPictureClock);
    }

    float framesIntoClockStep_ = 0.0f;
    int   clockStep_           = 0;
```

- Keep the line that goes on from where the MPC set the clock. The MPC puts every control back when a project loads, and the safety check tests that a control it put back stays put.
- If the plugin acts when a control changes (`parameterChanged`), let it ignore the clock.
- Every looping picture takes one turn of the clock for one play, whatever its `frames`. To make something move faster, draw more of the movement into the loop (`{720 * t}` is two turns in one play), or shorten `kClockTurnSeconds`. A turn may take from half a second to 6 seconds. Keep the steps at 30 a second: change `kClockSteps` with `kClockTurnSeconds`, so a one-second turn has 30 steps. An MPC's screen shows no more than 30 pictures a second, and steps beyond that only cost it work.
- One exception: when a looping picture is a backdrop or covers more than two rows of the page, keep the clock at 15 steps a second (`kClockSteps` 30 for a two-second turn). What a large moving picture costs an MPC at 30 a second has not been measured yet.
- Do not give a picture `{ "follows": ... }` of a hidden control of your own to make it play round and round. Write `"loops"` and use the clock.
- A picture that shows what the plugin is doing, such as lamps that light for the notes held, is different: it should only move when that happens. Give it a hidden control of its own, declared like the clock (on no page, not automatable), and have the picture follow that control. Set the control from `process()` when the thing it shows changes, and call `notifyHostParameterChanged` every time you set it. It may stand still while nothing is played. MP Forge's preview shows it moving while the plugin is played there.

## Putting pictures and controls together

- Controls have no background of their own, so a backdrop shows through all around each dial and its lettering. When the musician wants the controls on or in the artwork (knobs sitting on a tape machine, a panel around each section), draw one backdrop for the page and draw the artwork around the places the controls sit. In a slot whose top left is (x, y), a knob's dial is the circle centred at (x + 53, y + 53) with radius 45, and its name and value fill the box from (x + 110, y + 12) to (x + 266, y + 94). Keep what is behind the lettering dark and plain so it stays readable.
- "Put the controls on the drawings", "make the knobs part of the picture" and "the controls should match the drawings" all mean that. They do not mean a caption above a picture.
- A picture that follows a control belongs right beside that control: the slot directly below or above it in the same column, or a block that spans the columns of the controls it shows. The control's own name then labels the picture, so do not add `text` controls as captions for pictures.
- Prefer one or two large pictures to several small ones. A picture one slot tall is only 107 pixels high, and detail that small is lost on the MPC.
- When a page changes, look at the new view of the whole page that `check_plugin` shows you, not just the part you touched. If the page still looks sparse or scattered, fix that before you finish, even if the musician only asked about one thing on it.

A picture is shown again only after its drawing changes, so what you were shown last is how it still looks. To see a drawing in its place on the page, use `look_at_screen`, with an `area` round it for a close-up. Redraw until it looks right in its place, and change a drawing with `edit_plugin_file` when only a colour or a number is wrong. Bold shapes read best from where the musician stands, and rich detail is welcome where a picture is large enough to carry it. A looping picture plays at the speed of the plugin's `Picture Clock`, in the preview and on the MPC alike.
