// A new app for the MPC Sample, as MP Forge starts one: every pad rings a note, and the pad that was hit lights
// up. This is where the app is written. The kit, mpc_sample_app.h, says everything an app can do.

#include "mpc_sample_app.h"

#include <cmath>

namespace {

using sample::Colour;

const Colour kDark{16, 16, 24};
const Colour kInk{236, 235, 242};
const Colour kAccent{181, 39, 31};
const Colour kPadAtRest{40, 60, 110};
const Colour kPadHit{120, 170, 255};

const double kTwoPi = 6.283185307179586;
const float kLowestNote = 110.0f;
const float kSecondsANoteRings = 0.4f;
const float kLoudness = 0.3f;
const float kTooQuietToShow = 0.05f;

}  // namespace

class StarterApp : public sample::App {
public:
    void start(double framesASecond) override {
        framesASecond_ = framesASecond;
        fadeAFrame_ = static_cast<float>(std::exp(-1.0 / (kSecondsANoteRings * framesASecond)));
    }

    void padHit(int pad, float strength) override {
        padLastHit_ = pad;
        turnAFrame_ = kTwoPi * kLowestNote * std::pow(2.0, (pad - 1) / 12.0) / framesASecond_;
        level_ = strength;
    }

    void makeSound(sample::Sound& sound) override {
        for (int frame = 0; frame < sound.frames; ++frame) {
            const float heard = static_cast<float>(std::sin(turned_)) * level_ * kLoudness;
            sound.outLeft[frame] = heard;
            sound.outRight[frame] = heard;
            turned_ = std::fmod(turned_ + turnAFrame_, kTwoPi);
            level_ *= fadeAFrame_;
        }
    }

    void draw(sample::Screen& screen, sample::Lights& lights) override {
        screen.clear(kDark);
        screen.fill(0, 0, sample::kScreenWidth, 16, kAccent);
        screen.write("@NAME@", 6, 3, kInk, 2);
        screen.write("HIT A PAD", 6, 40, kInk, 3);
        for (int pad = 1; pad <= sample::kPads; ++pad) lights.pad(pad, isRinging(pad) ? kPadHit : kPadAtRest);
    }

private:
    bool isRinging(int pad) const { return pad == padLastHit_ && level_ > kTooQuietToShow; }

    double framesASecond_ = 44100.0;
    double turnAFrame_ = 0.0;
    double turned_ = 0.0;
    float fadeAFrame_ = 0.0f;
    float level_ = 0.0f;
    int padLastHit_ = 0;
};

MPC_SAMPLE_APP(StarterApp)
