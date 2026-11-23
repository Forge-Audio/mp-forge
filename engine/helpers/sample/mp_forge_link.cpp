// MP Forge Link: the program MP Forge puts on an MPC Sample, so that the device can be the screen and the
// controls of an app that runs in MP Forge on a computer.
//
// It carries. Whatever the device's controls send goes to the computer as it is, and the computer sends
// back rows of the picture for the screen. Which control is which is known on the computer alone. While no
// computer is linked the screen says so, and a push of the dial ends the program.
//
// It also takes an app from the computer and puts it on the SD card, at a door of its own, and runs that app
// when asked: see mp_forge_link_apps.h. While the app runs the device is the app's. When the app is left the
// link is back.
//
//   mp-forge-link <screen> <controls> <port> [<port for apps> <folder for apps> [<the device's own storage>]]
//
// The screen is the file a picture is written to, 320 by 240 dots of four bytes each. The controls are the
// port the device's pads, knobs and buttons speak MIDI on. The computer reaches the program at the port.
// Started without a port for apps, the program takes none.

#include "mp_forge_link_apps.h"
#include "mpc_sample_app.h"

#include <arpa/inet.h>
#include <cerrno>
#include <csignal>
#include <cstdlib>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <spawn.h>
#include <string>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>

extern char** environ;

namespace {

// What the program says first to a computer that reaches it, so that MP Forge knows it has found the program.
const char kGreeting[] = "MP FORGE LINK 1\n";
// The two things a computer sends: rows of the picture for the screen, and bytes to pass on to the controls,
// which is how their lights are set.
constexpr uint8_t kRowsForTheScreen = 'P';
constexpr uint8_t kBytesForTheControls = 'S';

constexpr int kBytesADot = 4;
constexpr size_t kBytesARow = static_cast<size_t>(sample::kScreenWidth) * kBytesADot;
constexpr size_t kMostBytesForTheControls = 1024;
constexpr int kSecondsAComputerMayTakeOverAMessage = 2;
constexpr int kSecondsAComputerMayTakeOverAnApp = 10;
constexpr int kSecondsTheMusicianHasToAnswer = 30;
constexpr int kNobody = -1;
// What the script that starts an app is told, so that it leaves the device to the link when the app is left.
char kStartedByTheLink[] = "from-mp-forge-link";
char kShell[] = "sh";

// The dial pushed in is a note on the controls' first channel.
constexpr uint8_t kNoteOnTheFirstChannel = 0x90;
constexpr uint8_t kNoteOfTheDial = 111;
constexpr uint8_t kFirstStatusByte = 0x80;
constexpr uint8_t kFirstByteSentInBetween = 0xF8;

const sample::Colour kDark{16, 16, 24};
const sample::Colour kInk{236, 235, 242};
const sample::Colour kSoft{120, 117, 135};
const sample::Colour kAccent{181, 39, 31};

using link_apps::readAll;
using link_apps::writeAll;

// A number sent as two bytes, the low one first.
int numberAt(const uint8_t* bytes) { return bytes[0] | bytes[1] << 8; }

// The device's screen. It takes a dot as blue, green, red and a byte it ignores.
class Panel {
public:
    explicit Panel(const int& file) : file_(file) {}

    void showRows(int firstRow, const uint8_t* rows, size_t byteCount) {
        if (lseek(file_, static_cast<off_t>(firstRow) * static_cast<off_t>(kBytesARow), SEEK_SET) < 0) return;
        writeAll(file_, rows, byteCount);
    }

    void show(const sample::Screen& picture) {
        std::vector<uint8_t> rows(sample::Screen::kByteCount);
        for (size_t dot = 0; dot < rows.size(); dot += kBytesADot) {
            const uint8_t* drawn = picture.bytes() + dot;
            rows[dot] = drawn[2]; rows[dot + 1] = drawn[1]; rows[dot + 2] = drawn[0]; rows[dot + 3] = 0;
        }
        showRows(0, rows.data(), rows.size());
    }

private:
    // The screen is let go of while an app runs and opened again after, so its file is looked up each time.
    const int& file_;
};

// The device's screen and controls, which the link lets go of while an app has them.
class Device {
public:
    Device(const char* screen, const char* controls) : screenPath_(screen), controlsPath_(controls) {}

    bool take() {
        screen_ = open(screenPath_, O_WRONLY | O_CLOEXEC);
        controls_ = open(controlsPath_, O_RDWR | O_CLOEXEC);
        return screen_ >= 0 && controls_ >= 0;
    }

    void letGo() {
        close(screen_);
        close(controls_);
    }

