// How MP Forge Link takes an app from the computer and puts it on the SD card, so that the card never has to
// leave the MPC Sample.
//
// Apps come in at a door of their own, apart from the one the picture and the controls go through. A computer
// says a fixed line first, then asks one thing at a time and is answered each time with one byte:
//
//   B  begin an app: one byte for how long its name is, then the name
//   F  a file of it: two bytes for how long its path is, the path, four bytes for its size, four for the sum
//      of its bytes, then the bytes
//   E  end: the app takes its place on the card
//   R  run an app that is on the card: one byte for how long its name is, then the name
//
// Numbers are sent low byte first. Whatever is refused ends the visit.
//
// An app is written beside its place, in a folder the boot menu does not list, and moved there whole at the
// end. A visit that breaks off takes that folder away again, so the card never holds half an app.
//
// Nothing is written anywhere but inside the one folder this was given: the folder on the card where the
// boot menu looks. A name or a path that would lead out of it is refused. A folder there that MP Forge did
// not make is never written over.

#pragma once

#include <cerrno>
#include <cstdint>
#include <cstring>
#include <dirent.h>
#include <fcntl.h>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

namespace link_apps {

constexpr char kGreeting[] = "MP FORGE LINK APPS 1\n";
constexpr char kSaidFirstByAComputer[] = "MP FORGE SENDS AN APP\n";

enum class Asked : uint8_t { Begin = 'B', File = 'F', End = 'E', Run = 'R' };

enum class Answer : uint8_t {
    Done = 'K',
    // The musician did not say yes on the device.
    NotAllowed = 'N',
    // The name or the path is not one that stays inside the folder for apps.
    Refused = 'B',
    // A folder of that name is there already, and MP Forge did not make it.
    NameIsTaken = 'T',
    // The card did not take what was written to it.
    NotWritten = 'C',
    // What arrived is not what was sent.
    Damaged = 'D',
    // There is no such app on the card to run.
    NoSuchApp = 'S',
    // The folder for apps is not on a card, but on the device's own storage.
    NotOnACard = 'I',
};

// The script that starts an app made with MP Forge, which is also how such an app is told from any other folder.
constexpr char kStartScriptOfAnApp[] = "start-app.sh";
// The boot menu leaves a folder out of its list when its name starts with this.
constexpr char kHidesAFolderFromTheMenu = '_';
constexpr char kEndsAnArrivingFolder[] = ".arriving";
constexpr char kEndsTheFolderBefore[] = ".before";

constexpr size_t kLongestName = 64;
constexpr uint32_t kLargestFile = 64u * 1024u * 1024u;
constexpr int kFoldersDeepAFileMayLie = 1;

inline bool readAll(int from, uint8_t* into, size_t count) {
    while (count > 0) {
        const ssize_t arrived = read(from, into, count);
        if (arrived < 0 && errno == EINTR) continue;
        if (arrived <= 0) return false;
        into += arrived;
        count -= static_cast<size_t>(arrived);
    }
    return true;
}

inline bool writeAll(int to, const uint8_t* bytes, size_t count) {
    while (count > 0) {
        const ssize_t written = write(to, bytes, count);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) return false;
        bytes += written;
        count -= static_cast<size_t>(written);
    }
    return true;
}

// One step of a path: a name that is only a name, and so can lead nowhere but into the folder it is in.
inline bool isOnlyAName(const std::string& step) {
    if (step.empty() || step.size() > kLongestName || step == "." || step == "..") return false;
    for (const char letter : step)
        if (letter == '/' || letter == '\\' || static_cast<unsigned char>(letter) < ' ') return false;
    return true;
}

// The name of an app: only a name, and one the boot menu lists.
inline bool canNameAnApp(const std::string& name) { return isOnlyAName(name) && name[0] != kHidesAFolderFromTheMenu && name[0] != '.'; }

// A file's path within its app: names with a / between them, no deeper than an app's files lie.
inline bool staysWithinItsApp(const std::string& path) {
    int foldersDeep = 0;
    size_t from = 0;
    for (size_t slash = path.find('/'); slash != std::string::npos; from = slash + 1, slash = path.find('/', from))
        if (!isOnlyAName(path.substr(from, slash - from)) || ++foldersDeep > kFoldersDeepAFileMayLie) return false;
    return isOnlyAName(path.substr(from));
}

inline bool isThere(const std::string& path) {
    struct stat found {};
    return lstat(path.c_str(), &found) == 0;
}

// Takes a folder away with everything in it. A link to somewhere else is taken away as the link it is.
inline bool takeAway(const std::string& path) {
    struct stat found {};
    if (lstat(path.c_str(), &found) != 0) return errno == ENOENT;
    if (!S_ISDIR(found.st_mode)) return unlink(path.c_str()) == 0;
    std::vector<std::string> names;
    if (DIR* opened = opendir(path.c_str())) {
        while (const dirent* entry = readdir(opened))
            if (std::strcmp(entry->d_name, ".") != 0 && std::strcmp(entry->d_name, "..") != 0) names.emplace_back(entry->d_name);
        closedir(opened);
    }
    bool isEmptied = true;
    for (const std::string& name : names) isEmptied = takeAway(path + "/" + name) && isEmptied;
    return isEmptied && rmdir(path.c_str()) == 0;
}

