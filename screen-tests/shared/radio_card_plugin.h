// The plugin behind the "Radio Check" screen test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME -> output, with the station added on top at the same volume.
//
// What it tests: whether a plugin on an MPC can play an internet radio station. A thread the plugin starts
// itself connects to one station that streams plain MP3 without encryption. The thread
// turns what arrives into sound with minimp3 (CC0, in engine/helpers/online/minimp3.h) and leaves it waiting in
// memory. The audio thread only takes sound that is already waiting, so a slow network gives a gap and
// never a freeze. The plugin sends the station one request and nothing else.
//
// What happens is shown on the screen through display parameters:
//
//   RADIO    how far it has got: looking the station up, tuning in, playing, or what went wrong
//   CODE     the number the station answered with (200 means "here it is")
//   PLAYED   how many seconds of the station have been played
//   GAPS     how many times the sound ran out and had to wait for more
//   WAITING  how full the store of sound waiting to be played is, in per cent
//   RATE     the station's sample rate, which should be the MPC's own
//   ASKED    how many frames of sound the MPC asked the plugin for in the last second
//   CALLS    in how many calls it asked for them
//   SMALLEST and LARGEST   the fewest and the most frames it asked for in one call
//
// The last four say how the MPC drives a plugin: sound runs out when it asks for more than a station sends.
//
// Real-time rules: all the waiting and all the decoding are done on the plugin's own thread, which never
// waits longer than a tenth of a second without looking whether it should stop. process() allocates, locks,
// logs and reads nothing. One known exception is part of the test, as on the other live cards: the MPC is
// told about each change from process(), on the audio thread.

#pragma once

#include "mpc_plugin.h"
#include "../../engine/helpers/online/small_sockets.h"

#define MINIMP3_IMPLEMENTATION
#define MINIMP3_FLOAT_OUTPUT
#define MINIMP3_NO_SIMD
#include "../../engine/helpers/online/minimp3.h"

// The station is the tester's own and is not kept with the card. station_here.example.h says how to give one.
#if __has_include("station_here.h")
#include "station_here.h"
#else
#include "station_here.example.h"
#endif

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstring>
#include <thread>
#include <vector>

namespace screen_test {

namespace net = mp_forge_net;

// Stereo sound that has arrived and waits to be played. One thread adds to it and one other takes from it,
// and neither ever waits for the other.
class SoundWaiting
{
public:
    explicit SoundWaiting(size_t frames) : frames_(frames), samples_(frames * 2, 0.0f) {}

    size_t waiting() const { return added_.load(std::memory_order_acquire) - taken_.load(std::memory_order_acquire); }
    size_t room() const { return frames_ - waiting(); }
    size_t holds() const { return frames_; }

    // For the thread that receives. The caller has made sure there is room.
    void add(const float* sound, size_t frames, int channels)
    {
        const size_t added = added_.load(std::memory_order_relaxed);
        for (size_t frame = 0; frame < frames; ++frame)
        {
            const size_t at = ((added + frame) % frames_) * 2;
            samples_[at]     = sound[frame * (size_t)channels];
            samples_[at + 1] = sound[frame * (size_t)channels + (size_t)(channels - 1)];
        }
        added_.store(added + frames, std::memory_order_release);
    }

