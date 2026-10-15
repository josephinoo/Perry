# Perry

Perry is an interactive Clojure development and nREPL companion extension for Zed.

It provides test runner diagnostics with assertion inlays and gutter status via `cider-nrepl`, project jack-in with automatic cider-nrepl middleware injection for Zed's native REPL, documentation inspection, and structural editing (Paredit). Zed natively handles Clojure REPL evaluation (via `repl::Run`).

## Installation

Perry requires the Zed editor fork supporting Extension API v0.8.0+ (Editor & UI API).

1. Open Zed.
2. Open the Command Palette (`cmd-shift-p` on macOS, `ctrl-shift-p` on Linux/Windows).
3. Run `zed: extensions`.
4. Click **Install Dev Extension**.
5. Select the `Perry` directory (`/Volumes/Workspace/Projects/Perry`).

## Suggested Keymaps
 
Add the following to your Zed `keymap.json` (`zed: open keymap`). Note that `ctrl-enter` is handled by Zed's native REPL (`repl::Run`), while Perry commands handle testing, Paredit, jack-in, doc, and namespace switching:

```json
[
  {
    "context": "Editor && (extension == clj || extension == cljs || extension == cljc || extension == edn || extension == bb)",
    "bindings": {
      "ctrl-enter": "repl::Run",
      "ctrl-alt-c ctrl-alt-j": ["zed::RunExtensionCommand", { "extension": "perry", "command": "jack-in" }],
      "ctrl-alt-c enter": ["zed::RunExtensionCommand", { "extension": "perry", "command": "load-file" }],
      "ctrl-alt-c ctrl-alt-n": ["zed::RunExtensionCommand", { "extension": "perry", "command": "switch-ns" }],
      "ctrl-alt-c d": ["zed::RunExtensionCommand", { "extension": "perry", "command": "show-doc" }],
      "ctrl-alt-c t": ["zed::RunExtensionCommand", { "extension": "perry", "command": "run-ns-tests" }],
      "ctrl-alt-c ctrl-alt-t": ["zed::RunExtensionCommand", { "extension": "perry", "command": "run-test-at-cursor" }],
      "ctrl-alt-c shift-t": ["zed::RunExtensionCommand", { "extension": "perry", "command": "run-all-tests" }],
      "ctrl-alt-c ctrl-alt-r": ["zed::RunExtensionCommand", { "extension": "perry", "command": "rerun-failed-tests" }],
      "ctrl-alt-c alt-l": ["zed::RunExtensionCommand", { "extension": "perry", "command": "clear-test-markers" }],
      "ctrl-alt-c right": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-slurp-forward" }],
      "ctrl-alt-c left": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-barf-forward" }],
      "ctrl-alt-c ctrl-left": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-slurp-backward" }],
      "ctrl-alt-c ctrl-right": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-barf-backward" }],
      "ctrl-alt-c ctrl-r": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-raise" }],
      "ctrl-alt-c ctrl-s": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-splice" }],
      "ctrl-alt-c (": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-wrap-round" }],
      "ctrl-alt-c [": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-wrap-square" }],
      "ctrl-alt-c {": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-wrap-curly" }],
      "ctrl-alt-c '": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-wrap-quote" }],
      "ctrl-alt-c /": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-split" }],
      "ctrl-alt-c \\": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-join" }],
      "ctrl-alt-c alt-s": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-splice-killing-forward" }],
      "ctrl-alt-c ctrl-alt-s": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-splice-killing-backward" }],
      "ctrl-alt-c k": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-kill" }],
      "ctrl-alt-c ctrl-k": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-kill-sexp" }],
      "ctrl-alt-c j": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-close-and-newline" }],
      "ctrl-alt-c f": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-forward" }],
      "ctrl-alt-c b": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-backward" }],
      "ctrl-alt-c u": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-up" }],
      "ctrl-alt-c d": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-down" }],
      "ctrl-alt-c ctrl-d": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-forward-down" }],
      "ctrl-alt-c ctrl-alt-d": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-backward-down" }],
      "ctrl-alt-c ctrl-u": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-forward-up" }],
      "ctrl-alt-c ctrl-alt-u": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-backward-up" }],
      "ctrl-alt-c .": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-thread-first" }],
      "ctrl-alt-c ,": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-thread-last" }],
      "ctrl-alt-c m": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-unthread" }],
      "ctrl-alt-c x": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-cycle-collection" }],
      "ctrl-alt-c up": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-form-up" }],
      "ctrl-alt-c down": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-move-form-down" }],
      "backspace": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-balanced-backspace" }],
      "delete": ["zed::RunExtensionCommand", { "extension": "perry", "command": "paredit-balanced-delete" }]
    }
  }
]
```

## Features

- **Jack-In**: Automatically detects Clojure CLI (`deps.edn`), Leiningen (`project.clj`), Babashka (`bb.edn`), or `shadow-cljs.edn`, injects `cider-nrepl`, spawns the server, and notifies when the port is ready for Zed's native REPL (`repl: refresh kernelspecs`).
- **Test Runner**: Runs namespace tests and test at cursor via `cider-nrepl`. Displays test failure inlays, green gutter marks for passes, and summary toasts.
- **Documentation & File Ops**: Inspect documentation (`show-doc`), load active buffer into REPL (`load-file`), and switch active namespace (`switch-ns`).
- **Structural Editing (Paredit)**: Pure text-based Clojure tree parser supporting slurp, barf, raise, splice, splice killing, wrap, split, join, kill, kill sexp, close-and-newline, structural movement, thread/unthread, cycle collection, drag form up/down, and balanced backspace/delete.