// Adds up the bytes of a file as they arrive, so that what was written can be known to be what was sent.
class Sum {
public:
    void add(const uint8_t* bytes, size_t count) {
        for (size_t at = 0; at < count; ++at) sum_ = (sum_ ^ bytes[at]) * 16777619u;
    }

    uint32_t value() const { return sum_; }

private:
    uint32_t sum_ = 2166136261u;
};

// The folder on the card where the boot menu looks, which is the only place an app is ever written.
class FolderForApps {
public:
    // `ownStorage` is a place on the device's own storage. A folder for apps that lies on the same storage
    // is not on a card, and nothing is written there. Left empty, nothing is known to be the device's own.
    FolderForApps(std::string folder, const std::string& ownStorage) : folder_(std::move(folder)) {
        struct stat here {}, own {};
        const bool isFound = !folder_.empty() && stat(folder_.c_str(), &here) == 0 && S_ISDIR(here.st_mode);
        const bool isOnOwnStorage = !ownStorage.empty() && stat(ownStorage.c_str(), &own) == 0 && isFound && own.st_dev == here.st_dev;
        isOnACard_ = isFound && !isOnOwnStorage;
    }

    bool isOnACard() const { return isOnACard_; }

    bool isTakenBySomethingElse(const std::string& name) const { return isThere(placeOf(name)) && !isThere(startScriptOf(name)); }

    std::string startScriptOf(const std::string& name) const { return placeOf(name) + "/" + kStartScriptOfAnApp; }

    // Whatever was left of an earlier try at the same app is taken away first.
    bool begin(const std::string& name) {
        abandon();
        arriving_ = name;
        return takeAway(beside(name, kEndsAnArrivingFolder)) && mkdir(beside(name, kEndsAnArrivingFolder).c_str(), 0755) == 0;
    }

    bool hasBegun() const { return !arriving_.empty(); }

    // Opens a file of the app that is arriving, to be written. Below zero when the card will not have it.
    int openForWriting(const std::string& pathWithinTheApp) const {
        const std::string file = beside(arriving_, kEndsAnArrivingFolder) + "/" + pathWithinTheApp;
        const size_t lastSlash = file.rfind('/');
        mkdir(file.substr(0, lastSlash).c_str(), 0755);
        return open(file.c_str(), O_WRONLY | O_CREAT | O_TRUNC | O_NOFOLLOW | O_CLOEXEC, 0755);
    }

    void takeAwayFile(const std::string& pathWithinTheApp) const { unlink((beside(arriving_, kEndsAnArrivingFolder) + "/" + pathWithinTheApp).c_str()); }

    // The app that arrived takes its place. An older one of the same name steps aside first, and is thrown
    // away only once the new one is there.
    bool end() {
        const std::string name = arriving_;
        const std::string before = beside(name, kEndsTheFolderBefore);
        arriving_.clear();
        if (!takeAway(before)) return false;
        if (isThere(placeOf(name)) && rename(placeOf(name).c_str(), before.c_str()) != 0) return false;
        if (rename(beside(name, kEndsAnArrivingFolder).c_str(), placeOf(name).c_str()) != 0) return putBack(name, before);
        takeAway(before);
        sync();
        return true;
    }

    // Takes away what has arrived of an app that did not arrive whole.
    void abandon() {
        if (hasBegun()) takeAway(beside(arriving_, kEndsAnArrivingFolder));
        arriving_.clear();
    }

private:
    std::string placeOf(const std::string& name) const { return folder_ + "/" + name; }

    // A folder next to the app's own that the boot menu does not list.
    std::string beside(const std::string& name, const char* ending) const { return folder_ + "/" + kHidesAFolderFromTheMenu + name + ending; }

    bool putBack(const std::string& name, const std::string& before) {
        if (isThere(before)) rename(before.c_str(), placeOf(name).c_str());
        takeAway(beside(name, kEndsAnArrivingFolder));
        return false;
    }

    std::string folder_;
    bool isOnACard_ = false;
    std::string arriving_;
};

// Whoever holds the device's screen and controls while an app is taken.
struct TheMusician {
    virtual ~TheMusician() = default;
    // Whether the musician lets this computer put apps on the card and run them. They are asked on the device.
    virtual bool allowsApps() = 0;
    virtual void isShownThatAnAppArrives(const std::string& name) = 0;
};

// One visit of a computer at the door for apps, from its first line to whatever ends it.
class Visit {
public:
    Visit(int computer, FolderForApps& apps, TheMusician& musician) : computer_(computer), apps_(apps), musician_(musician) {}

