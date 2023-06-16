mod osc {
    use std::process::{Command, Stdio};

    trait Terminal {
        fn supported() -> bool;
        fn cmd() -> Command;
    }

    macro_rules! term {
        ($name:ident, $plat:meta , $cmd:literal, $( $arg:literal),* $(,)?) => {
            struct $name;

            impl Terminal for $name {
                fn supported() -> bool {
                    #[cfg($plat)]
                    { true }
                    #[cfg(not($plat))]
                    { false }
                }

                fn cmd() -> Command {
                    // Only a few invocations of the term macro don't supply
                    // extra args so most need mutability. Silence the compiler.
                    #[allow(unused_mut)]
                    let mut cmd = Command::new($cmd);
                    $( cmd.arg($arg); )*
                    cmd
                }
            }
        };
    }

    // TODO Not quite working either
    term!(TerminalExe, target_family = "windows", "wt.exe",);

    // From: https://ss64.com/osx/open.html
    /* Mac Terminal.app commented out because doesn't quite work
    term!(
        TerminalApp,
        target_os = "macos",
        "open",
        "-n",
        "-F",
        "-W",
        "-a",
        "Terminal.app",
        // Alternative: "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal",
        // Alternative: "/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal",
    );

    // Wildly untested AppleScript chunk. I've seen all except the "in a"
    // parts in various AppleScript examples
    const APPLESCRIPT: &str = r#"
        tell application "Terminal"
            activate
            do script "/bin/zsh -c {}" in a new Terminal window
        end tell
    "#;

    term!(
        TerminalApp,
        target_os = "macos",
        "osascript",
        "-e",
        // TODO pass format!(APPLESCRIPT, which_example) here
    );
    */

    term!(
        GnomeTerminal,
        target_os = "linux",
        "gnome-terminal",
        "--wait",
        "--",
        "bash",
        "-c",
    );

    term!(
        XTerm,
        target_family = "unix",
        "xterm",
        "-hold",
        "-e",
        "bash",
        "-c",
    );

    term!(
        Rxvt,
        target_family = "unix",
        "rxvt",
        "-hold",
        "-e",
        "bash",
        "-c",
    );

    term!(
        ETerm,
        target_os = "linux",
        "Eterm",
        "--pause",
        "-e",
        "bash",
        "-c",
    );

    //term!(Alacritty, any, "~/.cargo/bin/alacritty", "-e", "/usr/bin/echo");

    #[allow(unused)]
    pub fn setup() {
        #[cfg(windows)]
        {
            nu_ansi_term::enable_ansi_support().unwrap();
        }

        let mut cmd = Command::new("cargo");
        cmd.stdin(Stdio::null());
        cmd.args(["build", "--examples"]);

        let status = cmd.status().expect("Failed to build examples");
        assert!(status.success());
    }

    macro_rules! test_one_term {
        ($term:ident) => {
            #[test]
            #[ignore]
            #[allow(non_snake_case)]
            fn $term() {
                if $term::supported() {
                    let mut cmd = $term::cmd();
                    cmd.stdin(Stdio::null());
                    cmd.arg(format!("cargo run --example {} -- --sleep 10000", EXAMPLE));

                    eprintln!("Running {:?} in {}", cmd, stringify!($term));
                    if let Ok(child) = cmd.spawn() {
                        let output = child
                            .wait_with_output()
                            .expect("Failed to wait for terminal");
                        assert!(output.status.success(), "{:?}", output);
                    }
                }
                // else we expect platform cannot run this
            }
        };
    }

    macro_rules! test_terms {
        ($( $term:ident ),+) => { $( test_one_term!($term); )+ }
    }

    macro_rules! test_all_terms {
        () => {
            test_terms!(TerminalExe, GnomeTerminal, XTerm, Rxvt, ETerm); //, TerminalApp);
        };
    }

    mod hyperlink {
        use super::*;
        const EXAMPLE: &str = "hyperlink";
        test_all_terms!();
    }

    mod title {
        use super::*;
        const EXAMPLE: &str = "title";
        test_all_terms!();
    }
} // mod osc
