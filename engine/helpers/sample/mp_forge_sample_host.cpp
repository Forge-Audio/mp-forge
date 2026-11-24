// The program that runs a Sample app on the MPC Sample itself, with no computer.
//
// MP Forge builds this together with the app's own code into one program for the device. It does there what
// MP Forge's player does on a computer: it tells the app what the musician touches, plays the app's sound,
// shows its picture and its lights, and reads the files the app asks for. A game controller plugged into the
// device is read for the app too, and the app hears the microphone or the line in when it listens there. The
// app's code is the same on both.
//
//   app <screen> <controls> <sound> <sound switches> <the app's sounds> <the card>
//
// Holding SHIFT and STOP together for a second leaves the app, whatever the app does with those buttons. An
// app that stops answering is left too, so the device is never stuck in it.

#include "mp_forge_device_controls.h"
#include "mp_forge_device_map.h"
#include "mp_forge_host_files.h"
#include "mp_forge_host_gamepad.h"
#include "mp_forge_host_listening.h"
#include "mp_forge_host_sound.h"
#include "mp_forge_host_surface.h"
#include "mp_forge_host_xbox_gamepad.h"
#include "mpc_sample_app.h"

#include <atomic>
#include <chrono>
#include <csignal>
#include <fcntl.h>
#include <mutex>
#include <poll.h>
#include <pthread.h>
#include <thread>
#include <unistd.h>

// What the app's own code gives, through the one line that names its class: MPC_SAMPLE_APP(TheApp).
extern "C" {
int32_t mp_forge_sample_kit();
void* mp_forge_sample_make(double framesASecond, const sample::SampleGiven* samples, int32_t sampleCount);
void mp_forge_sample_told(void* made, int32_t touched, int32_t number, float value);
void mp_forge_sample_sound(void* made, const float* inLeft, const float* inRight, float* outLeft, float* outRight, int32_t frames);
void mp_forge_sample_show(void* made, uint8_t* picture, uint8_t* padColours, uint8_t* litButtons);
void mp_forge_sample_levels(void* made, uint8_t* lit);
int32_t mp_forge_sample_sound_in(void* made);
int32_t mp_forge_sample_asked(void* made, char* path, int32_t* intoSample);
void mp_forge_sample_listed(void* made, const sample::CardEntryGiven* found, int32_t count, int32_t cardIs);
void mp_forge_sample_loaded(void* made, int32_t number, const sample::SampleGiven* loaded, const char* whyNot);
}

namespace mp_forge_host {

using Clock = std::chrono::steady_clock;

constexpr int kKitThisProgramSpeaks = 7;
constexpr auto kAFrameOfThePicture = std::chrono::milliseconds(33);
constexpr auto kHeldToLeave = std::chrono::seconds(1);
constexpr auto kLongestAnAppMayTakeToAnswer = std::chrono::seconds(5);
// Until the main volume knob has said where it stands, the app is heard at a volume that startles nobody.
constexpr float kMainVolumeNotYetKnown = 0.5f;
// From 1 to 99, where more is sooner. The device's own sound drivers work above this.
constexpr int kPlaceOfTheSoundAmongTheDevicesWork = 40;
constexpr int kLeftAsAsked = 0, kCouldNotStart = 1, kAppStoppedAnswering = 3, kAppFellOver = 4;

// What the app shows: its picture as the kit draws it, and its lights.
struct Shown {
    std::array<uint8_t, sample::Screen::kByteCount> picture{};
    std::array<uint8_t, DeviceLights::kPadBytes> padColours{};
    std::array<uint8_t, sample::kButtons> litButtons{};
    std::array<uint8_t, sample::kLevelStrips> levelLights{};
};

// One stretch of what came in at the device's inputs, a side at a time.
struct HeardAtTheInputs {
    const float* left;
    const float* right;
};

// The app, reached from one place at a time, as the kit promises it. When each call began is kept, so that
// an app that never comes back from one can be found out.
class RunningApp {
public:
    void start(std::vector<HeldSound> sounds) {
        sounds_ = std::move(sounds);
        sounds_.resize(static_cast<size_t>(sample::kMostSamples));
        std::vector<sample::SampleGiven> given;
        for (const HeldSound& sound : sounds_)
            if (!sound.left.empty()) given.push_back(sound.given());
        made_ = mp_forge_sample_make(kFramesASecond, given.data(), static_cast<int32_t>(given.size()));
    }

    void tell(sample::Touched what, int32_t number, float value) {
        const Call call(*this);
        mp_forge_sample_told(made_, static_cast<int32_t>(what), number, value);
    }

    void makeSound(const HeardAtTheInputs& heard, float* left, float* right, int frames) {
        const Call call(*this);
        mp_forge_sample_sound(made_, heard.left, heard.right, left, right, frames);
    }

