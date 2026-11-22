// Pocket Pads - an MPC Sample app by Workbench Tests
//
// Sixteen tuned pads. Each pad rings a note of its own and lights up as it is hit. K1 tunes them all, K2 sets
// how long a note rings, K3 adds grit, the fader sets how loud it is, and the dial picks the shape of the wave.
// The screen says what each of them does and where it stands. PLAY and STOP run a row of steps along the bottom.
// A MIDI keyboard or pad controller plugged in plays the pads too, and its mod wheel adds grit. PAD BANK opens
// the kit's page for choosing the MIDI that plays them: which socket it comes in at, and on which channel.

#include "mpc_sample_app.h"

#include <cmath>
#include <cstdio>

namespace {

using sample::Button;
using sample::Colour;
using sample::kPads;

const Colour kInk{236, 235, 242};
const Colour kSoft{120, 117, 135};
const Colour kDark{16, 16, 24};
const Colour kWell{38, 37, 48};
const Colour kAccent{181, 39, 31};
const Colour kRowColours[4] = {{240, 96, 48}, {236, 188, 44}, {64, 200, 124}, {64, 140, 240}};

// The notes of the pads, in semitones above the lowest: a five-note scale that sounds right in any order.
const int kSemitones[kPads] = {0, 3, 5, 7, 10, 12, 15, 17, 19, 22, 24, 27, 29, 31, 34, 36};
const char* const kButtonWords[sample::kButtons] = {
    "B1", "B2", "B3", "SAMPLE", "SEQ", "PAD FX", "KNOB FX", "SHIFT", "PAD BANK", "ERASE", "NOTE REPEAT", "CHOP",
    "MUTE", "LOOP", "16 LEVELS", "SAMPLE SELECT", "TAP TEMPO", "MINUS", "PLUS", "SAMPLE RECORD", "SEQ RECORD", "STOP", "PLAY", "DIAL",
};

constexpr float kLowestNote = 110.0f;
constexpr float kTwoPi = 6.2831853f;
constexpr float kMostTuning = 12.0f;
constexpr float kShortestRing = 0.08f;
constexpr float kLongestRing = 2.0f;
constexpr float kMostGrit = 12.0f;
constexpr int kWaves = 4;
const char* const kWaveWords[kWaves] = {"SINE", "TRIANGLE", "SQUARE", "SAW"};
constexpr float kQuietEnoughToStop = 0.0005f;
constexpr float kGlowFadesIn = 0.6f;
// The note a pad controller's first pad sends. The sixteen notes from there play the sixteen pads, and so do
// the same notes in every other octave of a keyboard.
constexpr int kNoteOfTheFirstPad = 36;
constexpr int kModWheel = 1;
constexpr int kSteps = 16;
constexpr float kStepsASecond = 8.0f;

constexpr int kTile = 40, kTileGap = 6, kTilesLeft = 10, kTilesTop = 24;
constexpr int kPanelLeft = 204, kPanelWidth = 108, kRowHeight = 27, kRowsTop = 24, kBarHeight = 7;

Colour between(Colour from, Colour to, float howFar) {
    const auto part = [howFar](uint8_t a, uint8_t b) { return static_cast<uint8_t>(std::lround(a + (b - a) * howFar)); };
    return {part(from.red, to.red), part(from.green, to.green), part(from.blue, to.blue)};
}

Colour colourOfPad(int index, float glow) {
    const Colour bright = kRowColours[index / 4];
    return between(between(kDark, bright, 0.28f), between(bright, kInk, 0.35f), glow);
}

struct Voice {
    float phase = 0.0f;
    float level = 0.0f;
};

}  // namespace

class PocketPads : public sample::App {
public:
    void start(double framesASecond) override { framesASecond_ = static_cast<float>(framesASecond); }

    void padHit(int pad, float strength) override {
        voices_[pad - 1] = Voice{0.0f, strength};
        glows_[pad - 1] = strength;
    }

    void noteOn(int note, float strength, int /*channel*/) override { padHit(padPlayedBy(note), strength); }

    void controllerMoved(int controller, float stands, int /*channel*/) override {
        if (controller == kModWheel) knobs_[2] = stands;
    }

    void knobTurned(int knob, float stands) override { knobs_[knob - 1] = stands; }
    void faderMoved(float stands) override { fader_ = stands; }
    void encoderTurned(int clicks) override { wave_ = ((wave_ + clicks) % kWaves + kWaves) % kWaves; }

    void buttonPressed(Button button) override {
        lastPressed_ = static_cast<int>(button);
        if (button == Button::Play) isPlaying_ = true;
        if (button == Button::Stop) isPlaying_ = false;
        if (button == Button::PadBank) chooseMidiIn();
    }

    void makeSound(sample::Sound& sound) override {
        const float fadeAFrame = fadeOfARingingNote();
        const float grit = 1.0f + knobs_[2] * (kMostGrit - 1.0f);
        for (int frame = 0; frame < sound.frames; ++frame) {
            const float heard = std::tanh(allVoices(fadeAFrame) * grit) / std::tanh(grit) * fader_;
            sound.outLeft[frame] = heard;
            sound.outRight[frame] = heard;
        }
        letTimePass(static_cast<float>(sound.frames) / framesASecond_);
    }

