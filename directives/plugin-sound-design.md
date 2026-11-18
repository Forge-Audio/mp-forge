# Plugin sound design rules

The sound is the plugin. A musician forgives a plain screen long before a dull or dirty sound, so give the sound at least the care you give the screen. This part says how to give a plugin a sound of its own, how to keep that sound clean, and how much movement and routing to build in.

## Order of priority

1. Safety always wins: nothing here excuses a crash, a NaN or a blown CPU budget.
2. What the musician asked for wins over these rules. If they want it harsh, gritty or broken, build that, and build it on purpose.
3. When the musician says little about the sound, design one that is this plugin's own, as the next section describes.

## Make the sound this plugin's own

One raw oscillator into a filter, or a textbook delay with a Mix knob, is correct and forgettable. Design the sound. Do not just wire it up.

- **Start from the sound.** Before any code, say in one sentence what this plugin sounds like and what sets it apart: "a delay whose repeats sink and smear like a tape left in the sun", not "a delay with modulation".
- **Plan the sound, and write the plan down.** Put it in `DESIGN.md` under a heading `The sound`, before you write audio code: the idea in one sentence; the signal flow, one line for each stage; what each control does to the sound and over what range; what moves by itself and what the hands move; how it answers playing (an instrument) or the sound coming in (an effect). Keep it up to date when the sound changes.
- **Spend the character in one place.** One strong idea carried all the way through does more than five small features. Let the other stages serve it.
- **Make every control earn its place.** Each one changes the sound clearly along its whole travel, with no dead stretch at either end and the most useful settings near the middle. Use `logParam` for frequencies and times and a `skew` where the detail is near zero. If two knobs always have to be turned together, make them one.
- **Offer a few big controls.** A control that moves several things at once in the code (brighter, tighter and louder together) is worth more to a musician than three small ones they have to balance. Lead `MAIN` with these.
- **Let playing shape an instrument.** How hard a note is played should change its tone, not only its loudness: brighter, sharper or more driven when hit hard. Follow the keyboard where it suits (higher notes brighter or shorter). Answer pitch bend, the mod wheel and pressure (`dsp/midi_cc.h`): the MPC passes all of them on to an instrument.
- **Let an effect listen.** An effect hears no notes, but it does hear the sound coming in and the song's tempo. Movement that follows how loud the input is, and times that lock to the tempo (cookbook section 9), make an effect feel played. Give it a `Mix` unless it only makes sense fully on.
- **Keep it alive.** A sound that is the same from the start of a note to its end, and the same on every note, is the sound's version of an untouched grid. Small differences between voices, slow drift, and something that moves while a note is held bring it to life. See "Movement and routing" below.
- **Good from the first note.** The starting settings are the first thing heard. They should show the plugin's idea at once, at a level that sits well with other plugins.
- **The first preset is `Default`.** Write it first, with exactly the starting value of every control, in the controls' order: `addProgram("Default", { ... })`. An MPC shows the first preset's name when the plugin loads, so the name and the sound then agree, and choosing `Default` always brings the plugin back to how it began. When you change a control's starting value, change it in `Default` too.
- **The other presets show the range.** Three to eight after `Default`, each a different use and not a small variation, named for what is heard (`Seasick Tape`, `Glass Bells`). Keep them about equally loud. Between them they use every kind of movement the plugin has.
- **A plugin that already has presets keeps their order.** An MPC project remembers its preset by its place in the list. If a plugin you are asked to change has presets and no `Default`, leave them as they are, and add `Default` at the front only when the musician asks for it.
- **Know the signs of a default.** You have not designed a sound yet if: nothing changes while a note is held; playing harder only makes it louder; half a knob's travel does nothing; every preset is the first one with two knobs moved; the only thing between this and the example plugin is its name.

## Keep the sound clean

Dirt belongs in a sound only when it is the idea. Clicks, zipper noise, harsh false tones, sudden jumps in level and mud are faults, and a musician hears them at once.

