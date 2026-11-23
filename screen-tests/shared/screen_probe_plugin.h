// The plugin behind the "Screen Probe" test card, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Signal flow: input -> VOLUME (parameter 0, smoothed, no side effects) -> output.
//
// What it tests: whether a plugin could paint a picture of its own into the part of the MPC's screen its page
// has, and stop when its page is no longer the one showing. Two things decide that, and the card looks at both.
//
//   1. How the MPC's picture reaches the screen. The card looks for the file Linux may offer a screen as
//      (/dev/fb0), reads how its dots lie, and, when PAINT is turned past halfway, paints a block into it for
//      eight seconds (see paint_test.h). Whether the block shows on the screen only eyes can tell.
//   2. Whether a plugin can tell that its page is showing. The card counts the calls the MPC makes to it, by
//      kind, and shows for each kind how many came in the last second and the longest time none came. If a
//      kind of call stops while another page is showing, a plugin can tell. TICK counts up once a second, so
//      that the MPC is told of a change at least that often and has a reason to ask.
//
// The card changes nothing on the MPC. It asks the screen's file how it lies, reads and writes dots in one
// block of it, and puts back what was under the block. It reads nothing else on the MPC.
//
// Parameters:
//
//   VOLUME   turns the sound passing through down
//   PAINT    turned past halfway, starts one paint test
//   FRESH    turned at all, starts the longest quiet times again from nothing
//   then nineteen readings, in the order the two pages show them (see Reading below)
//
// Real-time rules: the looking, painting and waiting are done on the plugin's own thread, which never waits
// longer than a fiftieth of a second without looking whether it should stop. process() allocates, locks, logs
// and reads nothing: it only copies whole numbers the thread left for it. A call is counted by adding one to
// a number, whichever thread the MPC makes it on. One known exception is part of the test, as on the other
// live cards: the MPC is told about each changed reading from process(), on the audio thread.

#pragma once

#include "mpc_plugin.h"
#include "paint_test.h"
#include "sound_passed_through.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <thread>

namespace screen_test {

// The calls of one kind the MPC makes. Counted on whichever thread they come on, looked at by one other.
class CallsOfOneKind
{
public:
    void count() { made_.fetch_add(1, std::memory_order_relaxed); }

    // Looked at ten times a second: the quiet time grows for as long as no call has come since the last look.
    void look()
    {
        const unsigned made = made_.load(std::memory_order_relaxed);
        quietLooks_ = made == madeAtLastLook_ ? quietLooks_ + 1 : 0;
        longestQuietLooks_ = std::max(longestQuietLooks_, quietLooks_);
        madeAtLastLook_ = made;
    }

    void closeASecond()
    {
        const unsigned made = made_.load(std::memory_order_relaxed);
        inLastSecond_ = (int)(made - madeAtLastSecond_);
        madeAtLastSecond_ = made;
    }

    void startAfresh() { quietLooks_ = longestQuietLooks_ = 0; }

    int inLastSecond() const { return inLastSecond_; }
    int longestQuietSeconds() const { return longestQuietLooks_ / kLooksASecond; }

    static constexpr int kLooksASecond = 10;

private:
    std::atomic<unsigned> made_ { 0 };
    unsigned              madeAtLastLook_ = 0, madeAtLastSecond_ = 0;
    int                   quietLooks_ = 0, longestQuietLooks_ = 0, inLastSecond_ = 0;
};

class ScreenProbePlugin : public mpc::Plugin
{
public:
    ScreenProbePlugin(const char* name, uint32_t uniqueId) : mpc::Plugin(infoFor(name, uniqueId))
    {
        addParam(mpc::floatParam("volume", "Volume", 0.0f, 100.0f, 100.0f, "%", 0));
        addParam(mpc::floatParam("paint", "Paint", 0.0f, 100.0f, 0.0f, "%", 0));
        addParam(mpc::floatParam("fresh", "Fresh", 0.0f, 100.0f, 0.0f, "%", 0));
        addTheScreenReadings();
        addTheCallReadings();
    }

    ~ScreenProbePlugin() override { stopWatching(); }

    void prepare(double, int) override
    {
        sound_.startAt(targetVolume());
        startWatchingOnce();
    }

    void process(mpc::ProcessContext& ctx) override
    {
        sound_.pass(ctx, targetVolume());
        for (int reading = 0; reading < kReadings; ++reading)
            show(kFirstReading + reading, found_[reading].load(std::memory_order_relaxed));
    }

    // The two readings that are words the screen's file gave, not numbers.
    bool formatValue(int index, float plainValue, std::string& out) override
    {
        const int reading = index - kFirstReading;
        if (reading != kName && reading != kOrder)
            return false;
        const bool isKnown = plainValue >= 0.5f;
        out = !isKnown ? "Unknown" : reading == kName ? screenName_ : colourOrder_;
        return true;
    }

