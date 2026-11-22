# The MPC Sample, and how an app for it is written

## What the device has

| Part | What an app is told | Notes |
| --- | --- | --- |
| Screen | nothing: the app draws on it | 320 dots across, 240 down, full colour, 2.4 inches. It is not a touch screen. |
| 16 pads | `padHit(pad, strength)`, `padLetGo(pad)` | Numbered 1 at the bottom left to 16 at the top right, four to a row. `strength` runs from just above 0 to 1. Each pad has a light of any colour. |
| 3 knobs | `knobTurned(knob, stands)` | K1, K2, K3, in a row above the pads. They have end stops: `stands` runs from 0 to 1. |
| Fader | `faderMoved(stands)` | Left of the pads, 0 at the bottom to 1 at the top. |
| Dial | `encoderTurned(clicks)`, and `Button::Encoder` when pressed | Right of the pads. No end stops: it says how many clicks it turned, and which way. |
| Buttons | `buttonPressed(button)`, `buttonLetGo(button)` | Listed below. Most can be lit. |
| Sound | `makeSound(sound)` | Stereo in and out, 44100 frames a second, 128 frames at a time. |
| Sound files | nothing: the app reads `samples()` | The WAV and MP3 files the musician added to the app in MP Forge, up to 64. There may be none. |
| MIDI in | `noteOn(note, strength, channel)`, `noteOff(note, channel)`, `controllerMoved(controller, stands, channel)`, `pitchBent(bend, channel)` | From a keyboard, a pad controller or a computer plugged into the USB socket or the MIDI IN socket. There may be nothing plugged in. Which socket and which channel is chosen on the kit's page, `chooseMidiIn()`. |
| Sound in | `soundInChosen(from)` when the musician has chosen where it comes from | The jacks at the back (`SoundIn::LineIn`), the built-in microphone (`SoundIn::Mic`), USB (`SoundIn::Usb`), or nowhere (`SoundIn::Off`, as it starts), chosen on the kit's page, `chooseSoundIn()`. |
| SD card | `sampleLoaded(number)` when a file from it has been loaded | Folders and WAV and MP3 files, looked through with the kit's page, `browseCard(number)`. There may be no card in. |

The buttons, by the names the kit gives them: `B1`, `B2`, `B3` (above the screen, for whatever the screen says they do), `Sample`, `Seq`, `PadFx`, `KnobFx`, `Shift`, `PadBank`, `Erase`, `NoteRepeat` (left of the pads), `Chop`, `Mute`, `Loop`, `SixteenLevels`, `SampleSelect`, `TapTempo`, `Minus`, `Plus`, `SampleRecord`, `SeqRecord`, `Stop`, `Play` (right of the pads), and `Encoder`.

The words printed on the device's buttons are Akai's sampler's. An app may use a button for anything, but the musician sees the printed word, so use a button for what its word suggests where you can: `Play` and `Stop` to run and halt, `Minus` and `Plus` to step a value, `Shift` held for a second meaning, `Mute` to silence.

The knobs and the fader are told once when the app starts, with where they stand, and then whenever they move. So an app never has to guess where a knob is.

## The shape of an app

```cpp
#include "mpc_sample_app.h"

class MyApp : public sample::App {
public:
    void start(double framesASecond) override { /* set up; ask for memory here */ }
    void padHit(int pad, float strength) override { /* start a note */ }
    void knobTurned(int knob, float stands) override { /* keep where it stands */ }
    void makeSound(sample::Sound& sound) override { /* write sound.frames frames to outLeft and outRight */ }
    void draw(sample::Screen& screen, sample::Lights& lights) override { /* draw the whole screen, set the lights */ }
};

MPC_SAMPLE_APP(MyApp)
```

Only `makeSound` and `draw` have to be written. The others do nothing unless the app says what they do.

MP Forge calls one function at a time. A pad hit never arrives in the middle of `makeSound`, so the same variables can be read and written by all of them without locks.

