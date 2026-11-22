// Sample Pads - an MPC Sample app by Workbench Tests
//
// Plays the sound files the musician added, one to a pad, in the order of their names. K1 tunes them all, K2
// moves where in its file a pad starts playing, and the fader sets how loud it is. The screen names the file on
// each pad and draws the shape of the last one hit, with a line that runs along it as it plays.
//
// SAMPLE SELECT opens the kit's page for picking a file from the SD card, which is loaded onto the pad that was
// hit last. With no sound files at all, each pad plays a short tone, and the screen says how to give a pad one.

#include "mpc_sample_app.h"

#include <cmath>
#include <cstdio>

namespace {

using sample::Button;
using sample::Colour;
using sample::kPads;
using sample::Sample;

const Colour kInk{236, 235, 242};
const Colour kSoft{120, 117, 135};
const Colour kDark{16, 16, 24};
const Colour kWell{38, 37, 48};
const Colour kAccent{48, 150, 220};

constexpr float kMostTuning = 12.0f;
// A voice comes in over this many frames, so that a file that starts loud never clicks.
constexpr int kFramesToComeIn = 96;
// Sound up to here is left as it is. Above it, it is bent to stay under full.
constexpr float kLoudestLeftAlone = 0.7f;
constexpr float kTwoPi = 6.2831853f;
constexpr float kLowestTone = 220.0f;
constexpr float kToneKeptAFrame = 0.9996f;
constexpr float kQuietEnoughToStop = 0.0005f;
constexpr float kGlowFadesIn = 0.4f;
constexpr int kLettersOnATile = 9;
constexpr int kReadsAColumn = 32;

constexpr int kTile = 40, kTileGap = 6, kTilesLeft = 10, kTilesTop = 24;
constexpr int kPanelLeft = 204, kPanelWidth = 108, kRowHeight = 27, kRowsTop = 24, kBarHeight = 7;
constexpr int kShapeTop = 112, kShapeHeight = 56, kWordsTop = 176;

// One pad's file as it plays: how far along it is, how fast it moves, and how hard the pad was hit.
struct Voice {
    const Sample* playing = nullptr;
    double place = 0.0;
    double step = 1.0;
    float level = 0.0f;
    int framesPlayed = 0;
};

// The tone a pad plays while the musician has added no sound files.
struct Tone {
    float phase = 0.0f;
    float level = 0.0f;
};

struct Heard {
    float left = 0.0f;
    float right = 0.0f;
};

Colour between(Colour from, Colour to, float howFar) {
    const auto part = [howFar](uint8_t a, uint8_t b) { return static_cast<uint8_t>(std::lround(a + (b - a) * howFar)); };
    return {part(from.red, to.red), part(from.green, to.green), part(from.blue, to.blue)};
}

float heldUnderFull(float mixed) {
    const float size = std::fabs(mixed);
    if (size <= kLoudestLeftAlone) return mixed;
    const float room = 1.0f - kLoudestLeftAlone;
    const float over = size - kLoudestLeftAlone;
    return std::copysign(kLoudestLeftAlone + room * over / (over + room), mixed);
}

}  // namespace

class SamplePads : public sample::App {
public:
    void start(double framesASecond) override {
        framesASecond_ = static_cast<float>(framesASecond);
        for (int pad = 0; pad < kPads; ++pad) measureShapeOf(pad);
    }

    // A file from the card has taken the place of a pad's file: whatever was playing there stops.
    void sampleLoaded(int number) override {
        if (number >= kPads) return;
        voices_[number] = Voice{};
        measureShapeOf(number);
    }

    void buttonPressed(Button button) override {
        if (button == Button::SampleSelect) browseCard(lastHit_);
    }

    void padHit(int pad, float strength) override {
        glows_[pad - 1] = strength;
        lastHit_ = pad - 1;
        if (samples().count() == 0) tones_[pad - 1] = Tone{0.0f, strength * 0.5f};
        if (!hasAFile(pad - 1)) return;
        const Sample& hit = samples().at(pad - 1);
        voices_[pad - 1] = Voice{&hit, knobs_[1] * hit.frames(), stepOfATunedFile(), strength, 0};
    }

