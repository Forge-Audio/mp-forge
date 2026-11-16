// The plugin behind every screen test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME (parameter 0, smoothed, no side effects) -> output.
//
// The other fifteen parameters ("Knob 2" to "Knob 16") change nothing in the sound. They exist so the
// test screens have values to show and turn. All the testing happens in the hand-written skins.
//
// Real-time rules: nothing is allocated, locked, logged or read in process().

#pragma once

#include "mpc_plugin.h"

#include <string>

namespace screen_test {

constexpr int   kKnobCount       = 16;
constexpr int   kVolume          = 0;
constexpr int   kMaxChannels     = 2;
constexpr float kSmoothingRate   = 0.002f;
constexpr float kPercentToFactor = 0.01f;

class TestCardPlugin : public mpc::Plugin
{
public:
    TestCardPlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId))
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        for (int number = 2; number <= kKnobCount; ++number)
            addValueOnlyKnob(number);
    }

    void prepare(double, int) override { reset(); }

    void reset() override { volume_ = targetVolume(); }

    void process(mpc::ProcessContext& ctx) override
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

private:
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

    void addValueOnlyKnob(int number)
    {
        const std::string id   = "knob" + std::to_string(number);
        const std::string name = "Knob " + std::to_string(number);
        addParam(mpc::floatParam(id.c_str(), name.c_str(), 0.0f, 100.0f, 50.0f, "%", 0));
    }

    float targetVolume() const { return get(kVolume) * kPercentToFactor; }

    // Inputs can be absent: treat a missing one as silence.
    static float inputSample(const mpc::ProcessContext& ctx, int channel, int frame)
    {
        const bool present = ctx.inputs != nullptr && channel < ctx.numInputs && ctx.inputs[channel] != nullptr;
        return present ? ctx.inputs[channel][frame] : 0.0f;
    }

    float volume_ = 1.0f;
};

} // namespace screen_test