- **Smooth everything that is heard.** Level, cutoff, delay time, mix, pan, drive and pitch go through `dsp::Smoother` or `dsp::LinearRamp`, never straight from the control to the sound. That covers controls, movement sources and switches alike.
- **Change switches and choices without a click.** Fade from the old sound to the new over 10 to 20 ms, or make the change at a moment of silence.
- **No clicks at the ends of notes.** The shortest attack is about 1 ms and the shortest release about 5 ms, whatever the knobs say. A voice taken for a new note is faded out over a few milliseconds first. Start an oscillator's phase afresh only when the voice was silent.
- **Move delay times without tearing.** Read between samples (`readLinear`), and either glide the time, which bends the pitch like tape, or fade between the old and the new reading point. Choose the one that suits the plugin.
- **Keep false tones out.** Use `PolyBlepOsc` for saws and squares, never a bare ramp or step. Hard distortion, wave folding, sync and fast frequency modulation all fold high tones back down as harsh ones. Prefer curves that bend gently (`fastTanh`, `softClip`), take the highs down before a hard stage, and run a heavy stage at twice the sample rate when the processor budget allows. When the roughness is the idea, as in a bit crusher, say so in `DESIGN.md` and leave it in.
- **Play sound files smoothly.** Read between samples, in a straight line at least and with a four-point curve when the pitch moves far from the original. Fade a file in and out over a few milliseconds where it starts and stops, and fade across the join of a loop.
- **Take out any standing offset.** Shaping that treats the top and bottom of the wave differently pushes the whole sound off centre. Follow it with `dsp::DCBlocker`.
- **Stage the levels.** At its starting settings a plugin played hard peaks somewhere between -12 and -6 dB, and no setting of its knobs takes it past 0 dB. Scale a sum of voices by about one over the square root of their number. A `softClip` at the very end is there for the rare peak; if it is working all the time, turn the level down earlier.
- **Keep the loudness steady.** Turning up drive or resonance should change the tone, not jump the level, so turn the output down as they rise. An effect at its starting settings is about as loud as the sound going in: the musician compares it with the effect switched off, and should hear the effect, not a change of volume.
- **Tame what feeds back.** Feedback stays under 1 and has a gentle clip inside the loop. A filter at full resonance stays in bounds with every other knob at its worst.
- **Look after the stereo picture.** Keep the low end, below about 120 Hz, in the centre when you widen a sound. Do not widen by turning one side upside down: it vanishes on a mono speaker.
- **Silence is silent.** An instrument with no notes sounding puts out exactly nothing. An effect fed silence dies away to nothing, and says how long that takes in `tailSamples`.
- **The same everywhere.** Work in milliseconds and hertz from the sample rate you are given, and do slower work every 16 or 32 samples, not once a block, so the sound does not change with the block size.

## Movement and routing

Movement is what turns a tone into a sound: a slow wobble, a filter that opens as a note is held, a delay that drifts. Build some into every plugin it suits, and size it to the plugin. More routing is not better. A musician with two good knobs explores further than one facing a wall of choices.

What can move a sound:

- In any plugin: a slow wave (`dsp::LFO`: free in Hz or locked to the tempo with `setTempoSync` and `syncToPpq`), random steps (its `S&H` shape), a slow random drift.
- In an instrument as well: how hard the note was played, where it is on the keyboard, the mod wheel, pressure, a second envelope (`dsp::ADSR`), a random amount for each note.
- In an effect as well: how loud the sound coming in is (take its size and smooth it with a fast rise and a slower fall), and a pattern of steps in time with the song.

How much to build:

- **A small plugin, or one whose musician asked for simple: movement built in.** Wire one or two sources to where they sound best, and give each a rate and a depth, named for what is heard: `Wobble`, `Drift`, `Pump`. No routing choices.
- **A fuller effect or a simple synth: movement you can point.** One or two sources, each with its own controls, an amount, and a `To` choice of three to five places it can go.
- **A synth with several pages, or a musician who asks for deep sound design or a mod matrix: a `MOD` page.** Up to four routes, one in each column: `Source`, `To`, `Amount`. The fourth row holds the sources' own controls, such as a wave's rate and shape. Offer at most eight sources and eight places to go. Go past four routes only when the musician asks, and never past eight.

Rules for all of them:

- A route's amount runs both ways, from minus to plus, and rests at zero. A route at zero changes nothing and costs next to nothing.
- Send movement only to things that are already smooth controls: tone, pitch, level, pan, drive, time, mix, another wave's rate. Never to a switch, a choice or the master volume.
- Movement is added to where the knob stands. It never turns the knob itself. Add it in the knob's own measure: octaves or semitones for frequencies and pitch, decibels for levels. Then clamp the sum to the knob's range before it reaches a filter or a delay, and smooth it like any other change.
- Work the sources out every 16 or 32 samples, not for every sample. How hard a note was played, its envelopes and its random amount belong to each voice. A slow wave is shared by all voices unless the musician wants otherwise.
- Use plain names for sources: `LFO 1`, `Envelope`, `Velocity`, `Wheel`, `Pressure`, `Loudness`, `Random`. The places to go carry the names of their knobs.
- The starting settings and most presets already use some movement, so the musician hears what it does before they find the page. One preset has none.
- When you add movement to a plugin that already exists, add the new controls after all the old ones in the code. Saved projects find their settings by that order.

## You cannot hear it, so reason it through

You can look at the screen. You cannot listen to the sound. So be stricter here, not looser.

- For each control, think through the sound at its lowest, its middle and its highest, and with the other controls at their worst: full resonance with full drive and full feedback. Say what stops each case from clicking, clipping or blowing up.
- What `check_plugin` reports about the sound matters as much as a failure. A warning of clipping, of silence or of heavy processor use is a fault in the sound: fix it and check again, and if you could not, say so in one sentence of your final message.
- Tell the musician what the plugin is meant to sound like, not that it sounds good. That is theirs to judge. Do not tell them you have not heard it: they know, and they will listen.

## Before you say the sound is done

- `DESIGN.md` says what the sound is, and the code does what it says.
- Every control is heard along its whole travel, and every control that is heard is smoothed.
- Notes start and stop without a click, and nothing sounds after all notes are off.
- No setting of the knobs clips, and the starting settings are neither quiet nor hot.
- Something moves in the sound, unless the musician asked for it still.
- The first preset is `Default` and holds every control's starting value. The presets after it differ from each other, are about equally loud, and show the movement.
- The check passed with no warning about the sound that you have not dealt with.
