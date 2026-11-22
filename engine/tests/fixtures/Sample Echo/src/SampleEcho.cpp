// Sample Echo - an MPC Sample app by Workbench Tests
//
// An echo on whatever comes in at the device's inputs. K1 sets how long the echo takes to come back, K2 how
// many times it repeats, and the fader how loud the echoes are beside the sound itself. The screen shows where
// each of them stands, how loud the sound coming in is, and where it comes in from.
//
// It listens at the jacks at the back from the start. SAMPLE opens the kit's page for choosing where sound
// comes in from: the jacks, the built-in microphone, USB, or nowhere.

#include "mpc_sample_app.h"

#include <cmath>
#include <cstdio>

namespace {

using sample::Button;
using sample::Colour;
using sample::SoundIn;

const Colour kInk{236, 235, 242};
const Colour kSoft{120, 117, 135};
const Colour kDark{16, 16, 24};
const Colour kWell{38, 37, 48};
const Colour kAccent{52, 168, 132};

constexpr float kShortestEcho = 0.03f;
constexpr float kLongestEcho = 0.75f;
constexpr float kMostRepeats = 0.85f;
// Room for the longest echo at the highest rate the device runs at.
constexpr int kMostFramesASecond = 96000;
constexpr int kMostEchoFrames = static_cast<int>(kLongestEcho * kMostFramesASecond) + 1;
// Sound up to here is left as it is. Above it, it is bent to stay under full.
constexpr float kLoudestLeftAlone = 0.7f;
// The meter falls back this much each frame, so a loud moment can be seen before it is gone.
constexpr float kMeterKeptAFrame = 0.9995f;

constexpr int kLeft = 12, kWidth = 296, kRowsTop = 28, kRowHeight = 34, kBarHeight = 10;
constexpr int kMeterTop = 140, kWordsTop = 176;

float heldUnderFull(float mixed) {
    const float size = std::fabs(mixed);
    if (size <= kLoudestLeftAlone) return mixed;
    const float room = 1.0f - kLoudestLeftAlone;
    const float over = size - kLoudestLeftAlone;
    return std::copysign(kLoudestLeftAlone + room * over / (over + room), mixed);
}

const char* wordsFor(SoundIn from) {
    if (from == SoundIn::Mic) return "IN: MIC";
    if (from == SoundIn::Usb) return "IN: USB";
    return from == SoundIn::Off ? "IN: OFF" : "IN: LINE IN";
}

}  // namespace

class SampleEcho : public sample::App {
public:
    // An app hears nothing until it says where to listen.
    void start(double framesASecond) override {
        framesASecond_ = std::min(static_cast<float>(framesASecond), static_cast<float>(kMostFramesASecond));
        hearFrom(SoundIn::LineIn);
    }

    void buttonPressed(Button button) override {
        if (button == Button::Sample) chooseSoundIn();
    }

    // What was still echoing came in from somewhere else, so it is let go.
    void soundInChosen(SoundIn) override {
        for (float& kept : keptLeft_) kept = 0.0f;
        for (float& kept : keptRight_) kept = 0.0f;
    }

    void knobTurned(int knob, float stands) override { knobs_[knob - 1] = stands; }
    void faderMoved(float stands) override { fader_ = stands; }

    void makeSound(sample::Sound& sound) override {
        const int echoFrames = framesOfTheEcho();
        const float repeats = knobs_[1] * kMostRepeats;
        for (int frame = 0; frame < sound.frames; ++frame) {
            const int comesBack = (writing_ + kMostEchoFrames - echoFrames) % kMostEchoFrames;
            const float echoLeft = keptLeft_[comesBack], echoRight = keptRight_[comesBack];
            keptLeft_[writing_] = sound.inLeft[frame] + echoLeft * repeats;
            keptRight_[writing_] = sound.inRight[frame] + echoRight * repeats;
            writing_ = (writing_ + 1) % kMostEchoFrames;
            sound.outLeft[frame] = heldUnderFull(sound.inLeft[frame] + echoLeft * fader_);
            sound.outRight[frame] = heldUnderFull(sound.inRight[frame] + echoRight * fader_);
            comingIn_ = std::max(comingIn_ * kMeterKeptAFrame, std::max(std::fabs(sound.inLeft[frame]), std::fabs(sound.inRight[frame])));
        }
    }

    void draw(sample::Screen& screen, sample::Lights& lights) override {
        screen.clear(kDark);
        screen.fill(0, 0, sample::kScreenWidth, 16, kAccent);
        screen.write("SAMPLE ECHO", 6, 3, kInk, 2);
        const float stands[kRows] = {knobs_[0], knobs_[1], fader_};
        for (int row = 0; row < kRows; ++row) drawBar(screen, kRowsTop + row * kRowHeight, kRowWords[row], stands[row]);
        drawBar(screen, kMeterTop, "COMING IN", std::min(1.0f, comingIn_));
        screen.write(wordsFor(soundIn()), kLeft, kWordsTop, kInk, 2);
        screen.write("SAMPLE: CHOOSE WHERE SOUND COMES IN FROM", kLeft, kWordsTop + 18, kSoft);
        lights.glow(Button::Sample, isChoosingSoundIn());
    }

private:
    static constexpr int kRows = 3;
    static constexpr const char* kRowWords[kRows] = {"K1 TIME", "K2 REPEATS", "FADER ECHO LEVEL"};

    int framesOfTheEcho() const {
        const float seconds = kShortestEcho + knobs_[0] * (kLongestEcho - kShortestEcho);
        return std::max(1, static_cast<int>(seconds * framesASecond_));
    }

    // What a control does, and under it a bar filled as far as the control stands.
    void drawBar(sample::Screen& screen, int y, const char* words, float stands) const {
        screen.write(words, kLeft, y, kSoft, 2);
        screen.fill(kLeft, y + 14, kWidth, kBarHeight, kWell);
        screen.fill(kLeft, y + 14, static_cast<int>(std::lround(kWidth * stands)), kBarHeight, kInk);
    }

    float framesASecond_ = 44100.0f;
    float keptLeft_[kMostEchoFrames] = {};
    float keptRight_[kMostEchoFrames] = {};
    int writing_ = 0;
    float knobs_[sample::kKnobs] = {0.4f, 0.4f, 0.5f};
    float fader_ = 0.5f;
    float comingIn_ = 0.0f;
};

MPC_SAMPLE_APP(SampleEcho)