    const int& screen() const { return screen_; }
    int controls() const { return controls_; }

private:
    const char* screenPath_;
    const char* controlsPath_;
    int screen_ = kNobody;
    int controls_ = kNobody;
};

// What the buttons say among the bytes the controls send: a push of the dial, or a press of any other button.
enum class Pressed { Nothing, TheDial, AnotherButton };

class ButtonPresses {
public:
    Pressed lastIn(const uint8_t* bytes, size_t count) {
        Pressed last = Pressed::Nothing;
        for (size_t at = 0; at < count; ++at) {
            const Pressed ended = pressEndedBy(bytes[at]);
            if (ended != Pressed::Nothing) last = ended;
        }
        return last;
    }

private:
    Pressed pressEndedBy(uint8_t byte) {
        if (byte >= kFirstByteSentInBetween) return Pressed::Nothing;
        if (byte >= kFirstStatusByte) return startAMessage(byte);
        if (status_ != kNoteOnTheFirstChannel) return Pressed::Nothing;
        if (note_ == kNoNoteYet) return takeTheNote(byte);
        const bool isAPress = byte > 0;
        const Pressed pressed = note_ == kNoteOfTheDial ? Pressed::TheDial : Pressed::AnotherButton;
        note_ = kNoNoteYet;
        return isAPress ? pressed : Pressed::Nothing;
    }

    Pressed startAMessage(uint8_t status) {
        status_ = status;
        note_ = kNoNoteYet;
        return Pressed::Nothing;
    }

    Pressed takeTheNote(uint8_t note) {
        note_ = note;
        return Pressed::Nothing;
    }

    static constexpr int kNoNoteYet = -1;
    int status_ = 0;
    int note_ = kNoNoteYet;
};

void writeAcross(sample::Screen& screen, const char* words, int y, sample::Colour colour, int size) {
    screen.write(words, (sample::kScreenWidth - sample::Screen::widthOf(words, size)) / 2, y, colour, size);
}

const sample::Screen& waitingPicture() {
    static sample::Screen picture;
    picture.clear(kDark);
    picture.fill(0, 0, sample::kScreenWidth, 6, kAccent);
    writeAcross(picture, "MP FORGE LINK", 56, kInk, 4);
    writeAcross(picture, "PLUG THE USB-C CABLE INTO THE COMPUTER", 112, kSoft, 2);
    writeAcross(picture, "AND OPEN AN APP IN MP FORGE", 130, kSoft, 2);
    writeAcross(picture, "PUSH THE DIAL TO LEAVE", 196, kInk, 2);
    return picture;
}

const sample::Screen& askingPicture() {
    static sample::Screen picture;
    picture.clear(kDark);
    picture.fill(0, 0, sample::kScreenWidth, 6, kAccent);
    writeAcross(picture, "THE COMPUTER WANTS TO", 40, kInk, 2);
    writeAcross(picture, "PUT APPS ON THIS CARD AND RUN THEM", 58, kInk, 2);
    writeAcross(picture, "PUSH THE DIAL TO ALLOW IT", 112, kInk, 3);
    writeAcross(picture, "ANY BUTTON SAYS NO", 150, kSoft, 2);
    writeAcross(picture, "ASKED ONCE EACH TIME THE LINK STARTS", 196, kSoft, 2);
    return picture;
}

sample::Screen arrivingPicture(const std::string& name) {
    sample::Screen picture;
    picture.clear(kDark);
    picture.fill(0, 0, sample::kScreenWidth, 6, kAccent);
    writeAcross(picture, "TAKING AN APP", 56, kInk, 4);
    writeAcross(picture, name.c_str(), 112, kInk, 2);
    writeAcross(picture, "FROM THE COMPUTER", 150, kSoft, 2);
    return picture;
}

// The two doors computers come in by: one to link, and one to hand over an app. The second is nobody's when
// the program was started without it.
struct Doors {
    int forLinking;
    int forApps;
};

// The link itself: it carries between one computer and the device's screen and controls, and takes apps.
class Link : private link_apps::TheMusician {
public:
    Link(Device& device, Doors doors, link_apps::FolderForApps apps) : device_(device), panel_(device.screen()), doors_(doors), apps_(std::move(apps)) {}

    // Comes back when the dial is pushed while no computer is linked, or when the device can no longer be used.
    void run() {
        showThatItWaits();
        while (takeWhatComesNext()) {}
    }

private:
    static constexpr int kTheControls = 0, kTheDoorForLinking = 1, kTheComputer = 2, kTheDoorForApps = 3, kWatched = 4;

    void showThatItWaits() { panel_.show(waitingPicture()); }

    bool isLinked() const { return computer_ != kNobody; }

