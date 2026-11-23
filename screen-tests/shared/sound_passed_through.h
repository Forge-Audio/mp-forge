// The sound of a test card that is not there for its sound: what comes in goes out, turned down by Volume.
//
// SPDX-License-Identifier: MIT

#pragma once

#include "mpc_plugin.h"

namespace screen_test {

class SoundPassedThrough
{
public:
    void startAt(float volume) { volume_ = volume; }

    // Real-time safe: it allocates, locks, logs and reads nothing.
    void pass(mpc::ProcessContext& ctx, float targetVolume)
    {
        const int channelCount = mpc::clamp(ctx.numOutputs, 0, kMaxChannels);
        for (int frame = 0; frame < ctx.numFrames; ++frame)
        {
            volume_ += kSmoothingRate * (targetVolume - volume_);
            for (int channel = 0; channel < channelCount; ++channel)
                ctx.outputs[channel][frame] = inputSample(ctx, channel, frame) * volume_;
        }
    }

private:
    static constexpr int   kMaxChannels   = 2;
    static constexpr float kSmoothingRate = 0.002f;

    // Inputs can be absent: a missing one is silence.
    static float inputSample(const mpc::ProcessContext& ctx, int channel, int frame)
    {
        const bool isPresent = ctx.inputs != nullptr && channel < ctx.numInputs && ctx.inputs[channel] != nullptr;
        return isPresent ? ctx.inputs[channel][frame] : 0.0f;
    }

    float volume_ = 1.0f;
};

} // namespace screen_test
