# Where the screen design rules come from

Notes for whoever keeps `plugin-screen-design.md`. They are not sent to the AI.

- **Last checked:** 2026-11-14, against MPC OS 3.10.1 and the first release of Akai's MPC Plugin SDK.
- Rules marked **SDK** are stated in Akai's SDK docs. Rules marked **Observed** come from looking at official plugin screens, and the sample is small.

**SDK docs** (Akai's MPC Plugin SDK, <https://github.com/inmusic-software/mpc-plugin-builder-sdk>):

- `docs/ui-guide.md`: pages, the 4 × 4 grid, blocks of four, control choice, kits, what the generator cannot do.
- `docs/ui-reference.md`: screen and content sizes, fonts, colours, tab limits, widget types.
- `ui-kit/README.md`: kit control sizes and themes.
- `docs/host-contract.md` and `AGENTS.md`: name and value lengths, the first-parameter rule, the ban on firmware files.

**Official screens looked at** (public product pages on Akai's store, viewed 2026-11-13, nothing downloaded):

| Plugin | Type | What it showed |
|---|---|---|
| Jura, <https://www.thempcstore.com/instruments/jura/> | Synth, on the MPC | Six pages starting with `MAIN`. Vintage hardware look. Boxed sections with coloured title strips. Four faders for the envelope. Square buttons for waveform choice. Values with units under each control. |
| Tape Echo, <https://www.thempcstore.com/effects/tape-echo/> | Effect, on the MPC | Two pages. Vintage hardware look. Sections titled `CHORUS`, `ECHO`, `REVERB`, `TONE`, each with a light. Name above knob, value between. Input on the left, mix and output on the right. |
| Flavor Pro, <https://www.thempcstore.com/effects/flavor-pro/> | Effect, desktop view only | Six modules, each with one big knob and one slider, detail panels below. One colour per module. |
| A blurred official MPC screen used as a backdrop on several store pages | Unknown plugin, on the MPC | Flat look. Dark panels, red accent, round knobs, an envelope graph, tabs along the bottom. |

**Limits of this evidence:**

- Only two official screens were seen clearly on the MPC itself. The "Observed" rules should be rechecked against more plugins on a real device.
- The SDK ships three example plugins (`SimpleSynth`, `StereoDelay`, `DrumSynth`). Once the SDK is in this project, their layout files are the best concrete examples to attach alongside this directive, and they are free to reuse.