    // The door for linking is only looked at while nobody is linked: one computer at a time.
    bool takeWhatComesNext() {
        pollfd watched[kWatched] = {{device_.controls(), POLLIN, 0}, {isLinked() ? kNobody : doors_.forLinking, POLLIN, 0}, {computer_, POLLIN, 0}, {doors_.forApps, POLLIN, 0}};
        if (!waitFor(watched, kWatched, kForever)) return false;
        if (watched[kTheControls].revents != 0 && !hearTheControls()) return false;
        if ((watched[kTheDoorForLinking].revents & POLLIN) != 0) letAComputerIn();
        if (watched[kTheComputer].revents != 0 && isLinked() && !takeAMessageFrom(computer_)) letTheComputerGo();
        if ((watched[kTheDoorForApps].revents & POLLIN) != 0) return takeAnApp();
        return true;
    }

    static constexpr int kForever = -1;

    static bool waitFor(pollfd* watched, int count, int milliseconds) {
        while (poll(watched, static_cast<nfds_t>(count), milliseconds) < 0)
            if (errno != EINTR) return false;
        return true;
    }

    // What the controls send goes to the computer while one is linked. While none is, a push of the dial ends
    // the program. Controls that can no longer be read end it too, so that the device is not left stuck.
    bool hearTheControls() {
        uint8_t sent[256];
        const ssize_t count = read(device_.controls(), sent, sizeof sent);
        if (count < 0 && errno == EINTR) return true;
        if (count <= 0) return false;
        if (!isLinked()) return buttons_.lastIn(sent, static_cast<size_t>(count)) != Pressed::TheDial;
        if (!writeAll(computer_, sent, static_cast<size_t>(count))) letTheComputerGo();
        return true;
    }

    void letAComputerIn() {
        const int computer = accept(doors_.forLinking, nullptr, nullptr);
        if (computer < 0) return;
        computer_ = computer;
        if (!greet(computer)) letTheComputerGo();
    }

    void letTheComputerGo() {
        if (isLinked()) close(computer_);
        computer_ = kNobody;
        buttons_ = ButtonPresses();
        showThatItWaits();
    }

    // A computer that stops in the middle of a message is let go, so the controls are never held up for long.
    static bool greet(int computer) {
        const int atOnce = 1;
        setsockopt(computer, IPPROTO_TCP, TCP_NODELAY, &atOnce, sizeof atOnce);
        waitNoLongerThan(kSecondsAComputerMayTakeOverAMessage, computer);
        fcntl(computer, F_SETFD, FD_CLOEXEC);
        return writeAll(computer, reinterpret_cast<const uint8_t*>(kGreeting), sizeof kGreeting - 1);
    }

    static void waitNoLongerThan(int seconds, int computer) {
        const timeval patience{seconds, 0};
        setsockopt(computer, SOL_SOCKET, SO_RCVTIMEO, &patience, sizeof patience);
    }

    bool takeAMessageFrom(int computer) {
        uint8_t kind = 0;
        if (!readAll(computer, &kind, 1)) return false;
        if (kind == kRowsForTheScreen) return showRowsFrom(computer);
        if (kind == kBytesForTheControls) return passOnToTheControlsFrom(computer);
        return false;
    }

    bool showRowsFrom(int computer) {
        uint8_t which[4];
        if (!readAll(computer, which, sizeof which)) return false;
        const int firstRow = numberAt(which), rowCount = numberAt(which + 2);
        if (rowCount == 0 || firstRow + rowCount > sample::kScreenHeight) return false;
        taken_.resize(static_cast<size_t>(rowCount) * kBytesARow);
        if (!readAll(computer, taken_.data(), taken_.size())) return false;
        panel_.showRows(firstRow, taken_.data(), taken_.size());
        return true;
    }

    bool passOnToTheControlsFrom(int computer) {
        uint8_t howMany[2];
        if (!readAll(computer, howMany, sizeof howMany)) return false;
        const size_t byteCount = static_cast<size_t>(numberAt(howMany));
        if (byteCount > kMostBytesForTheControls) return false;
        taken_.resize(byteCount);
        return readAll(computer, taken_.data(), byteCount) && writeAll(device_.controls(), taken_.data(), byteCount);
    }

    // A computer at the door for apps is served to its end before anything else. Once the link has drawn
    // over the picture of a linked computer, that computer is let go: it links again and sends its whole
    // picture afresh. False when the device could not be taken back from an app that ran.
    bool takeAnApp() {
        const int computer = accept(doors_.forApps, nullptr, nullptr);
        if (computer < 0) return true;
        fcntl(computer, F_SETFD, FD_CLOEXEC);
        waitNoLongerThan(kSecondsAComputerMayTakeOverAnApp, computer);
        hasDrawnOverThePicture_ = false;
        const std::string script = link_apps::Visit(computer, apps_, *this).serve();
        close(computer);
        const bool hasTheDevice = script.empty() || runTheApp(script);
        if (hasTheDevice && hasDrawnOverThePicture_) letTheComputerGo();
        return hasTheDevice;
    }