    void show(Shown& shown) {
        const Call call(*this);
        mp_forge_sample_show(made_, shown.picture.data(), shown.padColours.data(), shown.litButtons.data());
        mp_forge_sample_levels(made_, shown.levelLights.data());
    }

    // Where the app has its inputs listen.
    sample::SoundIn soundIn() {
        const Call call(*this);
        return static_cast<sample::SoundIn>(mp_forge_sample_sound_in(made_));
    }

    sample::Asked askedOfTheCard(char* path, int32_t* intoSample) {
        const Call call(*this);
        return static_cast<sample::Asked>(mp_forge_sample_asked(made_, path, intoSample));
    }

    void handOverFolder(const std::vector<Listed>& entries, sample::CardIs cardIs) {
        std::vector<sample::CardEntryGiven> given;
        for (const Listed& entry : entries) given.push_back({entry.name.c_str(), entry.isFolder ? 1 : 0});
        const Call call(*this);
        mp_forge_sample_listed(made_, given.data(), static_cast<int32_t>(given.size()), static_cast<int32_t>(cardIs));
    }

    // The sound that was at the number is let go only once the app has been handed the new one.
    void handOverSound(HeldSound sound, int32_t intoSample) {
        if (!hasRoomFor(sound, intoSample)) return sayWhyNot(kNoRoomForIt);
        const sample::SampleGiven given = sound.given();
        {
            const Call call(*this);
            mp_forge_sample_loaded(made_, intoSample, &given, nullptr);
        }
        sounds_[static_cast<size_t>(intoSample)] = std::move(sound);
    }

    void sayWhyNot(const char* whyNot) {
        const Call call(*this);
        mp_forge_sample_loaded(made_, 0, nullptr, whyNot);
    }

    bool hasNotAnsweredFor(Clock::duration longest) const {
        const Clock::rep began = callBegan_.load();
        return began != kNotInACall && Clock::now().time_since_epoch().count() - began > longest.count();
    }

private:
    static constexpr Clock::rep kNotInACall = 0;

    // One call into the app's code, from its start to its end.
    class Call {
    public:
        explicit Call(RunningApp& app) : app_(app), held_(app.oneAtATime_) { app_.callBegan_ = Clock::now().time_since_epoch().count(); }
        ~Call() { app_.callBegan_ = kNotInACall; }

    private:
        RunningApp& app_;
        std::lock_guard<std::mutex> held_;
    };

    // Whether the app would still hold no more sound than the device has room for, with this one at that number.
    bool hasRoomFor(const HeldSound& sound, int32_t intoSample) const {
        if (intoSample < 0 || intoSample >= sample::kMostSamples) return false;
        size_t heldElsewhere = 0;
        for (int32_t number = 0; number < sample::kMostSamples; ++number)
            if (number != intoSample) heldElsewhere += sounds_[static_cast<size_t>(number)].numbersHeld();
        return heldElsewhere + sound.numbersHeld() <= kMostNumbersOfSoundAnAppHolds;
    }

    void* made_ = nullptr;
    std::mutex oneAtATime_;
    std::atomic<Clock::rep> callBegan_{kNotInACall};
    std::vector<HeldSound> sounds_;
};

// The device's screen. It takes a dot as blue, green, red and a byte it ignores, and only the rows that
// changed are written to it, which is quicker on a screen of its kind.
class Panel {
public:
    explicit Panel(int file) : file_(file) {}

    void show(const std::array<uint8_t, sample::Screen::kByteCount>& picture) {
        const std::vector<uint8_t> rows = asTheScreenTakesIt(picture);
        int first = 0, last = sample::kScreenHeight - 1;
        if (isKnown_) {
            while (first <= last && isTheSameRow(rows, first)) ++first;
            while (last > first && isTheSameRow(rows, last)) --last;
        }
        if (first <= last) writeRows(rows, first, last);
        onTheScreen_ = rows;
        isKnown_ = true;
    }

private:
    static constexpr size_t kBytesADot = 4;
    static constexpr size_t kBytesARow = static_cast<size_t>(sample::kScreenWidth) * kBytesADot;

    static std::vector<uint8_t> asTheScreenTakesIt(const std::array<uint8_t, sample::Screen::kByteCount>& picture) {
        std::vector<uint8_t> rows(picture.size());
        for (size_t dot = 0; dot < rows.size(); dot += kBytesADot) {
            rows[dot] = picture[dot + 2]; rows[dot + 1] = picture[dot + 1]; rows[dot + 2] = picture[dot]; rows[dot + 3] = 0;
        }
        return rows;
    }

    bool isTheSameRow(const std::vector<uint8_t>& rows, int row) const {
        const size_t at = static_cast<size_t>(row) * kBytesARow;
        return std::equal(&rows[at], &rows[at] + kBytesARow, &onTheScreen_[at]);
    }