    // Serves the computer until it is done. Answers with the script of the app it asked to have run, or with
    // nothing when it asked for none.
    std::string serve() {
        if (greet()) while (takeWhatIsAsked()) {}
        apps_.abandon();
        return scriptToRun_;
    }

private:
    static constexpr size_t kBytesAtATime = 64 * 1024;

    bool greet() {
        uint8_t said[sizeof kSaidFirstByAComputer - 1];
        const bool isGreeted = writeAll(computer_, reinterpret_cast<const uint8_t*>(kGreeting), sizeof kGreeting - 1);
        return isGreeted && readAll(computer_, said, sizeof said) && std::memcmp(said, kSaidFirstByAComputer, sizeof said) == 0;
    }

    // False once the visit is over: the computer has gone, was refused, or has an app to be run.
    bool takeWhatIsAsked() {
        uint8_t asked = 0;
        if (!readAll(computer_, &asked, 1)) return false;
        switch (static_cast<Asked>(asked)) {
            case Asked::Begin: return answer(begin());
            case Asked::File: return answer(takeAFile());
            case Asked::End: return answer(apps_.hasBegun() && apps_.end() ? Answer::Done : Answer::NotWritten);
            case Asked::Run: return answer(findTheAppToRun()) && false;
        }
        return false;
    }

    // Whatever is refused ends the visit, once the computer has been told so.
    bool answer(Answer given) {
        const uint8_t sent = static_cast<uint8_t>(given);
        return writeAll(computer_, &sent, 1) && given == Answer::Done;
    }

    bool readNumber(size_t byteCount, uint32_t& number) {
        uint8_t bytes[4] = {};
        if (!readAll(computer_, bytes, byteCount)) return false;
        number = bytes[0] | static_cast<uint32_t>(bytes[1]) << 8 | static_cast<uint32_t>(bytes[2]) << 16 | static_cast<uint32_t>(bytes[3]) << 24;
        return true;
    }

    bool readWords(size_t bytesForTheLength, std::string& words) {
        uint32_t length = 0;
        if (!readNumber(bytesForTheLength, length) || length > 4 * kLongestName) return false;
        words.resize(length);
        return readAll(computer_, reinterpret_cast<uint8_t*>(&words[0]), length);
    }

    // Why an app of this name may not be written or run, or Done when it may.
    Answer mayHandle(const std::string& name) {
        if (!apps_.isOnACard()) return Answer::NotOnACard;
        if (!canNameAnApp(name)) return Answer::Refused;
        if (apps_.isTakenBySomethingElse(name)) return Answer::NameIsTaken;
        return musician_.allowsApps() ? Answer::Done : Answer::NotAllowed;
    }

    Answer begin() {
        std::string name;
        if (!readWords(1, name)) return Answer::Refused;
        const Answer allowed = mayHandle(name);
        if (allowed != Answer::Done) return allowed;
        musician_.isShownThatAnAppArrives(name);
        return apps_.begin(name) ? Answer::Done : Answer::NotWritten;
    }

    Answer takeAFile() {
        std::string path;
        uint32_t size = 0, sumSent = 0;
        if (!readWords(2, path) || !readNumber(4, size) || !readNumber(4, sumSent)) return Answer::Refused;
        if (!apps_.hasBegun() || !staysWithinItsApp(path) || size > kLargestFile) return Answer::Refused;
        const Answer written = write(path, size, sumSent);
        if (written != Answer::Done) apps_.takeAwayFile(path);
        return written;
    }

    // The bytes are read to their end even when the card stops taking them, so the answer finds the computer listening.
    Answer write(const std::string& path, uint32_t size, uint32_t sumSent) {
        const int file = apps_.openForWriting(path);
        std::vector<uint8_t> bytes(kBytesAtATime);
        Sum sum;
        bool isWritten = file >= 0;
        for (uint32_t left = size; left > 0;) {
            const size_t now = left < kBytesAtATime ? left : kBytesAtATime;
            if (!readAll(computer_, bytes.data(), now)) { closeIfOpen(file); return Answer::Damaged; }
            sum.add(bytes.data(), now);
            isWritten = isWritten && writeAll(file, bytes.data(), now);
            left -= static_cast<uint32_t>(now);
        }
        isWritten = isWritten && fsync(file) == 0;
        closeIfOpen(file);
        if (!isWritten) return Answer::NotWritten;
        return sum.value() == sumSent ? Answer::Done : Answer::Damaged;
    }

    static void closeIfOpen(int file) {
        if (file >= 0) close(file);
    }

    Answer findTheAppToRun() {
        std::string name;
        if (!readWords(1, name)) return Answer::Refused;
        const Answer allowed = mayHandle(name);
        if (allowed != Answer::Done) return allowed;
        if (!isThere(apps_.startScriptOf(name))) return Answer::NoSuchApp;
        scriptToRun_ = apps_.startScriptOf(name);
        return Answer::Done;
    }

    int computer_;
    FolderForApps& apps_;
    TheMusician& musician_;
    std::string scriptToRun_;
};

}  // namespace link_apps