    // Called for every call the MPC makes to the plugin but the ones that hand it sound.
    void countCall(VstInt32 opcode)
    {
        const int kind = kindOf(opcode);
        if (kind == kNotCounted)
            return;
        calls_[kind].count();
        if (kind == kOther)
            lastOtherCall_.store((int)opcode, std::memory_order_relaxed);
    }

    void countValueRead() { calls_[kValues].count(); }

private:
    enum ParamIndex { kVolume = 0, kPaint, kFresh, kFirstReading };
    enum CallKind { kText = 0, kValues, kIdle, kOther, kCallKinds, kNotCounted = kCallKinds };
    // The screen's nine readings, then two for each kind of call, the number of the last other call, and a tick.
    enum Reading
    {
        kScreen = 0, kName, kAcross, kDown, kBits, kOrder, kPainting, kKeptOnce, kKeptOver,
        kFirstOfTheCalls,
        kLastOtherCall = kFirstOfTheCalls + 2 * kCallKinds,
        kTick,
        kReadings
    };

    static constexpr int   kHighestShown    = 9999;
    static constexpr float kPercentToFactor = 0.01f;
    static constexpr float kHalfway         = 50.0f;
    static constexpr int   kStepsALook      = 1000 / CallsOfOneKind::kLooksASecond / PaintTest::kStepMs;
    static constexpr int   kStepsASecond    = 1000 / PaintTest::kStepMs;
    static constexpr char  kScreenPath[]    = "/dev/fb0";

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

    void addTheScreenReadings()
    {
        addParam(mpc::choiceParam("screen", "Screen", { "Waiting", "No screen file", "Cannot open", "Cannot ask", "Open" }, ScreenFile::kWaiting));
        addParam(mpc::intParam("name", "Name", 0, 1, 0));
        addParam(mpc::intParam("across", "Across", 0, kHighestShown, 0));
        addParam(mpc::intParam("down", "Down", 0, kHighestShown, 0));
        addParam(mpc::intParam("bits", "Bits", 0, kHighestShown, 0));
        addParam(mpc::intParam("order", "Order", 0, 1, 0));
        addParam(mpc::choiceParam("painting", "Painting", { "Not yet", "Cannot here", "Could not write", "Painted once", "Painting over", "Done" }, PaintTest::kNotYet));
        addParam(mpc::intParam("kept_once", "Kept once", 0, 100, 0, "%"));
        addParam(mpc::intParam("kept_over", "Kept over", 0, 100, 0, "%"));
    }

    void addTheCallReadings()
    {
        addReadingsOfCalls("text", "Text");
        addReadingsOfCalls("values", "Values");
        addReadingsOfCalls("idle", "Idle");
        addReadingsOfCalls("other", "Other");
        addParam(mpc::intParam("last_other", "Last other", 0, kHighestShown, 0));
        addParam(mpc::intParam("tick", "Tick", 0, kHighestShown, 0));
    }

    void addReadingsOfCalls(const std::string& id, const std::string& name)
    {
        addParam(mpc::intParam((id + "_now").c_str(), (name + " now").c_str(), 0, kHighestShown, 0));
        addParam(mpc::intParam((id + "_quiet").c_str(), (name + " quiet").c_str(), 0, kHighestShown, 0, "s"));
    }

    static int inLastSecondOf(int kind) { return kFirstOfTheCalls + 2 * kind; }
    static int longestQuietOf(int kind) { return inLastSecondOf(kind) + 1; }

    static int kindOf(VstInt32 opcode)
    {
        switch (opcode)
        {
            case effGetParamDisplay: case effGetParamName: case effGetParamLabel: return kText;
            case effEditIdle: case effIdle_Deprecated: return kIdle;
            case effProcessEvents: return kNotCounted;
            default: return kOther;
        }
    }

    float targetVolume() const { return get(kVolume) * kPercentToFactor; }

    // Writes a reading and tells the MPC, when the thread has left a new number for it.
    void show(int index, int found)
    {
        if ((int)std::lround(get(index)) == found)
            return;
        set(index, (float)found);
        notifyHostParameterChanged(index);
    }

    void leave(int reading, int found) { found_[reading].store(mpc::clamp(found, 0, kHighestShown), std::memory_order_relaxed); }

    // ---------------------------------------------------------------------------------------
    // The plugin's own thread
    // ---------------------------------------------------------------------------------------

    void startWatchingOnce()
    {
        if (!watching_.joinable())
            watching_ = std::thread([this] { keepWatching(); });
    }

    void stopWatching()
    {
        stop_.store(true);
        if (watching_.joinable())
            watching_.join();
    }