    void writeRows(const std::vector<uint8_t>& rows, int first, int last) {
        const size_t at = static_cast<size_t>(first) * kBytesARow;
        if (lseek(file_, static_cast<off_t>(at), SEEK_SET) < 0) return;
        writeAll(file_, &rows[at], static_cast<size_t>(last - first + 1) * kBytesARow);
    }

    int file_;
    bool isKnown_ = false;
    std::vector<uint8_t> onTheScreen_;
};

// The way out of an app: SHIFT and STOP, held together.
class WayOut {
public:
    void heard(sample::Touched what, int32_t number, float value) {
        if (what != sample::Touched::Button) return;
        if (number == static_cast<int32_t>(sample::Button::Shift)) shiftIsHeld_ = value > 0.0f;
        if (number == static_cast<int32_t>(sample::Button::Stop)) stopIsHeld_ = value > 0.0f;
        if (!(shiftIsHeld_ && stopIsHeld_)) isHeld_ = false;
        else if (!isHeld_) { isHeld_ = true; heldSince_ = Clock::now(); }
    }

    bool isAsked() const { return isHeld_ && Clock::now() - heldSince_ >= kHeldToLeave; }

private:
    bool shiftIsHeld_ = false, stopIsHeld_ = false, isHeld_ = false;
    Clock::time_point heldSince_;
};

std::atomic<bool> gWasToldToEnd{false};

void endWhenTold(int) { gWasToldToEnd = true; }

void sayTheAppFellOver(int) {
    const char said[] = "The app fell over. Its code did something a program is stopped for.\n";
    (void)!write(STDERR_FILENO, said, sizeof said - 1);
    _exit(kAppFellOver);
}

// An app that never comes back from a call holds everything up, the way out included. So a watch is kept
// from outside it, and a run whose app has stopped answering is ended.
void watchOver(const RunningApp& app) {
    for (;;) {
        std::this_thread::sleep_for(std::chrono::milliseconds(250));
        if (!app.hasNotAnsweredFor(kLongestAnAppMayTakeToAnswer)) continue;
        const char said[] = "The app stopped answering, so it was left.\n";
        (void)!write(STDERR_FILENO, said, sizeof said - 1);
        _exit(kAppStoppedAnswering);
    }
}

// The device's sound: where the app's goes, and where what the app hears comes from.
struct Sounds {
    SoundOut& goingOut;
    Listening& comingIn;
};

// The whole of a run: the app with the device's screen, controls, lights, sound and card around it.
class Run : private HearsTheControls, private HearsTheGamepad {
public:
    Run(int screen, int controls, Sounds sounds, SdCard card)
        : panel_(screen), controls_(controls), lights_(controls), sound_(sounds.goingOut), listening_(sounds.comingIn), card_(std::move(card)) {}

    void start(std::vector<HeldSound> sounds) {
        app_.start(std::move(sounds));
        std::thread(watchOver, std::cref(app_)).detach();
        std::thread([this] { keepMakingSound(); }).detach();
        writeAll(controls_, device_map::kAsksWhereTheKnobsStand, sizeof device_map::kAsksWhereTheKnobsStand);
    }

    // Comes back when the musician leaves the app, or when the controls can no longer be read.
    void keepGoing() {
        Clock::time_point nextPicture = Clock::now();
        while (!wayOut_.isAsked() && !gWasToldToEnd && hearTheControlsUntil(nextPicture)) {
            if (Clock::now() < nextPicture) continue;
            answerWhatWasAskedOfTheCard();
            listening_.listenAt(app_.soundIn());
            showWhatTheAppShows();
            nextPicture = std::max(nextPicture + kAFrameOfThePicture, Clock::now());
        }
        lights_.goDark();
    }

private:
    void touched(sample::Touched what, int32_t number, float value) override {
        wayOut_.heard(what, number, value);
        app_.tell(what, number, value);
    }

    void gamepadDid(sample::Touched what, int32_t number, float value) override { app_.tell(what, number, value); }

    // The knob is followed as hearing does: its lower half is for quiet, not for half of everything.
    void mainVolumeAt(float stands) override { mainVolume_ = stands * stands; }

    // Waits for the device's own controls and for a game controller together. False once the device's
    // controls can no longer be read.
    bool hearTheControlsUntil(Clock::time_point when) {
        const auto wait = std::chrono::duration_cast<std::chrono::milliseconds>(when - Clock::now()).count();
        gamepad_.lookForOne();
        xboxGamepad_.lookForOne();
        pollfd watched[3] = {{controls_, POLLIN, 0}, {gamepad_.file(), POLLIN, 0}, {xboxGamepad_.file(), POLLOUT, 0}};
        if (poll(watched, 3, static_cast<int>(std::max<long long>(0, wait))) <= 0) return true;
        if (watched[1].revents != 0) gamepad_.hear(*this);
        if (watched[2].revents != 0) xboxGamepad_.hear(*this);
        return watched[0].revents == 0 || hearTheDevicesControls();
    }