    void drawOverThePicture(const sample::Screen& picture) {
        hasDrawnOverThePicture_ = true;
        panel_.show(picture);
    }

    // Asked on the device, once each time the link starts: an app is somebody's program, and it runs with
    // every right the device has.
    bool allowsApps() override {
        if (!isAllowed_) isAllowed_ = theMusicianSaysYes();
        return isAllowed_;
    }

    bool theMusicianSaysYes() {
        drawOverThePicture(askingPicture());
        ButtonPresses answers;
        for (int secondsLeft = kSecondsTheMusicianHasToAnswer; secondsLeft > 0; --secondsLeft) {
            const Pressed pressed = pressedWithinASecond(answers);
            if (pressed != Pressed::Nothing) return pressed == Pressed::TheDial;
        }
        return false;
    }

    Pressed pressedWithinASecond(ButtonPresses& answers) {
        pollfd watched{device_.controls(), POLLIN, 0};
        uint8_t sent[256];
        if (!waitFor(&watched, 1, 1000) || watched.revents == 0) return Pressed::Nothing;
        const ssize_t count = read(device_.controls(), sent, sizeof sent);
        return count > 0 ? answers.lastIn(sent, static_cast<size_t>(count)) : Pressed::AnotherButton;
    }

    void isShownThatAnAppArrives(const std::string& name) override { drawOverThePicture(arrivingPicture(name)); }

    // Runs an app's start script and waits until the app is left. The screen and the controls are the app's
    // meanwhile. False when they could not be taken back.
    bool runTheApp(const std::string& script) {
        std::string scriptRun = script;
        char* const started[] = {kShell, &scriptRun[0], kStartedByTheLink, nullptr};
        pid_t app = 0;
        hasDrawnOverThePicture_ = true;
        letTheComputerGo();
        device_.letGo();
        if (posix_spawn(&app, "/bin/sh", nullptr, nullptr, started, environ) == 0) waitUntilLeft(app);
        return device_.take();
    }

    static void waitUntilLeft(pid_t app) {
        int howItEnded = 0;
        while (waitpid(app, &howItEnded, 0) < 0 && errno == EINTR) {}
    }

    Device& device_;
    Panel panel_;
    Doors doors_;
    link_apps::FolderForApps apps_;
    int computer_ = kNobody;
    bool isAllowed_ = false;
    bool hasDrawnOverThePicture_ = false;
    ButtonPresses buttons_;
    std::vector<uint8_t> taken_;
};

// A door computers come in by: a socket waiting at the port, on whichever network the device is on.
int doorAt(int port) {
    const int door = socket(AF_INET, SOCK_STREAM, 0);
    const int again = 1;
    sockaddr_in here{};
    here.sin_family = AF_INET;
    here.sin_addr.s_addr = htonl(INADDR_ANY);
    here.sin_port = htons(static_cast<uint16_t>(port));
    setsockopt(door, SOL_SOCKET, SO_REUSEADDR, &again, sizeof again);
    fcntl(door, F_SETFD, FD_CLOEXEC);
    const bool isOpen = door >= 0 && bind(door, reinterpret_cast<const sockaddr*>(&here), sizeof here) == 0 && listen(door, 1) == 0;
    return isOpen ? door : kNobody;
}

int couldNotStart(const char* why, const char* what) {
    std::fprintf(stderr, "mp-forge-link: %s %s\n", why, what);
    return EXIT_FAILURE;
}

}  // namespace

constexpr int kWithoutApps = 4, kWithApps = 6, kWithAppsAndOwnStorage = 7;

int main(int argumentCount, char** arguments) {
    const bool isStartedRight = argumentCount == kWithoutApps || argumentCount == kWithApps || argumentCount == kWithAppsAndOwnStorage;
    if (!isStartedRight) return couldNotStart("it is started as", "mp-forge-link <screen> <controls> <port> [<port for apps> <folder for apps> [<the device's own storage>]]");
    Device device(arguments[1], arguments[2]);
    if (!device.take()) return couldNotStart("the screen or the controls could not be opened:", arguments[1]);
    const bool takesApps = argumentCount >= kWithApps;
    const Doors doors{doorAt(std::atoi(arguments[3])), takesApps ? doorAt(std::atoi(arguments[4])) : kNobody};
    if (doors.forLinking == kNobody) return couldNotStart("nothing could wait at port", arguments[3]);
    if (takesApps && doors.forApps == kNobody) return couldNotStart("nothing could wait at port", arguments[4]);
    signal(SIGPIPE, SIG_IGN);
    const link_apps::FolderForApps apps(takesApps ? arguments[5] : "", argumentCount == kWithAppsAndOwnStorage ? arguments[6] : "");
    Link(device, doors, apps).run();
    return EXIT_SUCCESS;
}