    void knobTurned(int knob, float stands) override { knobs_[knob - 1] = stands; }
    void faderMoved(float stands) override { fader_ = stands; }

    void makeSound(sample::Sound& sound) override {
        for (int frame = 0; frame < sound.frames; ++frame) {
            const Heard heard = allVoices();
            const float tone = allTones();
            sound.outLeft[frame] = heldUnderFull((heard.left + tone) * fader_);
            sound.outRight[frame] = heldUnderFull((heard.right + tone) * fader_);
        }
        const float seconds = static_cast<float>(sound.frames) / framesASecond_;
        for (float& glow : glows_) glow = std::max(0.0f, glow - seconds / kGlowFadesIn);
    }

    void draw(sample::Screen& screen, sample::Lights& lights) override {
        screen.clear(kDark);
        screen.fill(0, 0, sample::kScreenWidth, 16, kAccent);
        screen.write("SAMPLE PADS", 6, 3, kInk, 2);
        drawPads(screen, lights);
        for (int row = 0; row < kRows; ++row) drawWhereItStands(screen, row);
        if (hasAFile(lastHit_)) drawTheLastOneHit(screen); else drawHowToGiveItAFile(screen);
        lights.glow(Button::SampleSelect, isBrowsing());
    }

private:
    static constexpr int kRows = 3;
    static constexpr const char* kRowWords[kRows] = {"K1 TUNE", "K2 START", "FADER VOL"};

    // The first file is on pad 1, the second on pad 2. A pad past the last file has none until one is loaded
    // onto it from the card.
    bool hasAFile(int pad) const { return samples().at(pad).frames() > 0; }

    // How far along its file a voice moves each frame: 1 plays it as recorded, 2 an octave up.
    double stepOfATunedFile() const { return std::pow(2.0, (knobs_[0] - 0.5) * 2.0 * kMostTuning / 12.0); }

    Heard allVoices() {
        Heard heard;
        for (Voice& voice : voices_) {
            if (voice.playing == nullptr) continue;
            const float level = voice.level * std::min(1.0f, static_cast<float>(voice.framesPlayed) / kFramesToComeIn);
            heard.left += voice.playing->leftAt(voice.place) * level;
            heard.right += voice.playing->rightAt(voice.place) * level;
            voice.place += voice.step;
            voice.framesPlayed += 1;
            if (voice.place >= voice.playing->frames()) voice.playing = nullptr;
        }
        return heard;
    }

    float allTones() {
        float heard = 0.0f;
        for (int pad = 0; pad < kPads; ++pad) {
            Tone& tone = tones_[pad];
            if (tone.level < kQuietEnoughToStop) continue;
            heard += std::sin(tone.phase) * tone.level;
            tone.phase = std::fmod(tone.phase + kTwoPi * kLowestTone * (1.0f + pad / 4.0f) / framesASecond_, kTwoPi);
            tone.level *= kToneKeptAFrame;
        }
        return heard;
    }

    // The shape of a file, as how loud it gets in each column the screen draws it in. Worked out once, when the
    // app starts, from a few frames of each column so that a long file takes no longer than a short one.
    void measureShapeOf(int pad) {
        const Sample& measured = samples().at(pad);
        const int framesAColumn = std::max(1, measured.frames() / kPanelWidth);
        const int framesAReading = std::max(1, framesAColumn / kReadsAColumn);
        for (int column = 0; column < kPanelWidth; ++column) {
            float loudest = 0.0f;
            for (int frame = column * framesAColumn; frame < (column + 1) * framesAColumn; frame += framesAReading)
                loudest = std::max(loudest, std::fabs(measured.left(frame)));
            shapes_[pad][column] = std::min(1.0f, loudest);
        }
    }