    bool hearTheDevicesControls() {
        uint8_t sent[256];
        const ssize_t count = read(controls_, sent, sizeof sent);
        if (count < 0 && errno == EINTR) return true;
        if (count <= 0) return false;
        heard_.read(sent, static_cast<size_t>(count), *this);
        return true;
    }

    // A file is read with the app left free to go on making its sound.
    void answerWhatWasAskedOfTheCard() {
        char path[sample::kLongestCardPath + 1] = {};
        int32_t intoSample = 0;
        const sample::Asked asked = app_.askedOfTheCard(path, &intoSample);
        if (asked == sample::Asked::Folder) handOverFolder(path);
        if (asked == sample::Asked::File) handOverFile(path, intoSample);
    }

    void handOverFolder(const char* path) {
        std::vector<Listed> entries;
        const bool isThere = card_.list(path, entries);
        app_.handOverFolder(entries, isThere ? sample::CardIs::There : sample::CardIs::Missing);
    }

    void handOverFile(const char* path, int32_t intoSample) {
        HeldSound sound;
        const char* whyNot = card_.load(path, kFramesASecond, sound);
        if (whyNot != nullptr) return app_.sayWhyNot(whyNot);
        app_.handOverSound(std::move(sound), intoSample);
    }

    void showWhatTheAppShows() {
        app_.show(shown_);
        panel_.show(shown_.picture);
        lights_.show(shown_.padColours, shown_.litButtons, shown_.levelLights);
    }

    // The sound is made on a thread of its own, which the device is asked to put before everything else. At
    // its inputs the app hears what comes in where it listens, a stretch for each stretch it makes.
    void keepMakingSound() {
        sched_param first{};
        first.sched_priority = kPlaceOfTheSoundAmongTheDevicesWork;
        pthread_setschedparam(pthread_self(), SCHED_FIFO, &first);
        const size_t frames = static_cast<size_t>(sound_.framesAStretch());
        std::vector<float> heardLeft(frames), heardRight(frames), left(frames), right(frames);
        for (;;) {
            listening_.hear(heardLeft.data(), heardRight.data(), static_cast<int>(frames));
            app_.makeSound({heardLeft.data(), heardRight.data()}, left.data(), right.data(), static_cast<int>(frames));
            sound_.play(left.data(), right.data(), mainVolume_);
        }
    }

    RunningApp app_;
    Panel panel_;
    int controls_;
    Controls heard_;
    ListedGamepad gamepad_;
    XboxGamepad xboxGamepad_;
    DeviceLights lights_;
    SoundOut& sound_;
    Listening& listening_;
    SdCard card_;
    WayOut wayOut_;
    Shown shown_;
    std::atomic<float> mainVolume_{kMainVolumeNotYetKnown * kMainVolumeNotYetKnown};
};

int couldNotStart(const char* why, const char* what) {
    std::fprintf(stderr, "The app could not start: %s %s\n", why, what);
    return kCouldNotStart;
}

void standUpToWhatEndsAProgram() {
    for (const int fellOver : {SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT}) signal(fellOver, sayTheAppFellOver);
    for (const int toldToEnd : {SIGTERM, SIGINT}) signal(toldToEnd, endWhenTold);
    signal(SIGPIPE, SIG_IGN);
}

int run(char** places) {
    if (mp_forge_sample_kit() != kKitThisProgramSpeaks) return couldNotStart("it was built with another version of MP Forge's kit.", "");
    const int screen = open(places[1], O_WRONLY | O_CLOEXEC);
    if (screen < 0) return couldNotStart("the screen could not be opened:", places[1]);
    const int controls = device_controls::openTheControls(places[2]);
    if (controls < 0) return couldNotStart("the controls could not be opened:", places[2]);
    standUpToWhatEndsAProgram();
    SoundOut sound(places[3], places[4]);
    Listening listening(places[3], places[4]);
    Run theRun(screen, controls, Sounds{sound, listening}, SdCard(places[6]));
    theRun.start(soundsAddedToTheApp(places[5], kFramesASecond));
    theRun.keepGoing();
    return kLeftAsAsked;
}

}  // namespace mp_forge_host

int main(int argumentCount, char** arguments) {
    if (argumentCount != 7) return mp_forge_host::couldNotStart("it is started as", "app <screen> <controls> <sound> <sound switches> <the app's sounds> <the card>");
    // The threads are not waited for: the run is over the moment the musician leaves.
    _exit(mp_forge_host::run(arguments));
}
