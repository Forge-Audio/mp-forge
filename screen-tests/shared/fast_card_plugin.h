// The plugin behind the "Draw Fast" screen test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME (parameter 0, smoothed, no side effects) -> output.
//
// What it tests: how often a second the MPC's screen can be given a new picture before more stops looking
// smoother. Four display parameters all follow the same movement, a glide from 0 to 100 and back every two
// seconds. They differ only in how often the plugin writes the movement down and tells the MPC:
//
//   SWEEP 30    30 times a second
//   SWEEP 45    45 times a second
//   SWEEP 60    60 times a second
//   SWEEP OWN   as often as OWN SPEED says (the one control you turn, 1 to 240 times a second)
//
// Every sweep reads the same clock, so the four are always at the same place in the movement and only their
// smoothness can differ.
//
// Real-time rules: nothing is allocated, locked, logged or read in process(). One known exception is part
// of the test: the MPC is told about each change from process(), on the audio thread. Akai's framework
// advises against that where avoidable; other MPC plugins do exactly this on real hardware.

#pragma once

#include "mpc_plugin.h"

#include <cmath>

namespace screen_test {

class FastCardPlugin : public mpc::Plugin
{
public:
    FastCardPlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId))
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        addParam(mpc::intParam("ownspeed", "Own Speed", kSlowestOwnSpeed, kFastestOwnSpeed, 120, "fps"));
        addParam(mpc::floatParam("sweep30", "Sweep 30", 0.0f, 100.0f, 0.0f, "", 0));
        addParam(mpc::floatParam("sweep45", "Sweep 45", 0.0f, 100.0f, 0.0f, "", 0));
        addParam(mpc::floatParam("sweep60", "Sweep 60", 0.0f, 100.0f, 0.0f, "", 0));
        addParam(mpc::floatParam("sweepown", "Sweep Own", 0.0f, 100.0f, 0.0f, "", 0));
    }

    void prepare(double sampleRate, int) override
    {
        sampleRate_ = sampleRate;
        reset();
    }

    void reset() override
    {
        volume_ = targetVolume();
        for (double& samples : samplesUntilNextPicture_)
            samples = 0.0;
    }

    void process(mpc::ProcessContext& ctx) override
    {
        passSoundThrough(ctx);
        samplesPlayed_ += ctx.numFrames;
        for (int sweep = 0; sweep < kSweeps; ++sweep)
            showWhenDue(sweep, ctx.numFrames);
    }

private:
    enum ParamIndex { kVolume = 0, kOwnSpeed, kFirstSweep };

    static constexpr int    kSweeps          = 4;
    static constexpr int    kOwnSweep        = 3;
    static constexpr int    kSlowestOwnSpeed = 1;
    static constexpr int    kFastestOwnSpeed = 240;
    static constexpr int    kMaxChannels     = 2;
    static constexpr float  kSmoothingRate   = 0.002f;
    static constexpr float  kPercentToFactor = 0.01f;
    static constexpr double kSweepSeconds    = 2.0;   // there and back

    static mpc::PluginInfo infoFor(const char* name, uint32_t uniqueId)
    {
        mpc::PluginInfo info;
        info.name         = name;
        info.vendor       = "Workbench Tests";   // must match plugin.json "manufacturer"
        info.uniqueId     = uniqueId;            // must match plugin.json "uid"
        info.version      = 1000;
        info.isInstrument = false;
        info.numInputs    = 2;
        info.numOutputs   = 2;
        info.tailSamples  = 1;                   // no tail
        return info;
    }

    float targetVolume() const { return get(kVolume) * kPercentToFactor; }

    void passSoundThrough(mpc::ProcessContext& ctx)
    {
        const int   numChannels = mpc::clamp(ctx.numOutputs, 0, kMaxChannels);
        const float target      = targetVolume();

        for (int frame = 0; frame < ctx.numFrames; ++frame)
        {
            volume_ += kSmoothingRate * (target - volume_);
            for (int channel = 0; channel < numChannels; ++channel)
                ctx.outputs[channel][frame] = inputSample(ctx, channel, frame) * volume_;
        }
    }

    // Inputs can be absent: treat a missing one as silence.
    static float inputSample(const mpc::ProcessContext& ctx, int channel, int frame)
    {
        const bool present = ctx.inputs != nullptr && channel < ctx.numInputs && ctx.inputs[channel] != nullptr;
        return present ? ctx.inputs[channel][frame] : 0.0f;
    }

    double picturesASecond(int sweep) const
    {
        static const double fixedSpeeds[kOwnSweep] = { 30.0, 45.0, 60.0 };
        return sweep == kOwnSweep ? (double)get(kOwnSpeed) : fixedSpeeds[sweep];
    }

    // Each sweep keeps its own time to its next picture, and shows where the shared movement is by then.
    void showWhenDue(int sweep, int numFrames)
    {
        samplesUntilNextPicture_[sweep] -= numFrames;
        if (samplesUntilNextPicture_[sweep] > 0.0)
            return;
        samplesUntilNextPicture_[sweep] += sampleRate_ / picturesASecond(sweep);
        show(kFirstSweep + sweep, sweepAt(samplesPlayed_ / sampleRate_));
    }

    // A triangle: 0 up to 100 and back down, once every kSweepSeconds.
    static float sweepAt(double seconds)
    {
        const double throughTheSweep = std::fmod(seconds, kSweepSeconds) / kSweepSeconds;
        return (float)(100.0 * (1.0 - std::fabs(2.0 * throughTheSweep - 1.0)));
    }

    // Writes a display parameter and tells the MPC.
    void show(int index, float value)
    {
        set(index, value);
        notifyHostParameterChanged(index);
    }

    double sampleRate_    = 44100.0;
    double samplesPlayed_ = 0.0;
    double samplesUntilNextPicture_[kSweeps] = { 0.0, 0.0, 0.0, 0.0 };
    float  volume_        = 1.0f;
};

} // namespace screen_test