## Sound

- Write every frame of `outLeft` and `outRight` every time. `sound.silence()` writes silence to both.
- `inLeft` and `inRight` hold what came in at the device's inputs. An effect reads them, changes the sound, and writes it out. Which inputs those are is the app's to choose, as "Where sound comes in from" below says. They hold silence until the app says where to listen. In MP Forge the musician's computer stands in for the device: while the app listens anywhere, they hold the computer's own sound input, a few hundredths of a second late.
- Keep a fixed number of voices in an array, such as sixteen, one for each pad. Never grow a list in `makeSound`.
- A note that starts or stops at full level clicks. Give every voice a short rise, a few thousandths of a second, and a fall.
- A control that jumps makes the sound jump. Move what is heard towards where the control stands a little each frame, such as `heard += (wanted - heard) * 0.002f`.
- Count time in frames. A step sequencer counts the frames it has made and moves on a step every `framesASecond * 60 / tempo / 4` of them.
- Sixteen voices at full level add up to sixteen. Scale each voice so that all of them together stay under 1, and pass the mix through something gentle such as `x / (1 + |x|)` as a last guard.

## The musician's sound files

`samples()` holds the sound files the musician added to the app, in the order of their names. They are there from `start` on and never change while the app runs. An app does not open files and cannot: this is the only way a recorded sound reaches it.

```cpp
const sample::Samples& files = samples();
files.count();                       // how many there are, and 0 is common
const sample::Sample& kick = files.at(0);          // numbered from 0; a number with no file gives an empty, silent one
const sample::Sample* named = files.named("kick"); // by name, whatever its capitals; nullptr when there is none
kick.name();                         // "kick" for kick.wav
kick.frames();                       // how long it is
kick.isStereo();
kick.left(frame), kick.right(frame);               // one frame, between -1 and 1; silence before the start and after the end
kick.leftAt(place), kick.rightAt(place);           // between two frames, for another speed
```

- MP Forge has already brought every file to the rate the app runs at. Moving on one frame for every frame made plays a file as it was recorded. Moving on by `step` plays it at another pitch with `leftAt(place)`: 2 is an octave up, 0.5 an octave down, and a step below 0 plays it backwards.
- A voice that plays a file keeps a pointer to the `Sample`, a `place` in it as a `double`, and a `step`. It has ended when `place` passes `frames()`, or falls below 0 going backwards. Reading outside the file is safe and silent, so nothing has to be checked in the inner loop.
- A file with one side has the same sound left and right. Play both sides always, and it is right for either.
- A file can start or be cut off at full level. Bring every voice in over a few thousandths of a second, and fade it out the same way when it is stopped before its end.
- A loop goes round by taking `frames()` off `place` when it passes the end. A slice is a start and an end in frames.
- To draw a file's shape, work out how loud it gets in each column once, in `start`, from a few frames of each column. Never read a whole file in `draw` or `makeSound`.
- Files add up like any other voices. Scale the mix and hold it under 1.
- With `count()` at 0 the app still has to be worth opening: it plays sounds of its own, or its screen says that sound files are added with + in MP Forge.

`mp-forge/examples/SamplePads.cpp` is a whole app that does all of this.

## The SD card

The musician can pick a WAV or MP3 file from the device's SD card while the app runs. Either reaches the app as plain sound, so the app does not have to know which it was. The kit has the page for it:

```cpp
void buttonPressed(sample::Button button) override {
    if (button == sample::Button::SampleSelect) browseCard(lastPadHit_);   // the file picked becomes sample `lastPadHit_`
}

void sampleLoaded(int number) override {
    voices_[number] = Voice{};     // what was playing the old sample stops
    measureShapeOf(number);        // and whatever was worked out from it is worked out again
}
```