    // For the audio thread: adds up to this many frames of waiting sound to what is already in the outputs.
    // Returns how many frames there were.
    size_t playOnto(float* const* outputs, int numChannels, size_t frames, float volume)
    {
        const size_t taken   = taken_.load(std::memory_order_relaxed);
        const size_t playing = std::min(frames, waiting());
        for (size_t frame = 0; frame < playing; ++frame)
        {
            const size_t at = ((taken + frame) % frames_) * 2;
            for (int channel = 0; channel < numChannels; ++channel)
                outputs[channel][frame] += samples_[at + (size_t)channel] * volume;
        }
        taken_.store(taken + playing, std::memory_order_release);
        return playing;
    }

private:
    const size_t        frames_;
    std::vector<float>  samples_;
    std::atomic<size_t> added_ { 0 };
    std::atomic<size_t> taken_ { 0 };
};

class RadioCardPlugin : public mpc::Plugin
{
public:
    RadioCardPlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId)), sound_(kFramesKeptWaiting)
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        addParam(mpc::choiceParam("radio", "Radio", { "Waiting", "No name", "No connection", "Refused", "Tuning in", "Playing", "Lost" }, kWaiting));
        addParam(mpc::intParam("code", "Code", 0, 999, 0));
        addParam(mpc::intParam("played", "Played", 0, kMostSecondsShown, 0, "s"));
        addParam(mpc::intParam("gaps", "Gaps", 0, kMostSecondsShown, 0));
        addParam(mpc::intParam("waiting", "Waiting", 0, 100, 0, "%"));
        addParam(mpc::intParam("rate", "Rate", 0, kHighestRateShown, 0, "Hz"));
        addParam(mpc::intParam("asked", "Asked", 0, kMostFramesShown, 0));
        addParam(mpc::intParam("calls", "Calls", 0, kMostFramesShown, 0));
        addParam(mpc::intParam("smallest", "Smallest", 0, kMostFramesShown, 0));
        addParam(mpc::intParam("largest", "Largest", 0, kMostFramesShown, 0));
    }

    ~RadioCardPlugin() override { stopListening(); }

    void prepare(double sampleRate, int) override
    {
        sampleRate_ = sampleRate;
        volume_     = targetVolume();
        startListeningOnce();
    }

    void process(mpc::ProcessContext& ctx) override
    {
        passSoundThrough(ctx);
        playTheStation(ctx);
        const Clock::time_point now = Clock::now();
        noteWhatWasAsked(ctx.numFrames, now);
        showNowAndThen(now);
    }

