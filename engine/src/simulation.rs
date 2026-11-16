//! The host simulator: the conditions a plugin is tested under, and how its report is read.

/// One set of conditions the MPC could put a plugin under.
#[derive(Debug, Clone, Copy)]
pub struct Session {
    pub name: &'static str,
    pub sample_rate: u32,
    pub block_size: u32,
    pub seconds: u32,
    pub transport_playing: bool,
    pub seed: u32,
    /// The MIDI notes played to an instrument, separated by commas. Effects are sent no notes.
    pub notes: &'static str,
}

impl Session {
    pub const EVERYDAY: Session = Session {
        name: "everyday use",
        sample_rate: 44_100,
        block_size: 128,
        seconds: 2,
        transport_playing: false,
        seed: 1234,
        notes: "60",
    };

    /// Long enough to hear a sound develop and fade. Instruments are played a four-note chord.
    pub const PREVIEW: Session = Session { name: "a preview", seconds: 8, notes: "48,55,60,64", ..Session::EVERYDAY };

    pub fn simulator_arguments(&self) -> Vec<String> {
        let mut arguments = vec![
            "--notes".to_string(),
            self.notes.to_string(),
            "--sr".to_string(),
            self.sample_rate.to_string(),
            "--block".to_string(),
            self.block_size.to_string(),
            "--seconds".to_string(),
            self.seconds.to_string(),
            "--seed".to_string(),
            self.seed.to_string(),
        ];
        if self.transport_playing {
            arguments.push("--playing".to_string());
        }
        arguments
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Pass,
    Info,
    Warning,
    Failure,
    Crash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct SimulationReport {
    findings: Vec<Finding>,
}

impl SimulationReport {
    pub fn parse(simulator_output: &str) -> Self {
        let findings = simulator_output.lines().filter_map(parse_finding).collect();
        Self { findings }
    }

    pub fn failures(&self) -> Vec<String> {
        self.messages_where(|severity| matches!(severity, Severity::Failure | Severity::Crash))
    }

    pub fn warnings(&self) -> Vec<String> {
        self.messages_where(|severity| severity == Severity::Warning)
    }

    fn messages_where(&self, wanted: impl Fn(Severity) -> bool) -> Vec<String> {
        self.findings
            .iter()
            .filter(|finding| wanted(finding.severity))
            .map(|finding| finding.message.clone())
            .collect()
    }
}

fn parse_finding(line: &str) -> Option<Finding> {
    let (label, message) = line.split_once(char::is_whitespace)?;
    let severity = match label {
        "PASS" => Severity::Pass,
        "INFO" => Severity::Info,
        "WARN" => Severity::Warning,
        "FAIL" => Severity::Failure,
        "CRASH" => Severity::Crash,
        _ => return None,
    };
    Some(Finding { severity, message: message.trim().to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = "\
== Render ==
INFO  main pass: 690 blocks, peak -7.70 dBFS
PASS  effect produced output from the test signal
WARN  clipping in the main pass (peak +2.1 dBFS)
FAIL  main pass: NaN in the output
CRASH SIGSEGV phase: render (main pass)

Result: FAIL  (1 passed, 1 warnings, 1 failures)
";

    #[test]
    fn failures_include_fails_and_crashes() {
        let report = SimulationReport::parse(REPORT);

        assert_eq!(
            report.failures(),
            ["main pass: NaN in the output", "SIGSEGV phase: render (main pass)"]
        );
    }

    #[test]
    fn warnings_are_kept_apart_from_failures() {
        let report = SimulationReport::parse(REPORT);

        assert_eq!(report.warnings(), ["clipping in the main pass (peak +2.1 dBFS)"]);
    }

    #[test]
    fn headings_and_the_result_line_are_not_findings() {
        let report = SimulationReport::parse("== Render ==\nResult: PASS  (20 passed, 0 warnings, 0 failures)\n");

        assert!(report.failures().is_empty());
        assert!(report.warnings().is_empty());
    }

    #[test]
    fn the_notes_to_play_are_passed_to_the_simulator() {
        let chord = Session { notes: "48,55,60", ..Session::EVERYDAY };

        let arguments = chord.simulator_arguments();

        assert_eq!(arguments[..2], ["--notes".to_string(), "48,55,60".to_string()]);
    }

    #[test]
    fn a_playing_session_asks_the_simulator_to_run_the_transport() {
        let playing = Session { transport_playing: true, ..Session::EVERYDAY };

        assert!(playing.simulator_arguments().contains(&"--playing".to_string()));
        assert!(!Session::EVERYDAY.simulator_arguments().contains(&"--playing".to_string()));
    }
}