- `browseCard(number)` opens the page over the whole screen. It shows the folders and the WAV and MP3 files of the card, a folder at a time. Turning the dial moves through them, pressing the dial goes into a folder or loads a file, B1 goes back up a folder, and B3 closes the page. The page closes by itself once a file is loaded.
- While the page is open the kit takes the dial, the dial's press, B1 and B3, and the app is not told of them. Everything else still reaches the app: the pads go on playing, and the knobs and the fader still work. `draw` is still called, and the page is drawn over what it drew. The lights are the app's as always.
- `isBrowsing()` says whether the page is open, such as to light the button that opened it. `closeBrowser()` closes it.
- The file is loaded by MP Forge between two calls to the app, never inside one. It becomes sample `number`, in place of whatever was there, and `samples().count()` grows to take it in. A sample between two that are filled is empty: `frames()` is 0 and it plays as silence. So ask whether a pad has a file with `samples().at(pad).frames() > 0`, not by counting.
- Keep a sample by its number or by a pointer to it, as `&samples().at(number)`, never as a copy: the pointer stays good when another file is loaded in its place, and a copy does not.
- A file can be refused: it is larger than 16 MB, it is not a WAV or MP3 that can be read, it is an MP3 holding more sound than an app has room for, or there is no room left. The page says so at its foot and stays open. An app holds about twelve minutes of stereo sound in all.
- There may be no card in. The page then says so. An app that needs files has to say on its own screen how to get them, and be worth opening without any.
- A computer has no card. In MP Forge, opening the page has the musician pick one WAV or MP3 file on their computer, and it is loaded as the sample the page was opened for, the same as a file from the card. The page closes once it is loaded, or when they pick none. The app does nothing different for this.
- Nothing is kept. A file picked from the card is gone when the app is closed, and the app starts again with the files the musician added in MP Forge.

## Where sound comes in from

The device takes sound in at the two jacks at the back, at its built-in microphone, and over USB from a computer or a phone. An app listens at one of them, or at none. It starts at none, so an app that works on sound coming in says where to listen as it starts: `hearFrom(sample::SoundIn::LineIn)` in `start`. The musician can only change that on the device itself, so such an app also gives them the kit's page for it:

```cpp
void buttonPressed(sample::Button button) override {
    if (button == sample::Button::Sample) chooseSoundIn();
}

void soundInChosen(sample::SoundIn from) override {
    clearWhatIsStillEchoing();      // what the app still holds came in from somewhere else
}
```

- `chooseSoundIn()` opens the page over the whole screen. It lists `LINE IN`, `MIC`, `USB` and `OFF`, with a block before the one in use. Turning the dial moves through them, pressing the dial takes the one it is on, and B3 closes the page. The page stays open after a choice, so the musician can hear it and choose again.
- While the page is open the kit takes the dial, the dial's press and B3, and the app is not told of them. Everything else still reaches the app, and `draw` is still called, as with the card's page. One of the kit's pages is open at a time: opening one closes any other.
- `soundIn()` says where sound comes in from now. Show it on the app's own screen, in a few capitals such as `IN: USB`, so the musician can see it without opening the page.
- `isChoosingSoundIn()` says whether the page is open, such as to light the button that opened it. `closeSoundInPage()` closes it.
- `hearFrom(sample::SoundIn::Usb)` sets it from the app's own code: once in `start`, to say where the app listens first, and again only in an app that has a way of its own to choose. Use the kit's page for the choosing unless the musician asked for something else.
- An app that takes no sound in never calls `hearFrom`. Its inputs stay off, and in MP Forge the musician's computer is then never listened to.
- With `SoundIn::Off`, `inLeft` and `inRight` hold silence. MP Forge sees to that, so the app reads them as always.
- The app's sound goes out at the jacks, the headphones and USB together, and at the built-in speaker when nothing is plugged into the jacks or the headphones. There is nothing to choose there. With the microphone in use the speaker would be heard by it, so an app that listens at `SoundIn::Mic` should say on its screen to use headphones.
- Nothing is kept: the app starts at the jacks again each time it is opened.