private:
    enum ParamIndex { kVolume = 0, kRadio, kCode, kPlayed, kGaps, kWaitingToPlay, kRate, kAsked, kCalls, kSmallest, kLargest };
    using Clock = std::chrono::steady_clock;

    // What the MPC asked of the plugin over a stretch of time.
    struct Asked
    {
        int frames   = 0;
        int calls    = 0;
        int smallest = 0;
        int largest  = 0;
    };
    enum HowFar { kWaiting = 0, kNoName, kNoConnection, kRefused, kTuningIn, kPlaying, kLost };

    static constexpr const char* kStationName = RADIO_CHECK_STATION;
    static constexpr const char* kAskForTheStream =
        "GET " RADIO_CHECK_STREAM " HTTP/1.0\r\nHost: " RADIO_CHECK_STATION "\r\nUser-Agent: mp-forge-radio-check\r\nConnection: close\r\n\r\n";
    static constexpr int    kStationPort         = 80;
    static constexpr int    kHereItIs            = 200;
    static constexpr size_t kFramesKeptWaiting   = 6 * 44100;      // about six seconds
    static constexpr double kSecondsBeforePlaying = 1.5;           // so a short hitch on the network is not heard
    static constexpr int    kSilentForTooLongMs  = 5000;
    static constexpr int    kSecondsBeforeRetry  = 5;
    static constexpr int    kLookEveryMs         = 100;
    static constexpr int    kLongestAnswer       = 4096;
    static constexpr int    kMostHeld            = 16 * 1024;      // room for several pieces of sound not yet decoded
    static constexpr int    kKeptBack            = 4096;           // more than two of the largest pieces a station sends
    static constexpr int    kMostSecondsShown    = 99999;
    static constexpr int    kHighestRateShown    = 96000;
    static constexpr int    kMostFramesShown     = 999999;
    static constexpr int    kShowEveryMs         = 250;            // often enough to read, seldom enough to cost nothing
    static constexpr int    kMaxChannels         = 2;
    static constexpr float  kSmoothingRate       = 0.002f;
    static constexpr float  kPercentToFactor     = 0.01f;

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
    // The audio thread: plays what is waiting, and never waits itself
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

    // Playing starts once enough sound is waiting, and stops to wait for more when it runs out.
    void playTheStation(mpc::ProcessContext& ctx)
    {
        const size_t wanted = (size_t)ctx.numFrames;
        if (!playing_ && sound_.waiting() >= (size_t)(kSecondsBeforePlaying * sampleRate_))
            playing_ = true;
        if (!playing_)
            return;
        const size_t played = sound_.playOnto(ctx.outputs, mpc::clamp(ctx.numOutputs, 0, kMaxChannels), wanted, volume_);
        framesPlayed_ += (double)played;
        if (played < wanted)
        {
            playing_ = false;
            ++gaps_;
        }
    }

    // Adds this call to the second being counted. When the second is over, it becomes what is shown.
    void noteWhatWasAsked(int numFrames, Clock::time_point now)
    {
        counting_.frames += numFrames;
        counting_.calls  += 1;
        counting_.smallest = counting_.calls == 1 ? numFrames : std::min(counting_.smallest, numFrames);
        counting_.largest  = std::max(counting_.largest, numFrames);
        if (now - countingSince_ < std::chrono::seconds(1))
            return;
        lastSecond_    = counting_;
        counting_      = Asked {};
        countingSince_ = now;
    }

    void showNowAndThen(Clock::time_point now)
    {
        if (now - lastShown_ < std::chrono::milliseconds(kShowEveryMs))
            return;
        lastShown_ = now;
        showWhatIsHappening();
    }

    void showWhatIsHappening()
    {
        const int reached = reached_.load(std::memory_order_relaxed);
        show(kRadio, reached == kTuningIn && playing_ ? (int)kPlaying : reached);
        show(kCode, code_.load(std::memory_order_relaxed));
        show(kPlayed, (int)std::min(framesPlayed_ / sampleRate_, (double)kMostSecondsShown));
        show(kGaps, std::min(gaps_, kMostSecondsShown));
        show(kWaitingToPlay, (int)(100 * sound_.waiting() / sound_.holds()));
        show(kRate, mpc::clamp(rate_.load(std::memory_order_relaxed), 0, kHighestRateShown));
        show(kAsked, std::min(lastSecond_.frames, kMostFramesShown));
        show(kCalls, lastSecond_.calls);
        show(kSmallest, lastSecond_.smallest);
        show(kLargest, lastSecond_.largest);
    }

    // Writes a display parameter and tells the MPC, when there is a new number for it.
    void show(int index, int found)
    {
        if ((int)std::lround(get(index)) == found)
            return;
        set(index, (float)found);
        notifyHostParameterChanged(index);
    }

    // ---------------------------------------------------------------------------------------
    // The plugin's own thread: listens to the station for as long as the plugin is there
    // ---------------------------------------------------------------------------------------

    void startListeningOnce()
    {
        if (!listening_.joinable())
            listening_ = std::thread([this] { keepListening(); });
    }

    void stopListening()
    {
        stop_.store(true);
        if (listening_.joinable())
            listening_.join();
    }

    bool shouldStop() const { return stop_.load(std::memory_order_relaxed); }

    void keepListening()
    {
        while (!shouldStop())
        {
            reached_.store(listenUntilItIsLost());
            waitSeconds(kSecondsBeforeRetry);
        }
    }

    void waitSeconds(int seconds) const
    {
        for (int waited = 0; waited < seconds * 1000 && !shouldStop(); waited += kLookEveryMs)
            std::this_thread::sleep_for(std::chrono::milliseconds(kLookEveryMs));
    }

    // Returns how it ended: with the station not found, not reached, refusing, or lost after playing.
    HowFar listenUntilItIsLost()
    {
        in_addr address {};
        if (!net::addressOf(kStationName, stop_, address))
            return kNoName;
        const int station = net::connectedTo(address, kStationPort, stop_);
        if (station == net::kNoSocket)
            return kNoConnection;
        const HowFar ended = stationSaysHereItIs(station) ? receiveFrom(station) : kRefused;
        close(station);
        return ended;
    }

    // Asks for the stream and reads the station's answer up to the empty line that ends it, one byte at a
    // time so that none of the sound after it is swallowed.
    bool stationSaysHereItIs(int station)
    {
        char answer[kLongestAnswer] = {};
        if (!net::sent(station, kAskForTheStream, std::strlen(kAskForTheStream)))
            return false;
        for (int length = 0; length < kLongestAnswer - 1 && std::strstr(answer, "\r\n\r\n") == nullptr; ++length)
            if (!net::ready(station, POLLIN, stop_) || recv(station, answer + length, 1, 0) != 1)
                return false;
        code_.store(mpc::clamp(net::codeIn(answer), 0, 999));
        return code_.load() == kHereItIs;
    }

    // Takes in the stream and turns it into sound for as long as it keeps coming.
    HowFar receiveFrom(int station)
    {
        mp3dec_t      decoder;
        unsigned char arrived[kMostHeld];
        int           held = 0;
        mp3dec_init(&decoder);
        reached_.store(kTuningIn);
        while (net::ready(station, POLLIN, stop_, kSilentForTooLongMs))
        {
            const ssize_t more = recv(station, arrived + held, (size_t)(kMostHeld - held), 0);
            if (more <= 0)
                break;
            held = soundOutOf(decoder, arrived, held + (int)more);
        }
        return kLost;
    }

    // Decodes the pieces of sound in what has arrived, and returns how many bytes are left over for the next time.
    // The decoder is only sure of a piece when it can see the start of the one after it. Given the very end of
    // what has arrived, it takes a piece that is cut off for rubbish, throws it away and starts afresh. So the
    // last few thousand bytes are always kept back until more has come in behind them.
    int soundOutOf(mp3dec_t& decoder, unsigned char* arrived, int held)
    {
        mp3d_sample_t       sound[MINIMP3_MAX_SAMPLES_PER_FRAME];
        mp3dec_frame_info_t about {};
        int                 used = 0;
        while (held - used > kKeptBack && !shouldStop())
        {
            const int frames = mp3dec_decode_frame(&decoder, arrived + used, held - used, sound, &about);
            if (about.frame_bytes == 0)
                break;
            used += about.frame_bytes;
            if (frames > 0)
                keepWaiting(sound, (size_t)frames, about);
        }
        std::memmove(arrived, arrived + used, (size_t)(held - used));
        return held - used;
    }

    // Leaves the sound waiting for the audio thread. When the store is full the thread holds back, and the
    // station is then simply read more slowly.
    void keepWaiting(const mp3d_sample_t* sound, size_t frames, const mp3dec_frame_info_t& about)
    {
        rate_.store(about.hz);
        while (sound_.room() < frames && !shouldStop())
            std::this_thread::sleep_for(std::chrono::milliseconds(5));
        if (!shouldStop())
            sound_.add(sound, frames, about.channels);
    }

    SoundWaiting      sound_;
    std::atomic<int>  reached_ { kWaiting };
    std::atomic<int>  code_ { 0 };
    std::atomic<int>  rate_ { 0 };
    std::atomic<bool> stop_ { false };
    std::thread       listening_;
    Asked             counting_;
    Asked             lastSecond_;
    Clock::time_point countingSince_ = Clock::now();
    Clock::time_point lastShown_     = Clock::now();
    double            sampleRate_   = 44100.0;
    double            framesPlayed_ = 0.0;
    int               gaps_         = 0;
    bool              playing_      = false;
    float             volume_       = 1.0f;
};

} // namespace screen_test