    void keepWatching()
    {
        screen_.openAt(kScreenPath);
        leaveWhatTheScreenIs();
        auto nextStep = std::chrono::steady_clock::now();
        for (int step = 1; !stop_.load(std::memory_order_relaxed); ++step)
        {
            stepThePaintTest();
            if (step % kStepsALook == 0)
                lookAtTheCalls();
            if (step % kStepsASecond == 0)
                closeASecondOfCalls();
            nextStep += std::chrono::milliseconds(PaintTest::kStepMs);
            std::this_thread::sleep_until(nextStep);
        }
        paintTest_.endEarly();
    }

    void leaveWhatTheScreenIs()
    {
        leave(kScreen, screen_.howFar());
        if (screen_.howFar() != ScreenFile::kOpen)
            return;
        const ScreenLayout& layout = screen_.layout();
        std::snprintf(screenName_, sizeof(screenName_), "%s", layout.name);
        std::snprintf(colourOrder_, sizeof(colourOrder_), "R%d G%d B%d", layout.red.firstBit, layout.green.firstBit, layout.blue.firstBit);
        leave(kAcross, layout.across);
        leave(kDown, layout.down);
        leave(kBits, layout.bytesADot * 8);
        leave(kName, 1);
        leave(kOrder, 1);
    }

    void stepThePaintTest()
    {
        const bool isAsked = get(kPaint) > kHalfway;
        if (isAsked && !wasAsked_ && !paintTest_.isUnderWay())
            paintTest_.begin();
        else
            paintTest_.step();
        wasAsked_ = isAsked;
        leave(kPainting, paintTest_.stage());
        leave(kKeptOnce, paintTest_.percentKeptWhenPaintedOnce());
        leave(kKeptOver, paintTest_.percentKeptWhenPaintedOver());
    }

    void lookAtTheCalls()
    {
        startAfreshIfAsked();
        for (int kind = 0; kind < kCallKinds; ++kind)
        {
            calls_[kind].look();
            leave(longestQuietOf(kind), calls_[kind].longestQuietSeconds());
        }
    }

    void startAfreshIfAsked()
    {
        const float fresh = get(kFresh);
        if (fresh == freshAtLastLook_)
            return;
        freshAtLastLook_ = fresh;
        for (CallsOfOneKind& calls : calls_)
            calls.startAfresh();
    }

    void closeASecondOfCalls()
    {
        for (int kind = 0; kind < kCallKinds; ++kind)
        {
            calls_[kind].closeASecond();
            leave(inLastSecondOf(kind), calls_[kind].inLastSecond());
        }
        leave(kLastOtherCall, lastOtherCall_.load(std::memory_order_relaxed));
        leave(kTick, secondsWatched_ = (secondsWatched_ + 1) % (kHighestShown + 1));
    }

    SoundPassedThrough sound_;
    ScreenFile         screen_;
    PaintTest          paintTest_ { screen_ };
    CallsOfOneKind     calls_[kCallKinds];
    std::atomic<int>   lastOtherCall_ { 0 };
    std::atomic<int>   found_[kReadings] = {};
    std::atomic<bool>  stop_ { false };
    std::thread        watching_;
    bool               wasAsked_ = false;
    int                secondsWatched_ = 0;
    float              freshAtLastLook_ = 0.0f;
    char               screenName_[ScreenLayout::kNameLength + 1] = {};
    char               colourOrder_[24] = {};
};

// ---------------------------------------------------------------------------------------
// Counting the MPC's calls: each is counted, then answered the way every plugin of the kit answers it
// ---------------------------------------------------------------------------------------

inline ScreenProbePlugin* probeOf(AEffect* effect) { return static_cast<ScreenProbePlugin*>(mpc::detail::self(effect)); }

inline VstIntPtr countedCall(AEffect* effect, VstInt32 opcode, VstInt32 index, VstIntPtr value, void* ptr, float opt)
{
    if (ScreenProbePlugin* probe = probeOf(effect))
        probe->countCall(opcode);
    return mpc::detail::dispatcher(effect, opcode, index, value, ptr, opt);
}

inline float countedValueRead(AEffect* effect, VstInt32 index)
{
    if (ScreenProbePlugin* probe = probeOf(effect))
        probe->countValueRead();
    return mpc::detail::getParameter(effect, index);
}

inline AEffect* withItsCallsCounted(AEffect* effect)
{
    if (effect == nullptr)
        return nullptr;
    effect->dispatcher   = &countedCall;
    effect->getParameter = &countedValueRead;
    return effect;
}

} // namespace screen_test

// Used in place of MPC_PLUGIN_ENTRY, so that the calls are counted from the first one on.
#define SCREEN_PROBE_ENTRY(PluginClass)                                                       \
    static AEffect* probeMadeFor(audioMasterCallback host) MPC_PLUGIN_ENTRY_BODY(PluginClass) \
    extern "C" MPC_EXPORT AEffect* VSTPluginMain(audioMasterCallback host)                    \
    { return screen_test::withItsCallsCounted(probeMadeFor(host)); }