`mp-forge/examples/SampleEcho.cpp` is a whole app that works on sound coming in and has this page.

## MIDI

Whatever is plugged into the device's USB socket or its MIDI IN socket can play the app: a keyboard, a pad controller, a computer running a sequencer. The app is told of both together, on every channel, unless the musician chooses otherwise:

```cpp
void noteOn(int note, float strength, int channel) override {
    startAVoice(note, strength);          // note 60 is middle C; strength runs from just above 0 to 1
}

void noteOff(int note, int channel) override { letGoOfTheVoicePlaying(note); }

void controllerMoved(int controller, float stands, int channel) override {
    if (controller == 1) vibrato_ = stands;      // 1 is the mod wheel; stands runs from 0 to 1
    if (controller == 64) isHeld_ = stands >= 0.5f;    // 64 is the pedal that holds notes
}

void pitchBent(float bend, int channel) override { bend_ = bend; }   // -1 to 1, and 0 where the wheel rests
```

- Notes are numbered 0 to 127 and channels 1 to 16. An app that does not care which channel plays it ignores `channel`, and that is right for most.
- A keyboard that says a note was let go by playing it as softly as can be is understood: the app is told `noteOff`.
- These are called between two calls to `makeSound`, never inside one, like every other touch. Keep them short.
- Keep a fixed number of voices and take the oldest when a note has none left. A keyboard can hold more notes than an app has voices.
- A pad controller's pads send the notes from 36 up. An app whose pads make its sound should let those notes play the pads, as `mp-forge/examples/PocketPads.cpp` does.
- Everything on the device still has to work with nothing plugged in. MIDI adds a way to play the app; it never replaces the pads, the knobs or the fader.
- An app is told of notes, controllers and the pitch wheel, and nothing else of MIDI. It cannot send MIDI, and it is not told a clock to keep time to.
- `chooseMidiIn()` opens the kit's page for the MIDI that plays the app, over the whole screen. It is one list: `USB AND MIDI IN`, `USB`, `MIDI IN` and `OFF`, then `EVERY CHANNEL` and `CHANNEL 1` to `CHANNEL 16`. The source in use and the channel in use each have a block before their name. Turning the dial moves through the list, pressing the dial takes the row it is on, and B3 closes the page. Put it on a button of the app's own, as with the other pages:

```cpp
void buttonPressed(sample::Button button) override {
    if (button == sample::Button::PadBank) chooseMidiIn();
}

void midiInChosen(sample::MidiHeard heard) override { letGoOfEveryVoice(); }   // a held note would never be let go
```

- The kit does the choosing: the app is only told of MIDI from where it listens, on the channel it listens on. `midiIn()` says what that is now, as `from` (`sample::MidiIn::UsbAndSocket`, `Usb`, `Socket` or `Off`) and `channel` (1 to 16, or `sample::kEveryChannel`). `hearMidi({sample::MidiIn::Usb, 10})` sets it from the app's own code.
- `isChoosingMidiIn()` says whether the page is open, such as to light the button that opened it. `closeMidiInPage()` closes it.
- In MP Forge a MIDI keyboard or pad controller plugged into the musician's computer plays the app on the drawing, standing in for both sockets: it is heard whether the app listens at `USB`, at `MIDI IN` or at both, and not when it is `OFF`. The channel counts as on the device.

`card()` is for an app that draws its own way of looking through the card, and only when the musician asks for that. `card().look()` asks for the current folder to be read, `goInto(number)` and `goUp()` move, and `load(number, intoSample)` asks for a file. None of them waits: `isBusy()` is true until MP Forge has answered, and then `count()`, `at(number).name`, `at(number).isFolder`, `folder()`, `isThere()` and `problem()` say what came back. In MP Forge it finds no card, since a computer has none: only the kit's page has a file picked there.

## The screen