    void draw(sample::Screen& screen, sample::Lights& lights) override {
        screen.clear(kDark);
        drawTitle(screen);
        drawPads(screen, lights);
        for (int knob = 0; knob < sample::kKnobs; ++knob) drawWhereItStands(screen, knob, kKnobWords[knob], knobs_[knob]);
        drawWhereItStands(screen, sample::kKnobs, "FADER VOL", fader_);
        drawWhatWasTouched(screen);
        drawSteps(screen);
        lights.glow(Button::Play, isPlaying_);
        lights.glow(Button::PadBank, isChoosingMidiIn());
    }

private:
    static constexpr const char* kKnobWords[sample::kKnobs] = {"K1 TUNE", "K2 RING", "K3 GRIT"};

    static int padPlayedBy(int note) { return ((note - kNoteOfTheFirstPad) % kPads + kPads) % kPads + 1; }

    float frequencyOf(int pad) const {
        const float tuning = (knobs_[0] - 0.5f) * 2.0f * kMostTuning;
        return kLowestNote * std::pow(2.0f, (static_cast<float>(kSemitones[pad]) + tuning) / 12.0f);
    }

    // What a ringing note is multiplied by each frame, so that it has died away after as long as K2 says.
    float fadeOfARingingNote() const {
        const float rings = kShortestRing + knobs_[1] * (kLongestRing - kShortestRing);
        return std::pow(kQuietEnoughToStop, 1.0f / (rings * framesASecond_));
    }

    float allVoices(float fadeAFrame) {
        float heard = 0.0f;
        for (int pad = 0; pad < kPads; ++pad) {
            Voice& voice = voices_[pad];
            if (voice.level < kQuietEnoughToStop) continue;
            heard += waveAt(voice.phase) * voice.level;
            voice.phase = std::fmod(voice.phase + kTwoPi * frequencyOf(pad) / framesASecond_, kTwoPi);
            voice.level *= fadeAFrame;
        }
        return heard * 0.5f;
    }

    // One turn of the wave the dial picked, from -1 to 1, at `phase` of the way round in radians.
    float waveAt(float phase) const {
        const float turn = phase / kTwoPi;
        switch (wave_) {
            case 1: return 1.0f - 4.0f * std::fabs(turn - 0.5f);
            case 2: return turn < 0.5f ? 0.6f : -0.6f;
            case 3: return (2.0f * turn - 1.0f) * 0.7f;
            default: return std::sin(phase);
        }
    }

    void letTimePass(float seconds) {
        for (float& glow : glows_) glow = std::max(0.0f, glow - seconds / kGlowFadesIn);
        if (isPlaying_) playedFor_ += seconds;
    }

    void drawTitle(sample::Screen& screen) const {
        screen.fill(0, 0, sample::kScreenWidth, 16, kAccent);
        screen.write("POCKET PADS", 6, 3, kInk, 2);
        screen.write(isPlaying_ ? "PLAY" : "STOP", 280, 3, kInk, 2);
    }

    void drawPads(sample::Screen& screen, sample::Lights& lights) const {
        for (int pad = 0; pad < kPads; ++pad) {
            const Colour colour = colourOfPad(pad, glows_[pad]);
            const int x = kTilesLeft + (pad % 4) * (kTile + kTileGap);
            const int y = kTilesTop + (3 - pad / 4) * (kTile + kTileGap);
            char number[4];
            std::snprintf(number, sizeof number, "%d", pad + 1);
            screen.fill(x, y, kTile, kTile, colour);
            screen.write(number, x + 3, y + 3, kDark);
            lights.pad(pad + 1, colour);
        }
    }

    // One row of the panel: what a control does, and under it a bar filled as far as the control stands.
    void drawWhereItStands(sample::Screen& screen, int row, const char* words, float stands) const {
        const int y = kRowsTop + row * kRowHeight;
        screen.write(words, kPanelLeft, y, kSoft, 2);
        screen.fill(kPanelLeft, y + 13, kPanelWidth, kBarHeight, kWell);
        screen.fill(kPanelLeft, y + 13, static_cast<int>(std::lround(kPanelWidth * stands)), kBarHeight, kInk);
    }

    void drawWhatWasTouched(sample::Screen& screen) const {
        const int y = kRowsTop + (sample::kKnobs + 1) * kRowHeight;
        screen.write("DIAL WAVE", kPanelLeft, y, kSoft, 2);
        screen.write(kWaveWords[wave_], kPanelLeft, y + 13, kInk, 2);
        if (lastPressed_ >= 0) screen.write(kButtonWords[lastPressed_], kPanelLeft, y + 32, kSoft, 2);
    }

    void drawSteps(sample::Screen& screen) const {
        const int running = static_cast<int>(playedFor_ * kStepsASecond) % kSteps;
        for (int step = 0; step < kSteps; ++step) {
            const bool isOn = isPlaying_ && step == running;
            screen.fill(10 + step * 19, 216, 15, 12, isOn ? kAccent : (step % 4 == 0 ? kSoft : kWell));
        }
    }

    float framesASecond_ = 44100.0f;
    Voice voices_[kPads];
    float glows_[kPads] = {};
    float knobs_[sample::kKnobs] = {0.5f, 0.5f, 0.5f};
    float fader_ = 0.75f;
    int wave_ = 0;
    int lastPressed_ = -1;
    bool isPlaying_ = false;
    float playedFor_ = 0.0f;
};

MPC_SAMPLE_APP(PocketPads)
