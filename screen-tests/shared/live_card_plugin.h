// The plugin behind the "Draw Live" screen test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME (parameter 0, smoothed, no side effects) -> output.
//
// What it tests: whether pictures on the MPC screen can move by themselves. Several times a second the
// plugin writes new values into its own display parameters and tells the MPC they changed:
//
//   SPEED   how many of those updates happen each second (the one control you turn)
//   SWEEP   glides from 0 to 100 and back, on its own
//   LEVEL   follows the loudest sample that passed through, falling back slowly
//   WAVE 1 to WAVE 24   the heights of a sine wave that travels sideways
//
// Real-time rules: nothing is allocated, locked, logged or read in process(). One known exception is part
// of the test: the MPC is told about each change from process(), on the audio thread. Akai's framework
// advises against that where avoidable; other MPC plugins do exactly this on real hardware.

#pragma once

#include "mpc_plugin.h"

#include <cmath>
#include <string>

namespace screen_test {

class LiveCardPlugin : public mpc::Plugin
{
public:
    LiveCardPlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId))
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        addParam(mpc::intParam("speed", "Speed", kSlowestFramesPerSecond, kFastestFramesPerSecond, 15, "fps"));
        addParam(mpc::floatParam("sweep", "Sweep", 0.0f, 100.0f, 0.0f, "", 0));
        addParam(mpc::floatParam("level", "Level", 0.0f, 100.0f, 0.0f, "", 0));
        for (int column = 1; column <= kWaveColumns; ++column)
            addWaveColumn(column);
    }

    void prepare(double sampleRate, int) override
    {
        sampleRate_ = sampleRate;
        reset();
    }

    void reset() override
    {
        volume_               = targetVolume();
        samplesUntilNextFrame_ = 0.0;
    }

    void process(mpc::ProcessContext& ctx) override
    {
        passSoundThrough(ctx);
        samplesUntilNextFrame_ -= ctx.numFrames;
        if (samplesUntilNextFrame_ > 0.0)
            return;
        const double framesPerSecond = get(kSpeed);
        samplesUntilNextFrame_ += sampleRate_ / framesPerSecond;
        showNextFrame(1.0 / framesPerSecond);
    }

private:
    enum ParamIndex { kVolume = 0, kSpeed, kSweep, kLevel, kFirstWaveColumn };

    static constexpr int    kWaveColumns            = 24;
    static constexpr int    kSlowestFramesPerSecond = 1;
    static constexpr int    kFastestFramesPerSecond = 30;
    static constexpr int    kMaxChannels            = 2;
    static constexpr float  kSmoothingRate          = 0.002f;
    static constexpr float  kPercentToFactor        = 0.01f;
    static constexpr float  kSmallestShownChange    = 0.25f;   // in the 0..100 range of a display parameter
    static constexpr float  kLevelFallPerFrame      = 0.85f;
    static constexpr double kSweepSeconds           = 4.0;     // there and back
    static constexpr double kWaveTurnsPerSecond     = 0.5;
    static constexpr double kWavesAcross            = 1.5;
    static constexpr double kTwoPi                  = 6.283185307179586;

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

    void addWaveColumn(int column)
    {
        const std::string id   = "wave" + std::to_string(column);
        const std::string name = "Wave " + std::to_string(column);
        addParam(mpc::floatParam(id.c_str(), name.c_str(), 0.0f, 100.0f, 50.0f, "", 0));
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
            {
                const float sample = inputSample(ctx, channel, frame);
                loudestSinceLastFrame_ = std::fmax(loudestSinceLastFrame_, std::fabs(sample));
                ctx.outputs[channel][frame] = sample * volume_;
            }
        }
    }

    // Inputs can be absent: treat a missing one as silence.
    static float inputSample(const mpc::ProcessContext& ctx, int channel, int frame)
    {
        const bool present = ctx.inputs != nullptr && channel < ctx.numInputs && ctx.inputs[channel] != nullptr;
        return present ? ctx.inputs[channel][frame] : 0.0f;
    }

    void showNextFrame(double secondsSinceLastFrame)
    {
        seconds_ += secondsSinceLastFrame;
        show(kSweep, sweepAt(seconds_));
        show(kLevel, fallingLevel());
        for (int column = 0; column < kWaveColumns; ++column)
            show(kFirstWaveColumn + column, waveHeightAt(seconds_, column));
    }

    // A triangle: 0 up to 100 and back down, once every kSweepSeconds.
    static float sweepAt(double seconds)
    {
        const double throughTheSweep = std::fmod(seconds, kSweepSeconds) / kSweepSeconds;
        return (float)(100.0 * (1.0 - std::fabs(2.0 * throughTheSweep - 1.0)));
    }

    float fallingLevel()
    {
        const float heard = mpc::clamp(loudestSinceLastFrame_, 0.0f, 1.0f) * 100.0f;
        level_                 = std::fmax(heard, level_ * kLevelFallPerFrame);
        loudestSinceLastFrame_ = 0.0f;
        return level_;
    }

    static float waveHeightAt(double seconds, int column)
    {
        const double alongTheWave = kWavesAcross * column / kWaveColumns;
        return (float)(50.0 + 45.0 * std::sin(kTwoPi * (seconds * kWaveTurnsPerSecond - alongTheWave)));
    }

    // Writes a display parameter and tells the MPC, unless the picture would not visibly change.
    void show(int index, float value)
    {
        if (std::fabs(get(index) - value) < kSmallestShownChange)
            return;
        set(index, value);
        notifyHostParameterChanged(index);
    }

    double sampleRate_            = 44100.0;
    double samplesUntilNextFrame_ = 0.0;
    double seconds_               = 0.0;
    float  volume_                = 1.0f;
    float  level_                 = 0.0f;
    float  loudestSinceLastFrame_ = 0.0f;
};

} // namespace screen_test