- `draw` is called about thirty times a second. Drawing the whole screen every time is the simple way and is fast enough: `screen.clear(colour)`, then the parts.
- `screen.fill(x, y, width, height, colour)` draws a block. `screen.dot(x, y, colour)` draws one dot. Anything outside the screen is left out, so nothing has to be checked.
- `screen.write(words, x, y, colour, size)` writes in the kit's own letters: capitals, digits, and `-`, `+`, `:`, `.`. Small letters come out as capitals and anything else as a gap. A letter is 3 dots wide and 5 tall, times `size`, and the next letter starts 4 dots on, times `size`. So ten letters at size 2 are 78 dots wide.
- Numbers are written with `std::snprintf` into a small array of `char` and then with `screen.write`.
- There are no other fonts, no pictures and no curves. Everything is made of blocks and dots, which suits the device. Draw a knob's place as a bar, a waveform as a row of thin blocks, a step as a square.
- Pick a few colours and keep to them: a dark ground, a light ink for words, a soft ink for labels, one or two accents. `Colour{red, green, blue}`, each from 0 to 255.

## The lights

- `lights.pad(pad, colour)` sets a pad's light. Set all sixteen in `draw`. A dark colour is a pad at rest, a bright one a pad that was hit or is on.
- `lights.glow(Button::Play, isOn)` lights a button or puts it out.
- The lights stay as the app last left them, like the screen, so set them every time `draw` is called.

## What the check does

`check_plugin` reads the code for anything an app may not do, builds it, and then plays it for some seconds: every pad hit at full strength and let go, four pads at once, every button pressed and let go, each knob and the fader to both ends and the middle, a keyboard plugged in and played (three notes, one at a time and then together, the mod wheel and the holding pedal moved, and the pitch wheel to both ends and back), the dial three clicks each way and then pressed once more, with a steady note coming in at the inputs. The app has the sound files the musician added, and a small SD card that is the same every time: a folder `DRUMS` with `kick.wav` and `snare.wav`, a folder `LOOPS` with `bass loop.wav` and `quiet.mp3`, which is silence, and `tone.wav` at the top. `look_at_screen` has the same card, so the kit's page can be looked at: press the button that opens it, then `ENCODER` to go into `DRUMS`. The check blocks an app that crashes or hangs, whose sound holds a value that is not a number, whose sound goes over 1, or that takes too long to make its sound. It shows the screen the app ended on.

`look_at_screen` takes a list of `touches`, done one after another a twentieth of a second apart, and shows the screen half a second after the last. A touch is one of:

```json
{ "control": "pad", "number": 5, "value": 127 }
{ "control": "pad", "number": 5, "value": 0 }
{ "control": "knob", "number": 1, "value": 0.8 }
{ "control": "fader", "value": 0.25 }
{ "control": "encoder", "value": 2 }
{ "control": "button", "name": "PLAY", "value": 1 }
{ "control": "note", "number": 60, "value": 100 }
{ "control": "note", "number": 60, "value": 0 }
{ "control": "controller", "number": 1, "value": 64 }
{ "control": "bend", "value": -0.5 }
```

A pad's `value` is how hard it is hit, from 1 to 127, and 0 lets it go. A button's `name` is the word printed on it: `B1`, `B2`, `B3`, `SAMPLE`, `SEQ`, `PAD FX`, `KNOB FX`, `SHIFT`, `PAD BANK`, `ERASE`, `NOTE REPEAT`, `CHOP`, `MUTE`, `LOOP`, `16 LEVELS`, `SAMPLE SELECT`, `TAP TEMPO`, `MINUS`, `PLUS`, `SAMPLE RECORD`, `SEQ RECORD`, `STOP`, `PLAY`, `ENCODER`. Its `value` is 1 for pressed and 0 for let go. The last four are MIDI from something plugged in: a note's `value` is how hard it is played, from 1 to 127, and 0 lets it go; a controller's runs from 0 to 127; a bend's from -1 to 1. Each is on channel 1 unless it names a `channel`.
