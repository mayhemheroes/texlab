use std::{
    fs, io,
    path::Path,
    process::{Command, Stdio},
};

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};
use once_cell::sync::Lazy;
use regex::Regex;
use tempfile::tempdir;

use crate::RangeExt;

static LINE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new("(\\d+):(\\d+):(\\d+):(\\w+):(\\w+):(.*)").unwrap());

pub fn lint_with_chktex(text: &str, current_dir: &Path) -> io::Result<Vec<Diagnostic>> {
    let directory = tempdir()?;
    fs::write(directory.path().join("file.tex"), text)?;
    let _ = fs::copy(
        current_dir.join("chktexrc"),
        directory.path().join("chktexrc"),
    );

    let output = Command::new("chktex")
        .args(&["-I0", "-f%l:%c:%d:%k:%n:%m\n", "file.tex"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(directory.path())
        .output()?;

    let mut diagnostics = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let captures = LINE_REGEX.captures(line).unwrap();
        let line = captures[1].parse::<u32>().unwrap() - 1;
        let character = captures[2].parse::<u32>().unwrap() - 1;
        let digit = captures[3].parse::<u32>().unwrap();
        let kind = &captures[4];
        let code = &captures[5];
        let message = captures[6].into();
        let range = Range::new_simple(line, character, line, character + digit);
        let severity = match kind {
            "Message" => DiagnosticSeverity::INFORMATION,
            "Warning" => DiagnosticSeverity::WARNING,
            _ => DiagnosticSeverity::ERROR,
        };

        diagnostics.push(Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(code.into())),
            code_description: None,
            source: Some("chktex".into()),
            message,
            related_information: None,
            tags: None,
            data: None,
        });
    }

    Ok(diagnostics)
}
