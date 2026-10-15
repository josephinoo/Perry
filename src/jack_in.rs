use std::path::{Path, PathBuf};

pub const NREPL_VERSION: &str = "1.3.1";
pub const CIDER_NREPL_VERSION: &str = "0.52.1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectKind {
    ClojureCli,
    Leiningen,
    ShadowCljs,
    Babashka,
}

impl ProjectKind {
    pub fn label(self) -> &'static str {
        match self {
            ProjectKind::ClojureCli => "Clojure CLI (deps.edn)",
            ProjectKind::Leiningen => "Leiningen (project.clj)",
            ProjectKind::ShadowCljs => "shadow-cljs",
            ProjectKind::Babashka => "Babashka",
        }
    }

    pub fn marker_file(self) -> &'static str {
        match self {
            ProjectKind::ClojureCli => "deps.edn",
            ProjectKind::Leiningen => "project.clj",
            ProjectKind::ShadowCljs => "shadow-cljs.edn",
            ProjectKind::Babashka => "bb.edn",
        }
    }

    pub const ALL: [ProjectKind; 4] = [
        ProjectKind::ClojureCli,
        ProjectKind::Leiningen,
        ProjectKind::ShadowCljs,
        ProjectKind::Babashka,
    ];
}

pub fn detect_project_kind_with<F: Fn(&str) -> bool>(file_exists: F) -> Option<ProjectKind> {
    ProjectKind::ALL
        .into_iter()
        .find(|kind| file_exists(kind.marker_file()))
}

pub fn detect_project_kind() -> Option<ProjectKind> {
    detect_project_kind_with(|marker_file| {
        zed_extension_api::panel::read_active_worktree_file(marker_file).is_ok()
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JackInCommand {
    pub kind: ProjectKind,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub port: u16,
}

impl JackInCommand {
    pub fn new(kind: ProjectKind, root: &Path, port: u16) -> Self {
        let middleware_arg = "[cider.nrepl/cider-middleware]";

        let (program, args) = match kind {
            ProjectKind::ClojureCli => {
                let cli_args = vec![
                    "-Sdeps".to_string(),
                    format!(
                        "{{:deps {{nrepl/nrepl {{:mvn/version \"{NREPL_VERSION}\"}} \
                         cider/cider-nrepl {{:mvn/version \"{CIDER_NREPL_VERSION}\"}}}}}}"
                    ),
                    "-M".to_string(),
                    "-m".to_string(),
                    "nrepl.cmdline".to_string(),
                    "--port".to_string(),
                    port.to_string(),
                    "--middleware".to_string(),
                    middleware_arg.to_string(),
                ];
                ("clojure".to_string(), cli_args)
            }
            ProjectKind::Leiningen => {
                let lein_args = vec![
                    "update-in".to_string(),
                    ":plugins".to_string(),
                    "conj".to_string(),
                    format!("[cider/cider-nrepl \"{CIDER_NREPL_VERSION}\"]"),
                    "--".to_string(),
                    "repl".to_string(),
                    ":headless".to_string(),
                    ":port".to_string(),
                    port.to_string(),
                ];
                ("lein".to_string(), lein_args)
            }
            ProjectKind::ShadowCljs => {
                let shadow_args = vec![
                    "shadow-cljs".to_string(),
                    "-d".to_string(),
                    format!("nrepl/nrepl:{NREPL_VERSION}"),
                    "-d".to_string(),
                    format!("cider/cider-nrepl:{CIDER_NREPL_VERSION}"),
                    "server".to_string(),
                ];
                ("npx".to_string(), shadow_args)
            }
            ProjectKind::Babashka => {
                let bb_args = vec!["nrepl-server".to_string(), format!("localhost:{port}")];
                ("bb".to_string(), bb_args)
            }
        };

        Self {
            kind,
            program,
            args,
            cwd: root.to_path_buf(),
            port,
        }
    }
}

pub fn parse_port_from_output(text: &str) -> Option<u16> {
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();

        if let Some(pos) = lower.find("port ") {
            let after = &line[pos + 5..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(port) = digits.parse::<u16>() {
                if port > 0 {
                    return Some(port);
                }
            }
        }

        if let Some(pos) = lower.find("127.0.0.1:") {
            let after = &line[pos + "127.0.0.1:".len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(port) = digits.parse::<u16>() {
                if port > 0 {
                    return Some(port);
                }
            }
        }

        if let Some(pos) = lower.find("localhost:") {
            let after = &line[pos + "localhost:".len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(port) = digits.parse::<u16>() {
                if port > 0 {
                    return Some(port);
                }
            }
        }
    }
    None
}
