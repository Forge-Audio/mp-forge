// Stream Check - plays an internet radio station through MP Forge's own stream helper, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written the way MP Forge's guide mp-forge/online.md tells a plugin to read a stream: the addresses are in
// online.json, and this code never touches the network itself.

#include "mpc_plugin.h"
#include "dsp/smoother.h"
#include "mp_forge_stream.h"

#include <vector>

using namespace mpc::dsp;

class StreamCheck : public mpc::Plugin
{
public:
    enum Params { kVolume, kPlaying, kStation, kNumParams };

    static mpc::PluginInfo makeInfo()
    {
        mpc::PluginInfo info;
        info.name = "Stream Check";
        info.vendor = "Workbench Tests";
        info.uniqueId = MPC_FOURCC('W', 'S', 'c', '1');
        info.version = 1000;
        info.isInstrument = false;
        info.numInputs = 2;
        info.numOutputs = 2;
        info.tailSamples = 0;
        return info;
    }

    StreamCheck() : mpc::Plugin(makeInfo())
    {
        using namespace mpc;
        addParam(floatParam("volume", "Volume", 0.0f, 100.0f, 70.0f, "%", 0));
        addParam(boolParam("playing", "Playing", true));
        addParam(choiceParam("station", "Station", {"First", "Second"}, 0));
        if (numParams() != kNumParams) mpc::log("StreamCheck: parameter count mismatch");
    }

    void prepare(double sampleRate, int maxBlockSize) override
    {
        stream_.start(sampleRate);
        fromTheStreamLeft_.assign((size_t)maxBlockSize, 0.0f);
        fromTheStreamRight_.assign((size_t)maxBlockSize, 0.0f);
        volume_.setTime(kFadeMs, sampleRate);
        volume_.snap(0.0f);
    }

    void process(mpc::ProcessContext& ctx) override
    {
        stream_.open(streamWanted());
        volume_.setTarget(volumeWanted());
        stream_.read(fromTheStreamLeft_.data(), fromTheStreamRight_.data(), ctx.numFrames);
        addTheStreamTo(ctx);
    }

private:
    static constexpr float kFadeMs = 20.0f;

    int streamWanted() const { return getBool(kPlaying) ? getInt(kStation) : mpforge::AudioStream::kNoStream; }

    // Silent until the station is heard, so that it fades in when it starts and after the station changes.
    float volumeWanted() const
    {
        const bool heard = stream_.state() == mpforge::AudioStream::kPlaying && stream_.opened() == streamWanted();
        return heard ? get(kVolume) * 0.01f : 0.0f;
    }

    // Plays what was read from the station on top of what passes through.
    void addTheStreamTo(mpc::ProcessContext& ctx)
    {
        const float* inLeft = ctx.inputs[0];
        const float* inRight = ctx.numInputs > 1 ? ctx.inputs[1] : ctx.inputs[0];
        float* outLeft = ctx.outputs[0];
        float* outRight = ctx.numOutputs > 1 ? ctx.outputs[1] : ctx.outputs[0];
        for (int frame = 0; frame < ctx.numFrames; ++frame)
        {
            const float volume = volume_.process();
            outLeft[frame] = inLeft[frame] + fromTheStreamLeft_[(size_t)frame] * volume;
            outRight[frame] = inRight[frame] + fromTheStreamRight_[(size_t)frame] * volume;
        }
    }

    mpforge::AudioStream stream_;
    std::vector<float>   fromTheStreamLeft_, fromTheStreamRight_;
    Smoother             volume_;
};

MPC_PLUGIN_ENTRY(StreamCheck)
