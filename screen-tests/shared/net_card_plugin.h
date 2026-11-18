// The plugin behind the "Net Check" screen test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME (parameter 0, smoothed, no side effects) -> output.
//
// What it tests: whether a plugin on an MPC can reach the internet, and whether it can keep a thread of its
// own. Every ten seconds a thread the plugin starts makes one try:
//
//   1. it asks a public name server (1.1.1.1) for the address of example.com,
//   2. it connects to that address,
//   3. it asks for the front page and reads the first line of the answer.
//
// It sends nothing else, and keeps nothing of what it reads but the number the web server answered with.
// What happened is shown on the screen through display parameters:
//
//   NET     how far the last try got
//   CODE    the number the web server answered with (200 means "here it is")
//   TRIES   how many tries have been made
//   GOOD    how many of them were answered
//   TOOK    how long the last try took, in milliseconds
//   TICK    counts up once a second for as long as the plugin's own thread runs
//
// Real-time rules: all the waiting is done on the plugin's own thread, which never waits longer than a tenth
// of a second without looking whether it should stop. process() allocates, locks, logs and reads nothing: it
// only copies whole numbers the thread left for it. One known exception is part of the test, as on the other
// live cards: the MPC is told about each change from process(), on the audio thread.

#pragma once

#include "mpc_plugin.h"
#include "../../engine/helpers/online/small_sockets.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstring>
#include <thread>

namespace screen_test {

namespace net = mp_forge_net;

class NetCardPlugin : public mpc::Plugin
{
public:
    NetCardPlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId))
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        addParam(mpc::choiceParam("net", "Net", { "Waiting", "No name", "No connection", "No answer", "Answered" }, kWaiting));
        addParam(mpc::intParam("code", "Code", 0, kHighestShown, 0));
        addParam(mpc::intParam("tries", "Tries", 0, kHighestShown, 0));
        addParam(mpc::intParam("good", "Good", 0, kHighestShown, 0));
        addParam(mpc::intParam("took", "Took", 0, kHighestShown, 0, "ms"));
        addParam(mpc::intParam("tick", "Tick", 0, kHighestShown, 0));
    }

    ~NetCardPlugin() override { stopTrying(); }

    void prepare(double, int) override
    {
        volume_ = targetVolume();
        startTryingOnce();
    }

    void process(mpc::ProcessContext& ctx) override
    {
        passSoundThrough(ctx);
        for (int shown = 0; shown < kShown; ++shown)
            show(kFirstShown + shown, found_[shown].load(std::memory_order_relaxed));
    }

private:
    enum ParamIndex { kVolume = 0, kFirstShown };
    enum Shown { kNet = 0, kCode, kTries, kGood, kTook, kTick, kShown };
    enum HowFar { kWaiting = 0, kNoName, kNoConnection, kNoAnswer, kAnswered };

    static constexpr int   kHighestShown        = 9999;
    static constexpr int   kMaxChannels         = 2;
    static constexpr float kSmoothingRate       = 0.002f;
    static constexpr float kPercentToFactor     = 0.01f;
    static constexpr int   kSecondsBetweenTries = 10;
    static constexpr int   kSecondsToFirstTry   = 2;
    static constexpr int   kLookEveryMs         = 100;
    static constexpr int   kWebPort             = 80;

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

    // ---------------------------------------------------------------------------------------
    // The sound: passed through, as on every test card
    // ---------------------------------------------------------------------------------------

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

    // Writes a display parameter and tells the MPC, when the thread has left a new number for it.
    void show(int index, int found)
    {
        if ((int)std::lround(get(index)) == found)
            return;
        set(index, (float)found);
        notifyHostParameterChanged(index);
    }

    // ---------------------------------------------------------------------------------------
    // The plugin's own thread
    // ---------------------------------------------------------------------------------------

    void startTryingOnce()
    {
        if (!trying_.joinable())
            trying_ = std::thread([this] { keepTrying(); });
    }

    void stopTrying()
    {
        stop_.store(true);
        if (trying_.joinable())
            trying_.join();
    }

    bool shouldStop() const { return stop_.load(std::memory_order_relaxed); }

    void keepTrying()
    {
        int secondsToNextTry = kSecondsToFirstTry;
        while (waitOneSecond())
        {
            countUp(kTick);
            if (--secondsToNextTry > 0)
                continue;
            secondsToNextTry = kSecondsBetweenTries;
            tryOnce();
        }
    }

    // False once the plugin is going away.
    bool waitOneSecond() const
    {
        for (int waited = 0; waited < 1000 && !shouldStop(); waited += kLookEveryMs)
            std::this_thread::sleep_for(std::chrono::milliseconds(kLookEveryMs));
        return !shouldStop();
    }

    void countUp(Shown which) { found_[which].store((found_[which].load() + 1) % (kHighestShown + 1)); }

    void tryOnce()
    {
        const auto began = std::chrono::steady_clock::now();
        countUp(kTries);
        const int code = answerOfTheWebServer();
        if (code > 0)
        {
            found_[kCode].store(code);
            countUp(kGood);
        }
        const auto took = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - began).count();
        found_[kTook].store((int)std::min<long long>(took, kHighestShown));
    }

    // The number example.com's web server answers with, or 0. How far the try got is left for the screen.
    int answerOfTheWebServer()
    {
        in_addr address {};
        if (!net::addressOf("example.com", stop_, address))
            return gotAsFarAs(kNoName);
        const int web = net::connectedTo(address, kWebPort, stop_);
        if (web == net::kNoSocket)
            return gotAsFarAs(kNoConnection);
        const int code = codeOfTheFrontPage(web);
        close(web);
        gotAsFarAs(code > 0 ? kAnswered : kNoAnswer);
        return code;
    }

    int gotAsFarAs(HowFar howFar)
    {
        found_[kNet].store(howFar);
        return 0;
    }

    // ---------------------------------------------------------------------------------------
    // The three steps of one try
    // ---------------------------------------------------------------------------------------

    // The number a web server puts first in its answer, or 0 when it gave no such line.
    int codeOfTheFrontPage(int web) const
    {
        static const char asking[] = "GET / HTTP/1.0\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        char firstWords[32] = {};
        if (!net::sent(web, asking, sizeof(asking) - 1) || !net::ready(web, POLLIN, stop_))
            return 0;
        if (recv(web, firstWords, sizeof(firstWords) - 1, 0) <= 0)
            return 0;
        return std::strncmp(firstWords, "HTTP/", 5) == 0 ? mpc::clamp(net::codeIn(firstWords), 0, kHighestShown) : 0;
    }

    std::atomic<int>  found_[kShown] = {};
    std::atomic<bool> stop_ { false };
    std::thread       trying_;
    float             volume_ = 1.0f;
};

} // namespace screen_test