    void drawPads(sample::Screen& screen, sample::Lights& lights) const {
        for (int pad = 0; pad < kPads; ++pad) {
            const Colour atRest = hasAFile(pad) ? between(kDark, kAccent, 0.45f) : kWell;
            const Colour colour = between(atRest, kInk, glows_[pad] * 0.7f);
            const int x = kTilesLeft + (pad % 4) * (kTile + kTileGap);
            const int y = kTilesTop + (3 - pad / 4) * (kTile + kTileGap);
            char number[4];
            std::snprintf(number, sizeof number, "%d", pad + 1);
            screen.fill(x, y, kTile, kTile, colour);
            screen.write(number, x + 3, y + 3, kDark, 2);
            if (hasAFile(pad)) screen.write(shortened(samples().at(pad).name(), kLettersOnATile).words, x + 3, y + 30, kDark);
            lights.pad(pad + 1, colour);
        }
    }

    // One row of the panel: what a control does, and under it a bar filled as far as the control stands.
    void drawWhereItStands(sample::Screen& screen, int row) const {
        const float stands[kRows] = {knobs_[0], knobs_[1], fader_};
        const int y = kRowsTop + row * kRowHeight;
        screen.write(kRowWords[row], kPanelLeft, y, kSoft, 2);
        screen.fill(kPanelLeft, y + 13, kPanelWidth, kBarHeight, kWell);
        screen.fill(kPanelLeft, y + 13, static_cast<int>(std::lround(kPanelWidth * stands[row])), kBarHeight, kInk);
    }

    void drawHowToGiveItAFile(sample::Screen& screen) const {
        char pad[8];
        std::snprintf(pad, sizeof pad, "PAD %d", lastHit_ + 1);
        screen.write(pad, kPanelLeft, kShapeTop, kInk, 2);
        screen.write("HAS NO FILE", kPanelLeft, kShapeTop + 14, kInk, 2);
        screen.write("SAMPLE SELECT PICKS ONE", kPanelLeft, kShapeTop + 34, kSoft);
        screen.write("FROM THE SD CARD. OR ADD", kPanelLeft, kShapeTop + 44, kSoft);
        screen.write("FILES WITH + IN MP FORGE.", kPanelLeft, kShapeTop + 54, kSoft);
    }

    void drawTheLastOneHit(sample::Screen& screen) const {
        const int shown = lastHit_;
        const Sample& file = samples().at(shown);
        screen.fill(kPanelLeft, kShapeTop, kPanelWidth, kShapeHeight, kWell);
        for (int column = 0; column < kPanelWidth; ++column) {
            const int tall = std::max(1, static_cast<int>(shapes_[shown][column] * kShapeHeight));
            screen.fill(kPanelLeft + column, kShapeTop + (kShapeHeight - tall) / 2, 1, tall, kAccent);
        }
        const Voice& voice = voices_[shown];
        if (voice.playing != nullptr) screen.fill(kPanelLeft + static_cast<int>(voice.place / file.frames() * kPanelWidth), kShapeTop, 1, kShapeHeight, kInk);
        screen.write(shortened(file.name(), kPanelWidth / sample::Screen::widthOfALetter(2)).words, kPanelLeft, kWordsTop, kInk, 2);
        char length[24];
        std::snprintf(length, sizeof length, "%.1f S %s", file.frames() / framesASecond_, file.isStereo() ? "STEREO" : "MONO");
        screen.write(length, kPanelLeft, kWordsTop + 14, kSoft);
    }

    // A name cut to as many letters as there is room for.
    struct Shortened {
        char words[sample::kLongestSampleName + 1];
    };

    static Shortened shortened(const char* name, int letters) {
        Shortened cut{};
        std::snprintf(cut.words, sizeof cut.words, "%.*s", letters, name);
        return cut;
    }

    float framesASecond_ = 44100.0f;
    Voice voices_[kPads];
    Tone tones_[kPads];
    float shapes_[kPads][kPanelWidth] = {};
    float glows_[kPads] = {};
    float knobs_[sample::kKnobs] = {0.5f, 0.0f, 0.5f};
    float fader_ = 0.75f;
    int lastHit_ = 0;
};

MPC_SAMPLE_APP(SamplePads)
