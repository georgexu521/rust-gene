use super::*;

pub(crate) fn prepare_native_lab_recovery_smoke_project(project: &Path, log_path: &Path) {
    let session_id = Some("native-lab-recovery-smoke".to_string());
    let proposal_output = priority_agent::lab::commands::handle_lab_command(
        project,
        session_id.clone(),
        "propose Desktop LabRun recovery smoke: verify paused run recovery, report previews, and full report paging in the desktop workbench.",
    );
    let Some(proposal_id) = parse_native_lab_proposal_id(&proposal_output) else {
        let _ = append_desktop_log(
            log_path,
            &format!(
                "native_lab_recovery_project prepared=false step=propose output={}",
                sanitize_log_value(&proposal_output)
            ),
        );
        return;
    };
    let approve_output = priority_agent::lab::commands::handle_lab_command(
        project,
        session_id.clone(),
        &format!("approve {proposal_id}"),
    );
    let plan_output = priority_agent::lab::commands::handle_lab_command(
        project,
        session_id.clone(),
        "plan Desktop LabRun recovery smoke plan. Success means desktop can recover a paused LabRun, show artifact reports, and page through markdown report content.",
    );
    let meeting_output = priority_agent::lab::commands::handle_lab_command(
        project,
        session_id.clone(),
        "meeting Desktop LabRun recovery smoke meeting: confirm paused project recovery and report inspection remain visible after app startup.",
    );
    let pause_output = priority_agent::lab::commands::handle_lab_command(
        project,
        session_id.clone(),
        "pause app_shutdown",
    );
    let recovery_output =
        priority_agent::lab::commands::handle_lab_command(project, session_id, "recovery");
    let prepared = approve_output.contains("LabRun created")
        && plan_output.contains("Created")
        && meeting_output.contains("Lab meeting")
        && pause_output.contains("Paused LabRun")
        && recovery_output.contains("Recovery: available");
    let _ = append_desktop_log(
        log_path,
        &format!(
            "native_lab_recovery_project prepared={} proposal={} approve={} plan={} meeting={} pause={} recovery={}",
            prepared,
            proposal_id,
            sanitize_log_value(&approve_output),
            sanitize_log_value(&plan_output),
            sanitize_log_value(&meeting_output),
            sanitize_log_value(&pause_output),
            sanitize_log_value(&recovery_output)
        ),
    );
}

fn parse_native_lab_proposal_id(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Lab proposal created: ")
            .map(str::trim)
            .filter(|proposal_id| !proposal_id.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn schedule_native_interaction_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_interaction_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_interaction_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_live_provider_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_live_provider_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_live_provider_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_multitool_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_multitool_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_multitool_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_soak_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_soak_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_soak_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_extended_soak_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_extended_soak_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_extended_soak_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_soak_restart_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_soak_restart_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_soak_restart_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_extended_soak_restart_smoke(
    window: WebviewWindow,
    log_path: PathBuf,
) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_extended_soak_restart_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_extended_soak_restart_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_lab_recovery_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_lab_recovery_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_lab_recovery_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn schedule_native_restart_smoke(window: WebviewWindow, log_path: PathBuf) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let result = window.eval(native_restart_smoke_script());
        if let Err(err) = result {
            let _ = append_desktop_log(
                &log_path,
                &format!(
                    "native_restart_smoke ok=false eval_error={}",
                    sanitize_log_value(&err.to_string())
                ),
            );
        }
    });
}

pub(crate) fn native_interaction_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const candidates = () => Array.from(document.querySelectorAll("button, [role='button'], [aria-label]"));
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && visible(element));
  const byEnabledLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && !element.disabled && visible(element));
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const byTextIncludes = (label) => buttonCandidates().find((element) => element.textContent?.trim().includes(label) && visible(element));
  const setTextareaValue = (label, value) => {
    const element = document.querySelector(`textarea[aria-label="${label}"]`);
    if (!element) {
      throw new Error(`missing textarea ${label}`);
    }
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    steps.push(`typed-${label}`);
  };
  const setInputValue = (label, value) => {
    const element = document.querySelector(`input[aria-label="${label}"]`);
    if (!element) {
      throw new Error(`missing input ${label}`);
    }
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    steps.push(`typed-${label}`);
  };
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
    };
    const waitFor = async (name, predicate) => {
    for (let index = 0; index < 30; index += 1) {
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(200);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    await waitFor("app-ready", () => text().includes("What should we build in rust-agent?"));
    await sleep(500);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");
    await click("settings-open", () => byText("Settings"));
    await waitFor("settings-visible", () => document.querySelector("[aria-label='Settings']"));
    await click("settings-provider-open", () => byText("Provider"));
    await waitFor("settings-provider-visible", () =>
      text().includes("Provider setup") &&
      (text().includes("Provider key required") || text().includes("Provider is configured"))
    );
    await click("settings-close", () => byTextIncludes("Back to app"));
    await waitFor("settings-closed", () => !document.querySelector("[aria-label='Settings']"));
    await click("labrun-tab-open", () => byText("LabRun"));
    await waitFor("labrun-visible", () =>
      text().includes("Proposal intake") &&
      text().includes("Project controls") &&
      text().includes("Reports and artifacts")
    );
    setInputValue("Search LabRun artifacts", "lab");
    await waitFor("labrun-search-visible", () => text().includes("Reports and artifacts"));
    setInputValue("Search LabRun artifacts", "");
    await click("execution-tab-open", () => byText("Execution"));
    await waitFor("execution-visible", () =>
      text().includes("Trace evidence") &&
      text().includes("Stored output")
    );
    await click("context-menu-open", () => byLabel("Add context"));
    await waitFor("context-menu-visible", () => text().includes("Add context") && text().includes("Current diff"));
    await click("current-diff-add", () => byLabel("Reference current diff"));
    await waitFor("context-chip-visible", () => Boolean(byLabel("Open context Current diff")));
    await click("context-detail-open", () => byLabel("Open context Current diff"));
    await waitFor("context-detail-visible", () => document.querySelector("[aria-label='Context details']"));
    await click("context-detail-close", () => byLabel("Close context details"));
    await waitFor("context-detail-closed", () => !document.querySelector("[aria-label='Context details']"));
    await click("trace-open", () => byText("Trace"));
    await waitFor("trace-visible", () => document.querySelector("[aria-label='Run trace']"));
    await click("trace-close", () => byText("Close"));
    await waitFor("trace-closed", () => !document.querySelector("[aria-label='Run trace']"));
    setTextareaValue("Message", "Native smoke real run");
    await waitFor("send-enabled", () => Boolean(byEnabledLabel("Send message")));
    await click("run-submit", () => byEnabledLabel("Send message"));
    await waitFor("run-started", () =>
      text().includes("Runtime connected") ||
      text().includes("Priority Agent running") ||
      text().includes("Working")
    );
    await waitFor("shell-card-visible", () => text().includes("scripts/desktop-native-smoke.sh --fixture-run"));
    await waitFor("file-card-visible", () => text().includes("Edited file") && text().includes("Composer.tsx"));
    await waitFor("permission-waiting", () => text().includes("Permission needed: bash") && Boolean(byText("Approve")));
    await click("permission-approve", () => byText("Approve"));
    await waitFor("permission-approved", () => text().includes("Permission approved"));
    await waitFor("assistant-answer-visible", () => text().includes("Native smoke fixture completed"));
    await waitFor("assistant-final", () => Boolean(document.querySelector(".message.assistant.final")));
    await waitFor("run-completed", () =>
      text().includes("Run completed") ||
      text().includes("Done") ||
      text().includes("Priority Agent idle")
    );
    await waitFor("usage-visible", () =>
      text().includes("Token usage") ||
      /Context\s+\d+%/.test(text())
    );
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after run");
    }
    return await record(`native_interaction_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_interaction_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_live_provider_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && visible(element));
  const byEnabledLabel = (label) => candidates().find((element) => element.getAttribute("aria-label") === label && !element.disabled && visible(element));
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const setTextareaValue = (label, value) => {
    const element = document.querySelector(`textarea[aria-label="${label}"]`);
    if (!element) {
      throw new Error(`missing textarea ${label}`);
    }
    const setter = Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    steps.push(`typed-${label}`);
  };
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
  };
  const waitFor = async (name, predicate, attempts = 180, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    await waitFor("app-ready", () => text().includes("What should we build in rust-agent?"), 60, 250);
    await sleep(500);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");
    await click("labrun-tab-open", () => byText("LabRun"));
    await waitFor("labrun-visible", () =>
      text().includes("Proposal intake") &&
      text().includes("Project controls") &&
      text().includes("Reports and artifacts"),
      60,
      250
    );
    await click("execution-tab-open", () => byText("Execution"));
    await waitFor("execution-visible", () =>
      text().includes("Trace evidence") &&
      text().includes("Stored output"),
      60,
      250
    );
    setTextareaValue("Message", "Desktop live provider QA. Reply with exactly: desktop live qa ok. Do not call tools.");
    await waitFor("send-enabled", () => Boolean(byEnabledLabel("Send message")), 60, 250);
    await click("run-submit", () => byEnabledLabel("Send message"));
    await waitFor("run-started", () =>
      text().includes("Runtime connected") ||
      text().includes("Priority Agent running") ||
      text().includes("Working"),
      120,
      500
    );
    await waitFor("assistant-live-answer-visible", () =>
      text().toLowerCase().includes("desktop live qa ok"),
      180,
      500
    );
    await waitFor("assistant-final", () => Boolean(document.querySelector(".message.assistant.final")), 120, 500);
    await waitFor("run-completed", () =>
      text().includes("Run completed") ||
      text().includes("Done") ||
      text().includes("Priority Agent idle"),
      120,
      500
    );
    await waitFor("usage-visible", () =>
      text().includes("Token usage") ||
      /Context\s+\d+%/.test(text()),
      60,
      500
    );
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after live run");
    }
    return await record(`native_live_provider_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_live_provider_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_multitool_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const candidates = () => Array.from(document.querySelectorAll("button, [role='button'], [aria-label]"));
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
  };
  const waitFor = async (name, predicate, attempts = 240, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      const approve = byText("Approve");
      if (approve && text().includes("Permission needed")) {
        approve.click();
        steps.push("permission-approved");
        await sleep(350);
      }
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    await waitFor("app-ready", () => text().includes("What should we build in") || text().includes("Ask Liz to inspect"), 60, 250);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");
    await click("execution-tab-open", () => byText("Execution"));
    await waitFor("execution-visible", () =>
      text().includes("Trace evidence") &&
      text().includes("Stored output"),
      60,
      250
    );
    setTextareaValue("Message", [
      "Desktop multi-tool QA. Use tools, not just text.",
      "In this project, replace the entire contents of qa_target.txt with exactly:",
      "desktop tool qa ok",
      "Then run `cat qa_target.txt` to verify it.",
      "Finish with a concise final answer containing exactly: desktop tool qa ok"
    ].join("\n"));
    await waitFor("send-enabled", () => Boolean(byLabel("Send message")), 60, 250);
    await click("run-submit", () => byLabel("Send message"));
    await waitFor("run-started", () =>
      text().includes("Runtime connected") ||
      text().includes("Priority Agent running") ||
      text().includes("Working"),
      120,
      500
    );
    await waitFor("tool-visible", () =>
      text().includes("Tool") ||
      text().includes("bash") ||
      text().includes("file_") ||
      text().includes("Edited file"),
      240,
      500
    );
    await sleep(2000);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after multitool run");
    }
    return await record(`native_multitool_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_multitool_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_soak_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
  };
  const waitFor = async (name, predicate, attempts = 300, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      const approve = byText("Approve");
      if (approve && text().includes("Permission needed")) {
        approve.click();
        steps.push("permission-approved");
        await sleep(350);
      }
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };
  const submitTask = async (name, message) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    steps.push(`${name}-invoke-start`);
    await window.__TAURI_INTERNALS__.invoke("send_message", { contexts: [], message });
    steps.push(`${name}-invoke-complete`);
  };
  const previewFile = async (path) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    return await window.__TAURI_INTERNALS__.invoke("desktop_file_preview", { path, limit: 4096 });
  };
  const expectFile = async (path, expected) => {
    const preview = await previewFile(path);
    const actual = (preview?.content || "").trim();
    if (actual !== expected) {
      throw new Error(`${path} content mismatch: ${actual}`);
    }
    steps.push(`${path}-verified`);
    return actual;
  };
  try {
    await waitFor("app-ready", () => text().includes("What should we build in") || text().includes("Ask Liz to inspect"), 60, 250);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");
    await click("execution-tab-open", () => byText("Execution"));
    await waitFor("execution-visible", () =>
      text().includes("Trace evidence") &&
      text().includes("Stored output"),
      60,
      250
    );
    await submitTask("first", [
      "Desktop soak QA turn 1. This is a file-edit task, not a text answer.",
      "You must use tools in this turn: read qa_target.txt, write qa_target.txt, then run cat qa_target.txt.",
      "In this project, replace the entire contents of qa_target.txt with exactly:",
      "desktop tool qa ok",
      "Then run `cat qa_target.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop tool qa ok"
    ].join("\n"));
    await submitTask("second", [
      "Desktop soak QA turn 2. This is another file-edit task, not a text answer.",
      "You must use tools again in the same desktop session: read qa_followup.txt, write qa_followup.txt, then run cat qa_followup.txt.",
      "Replace the entire contents of qa_followup.txt with exactly:",
      "desktop soak qa ok",
      "Then run `cat qa_followup.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop soak qa ok"
    ].join("\n"));
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after soak run");
    }
    return await record(`native_soak_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_soak_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_extended_soak_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
  };
  const waitFor = async (name, predicate, attempts = 360, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      const approve = byText("Approve");
      if (approve && text().includes("Permission needed")) {
        approve.click();
        steps.push("permission-approved");
        await sleep(350);
      }
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };
  const submitTask = async (name, message) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    steps.push(`${name}-invoke-start`);
    await window.__TAURI_INTERNALS__.invoke("send_message", { contexts: [], message });
    steps.push(`${name}-invoke-complete`);
  };
  const previewFile = async (path) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    return await window.__TAURI_INTERNALS__.invoke("desktop_file_preview", { path, limit: 4096 });
  };
  const expectFile = async (path, expected) => {
    const preview = await previewFile(path);
    const actual = (preview?.content || "").trim();
    if (actual !== expected) {
      throw new Error(`${path} content mismatch: ${actual}`);
    }
    steps.push(`${path}-verified`);
    return actual;
  };
  const repairExpectedFile = async (name, path, expected, message) => {
    try {
      return await expectFile(path, expected);
    } catch (firstError) {
      steps.push(`${name}-repair-needed`);
      await submitTask(`${name}-repair`, message);
      return await expectFile(path, expected);
    }
  };

  try {
    await waitFor("app-ready", () => text().includes("What should we build in") || text().includes("Ask Liz to inspect"), 60, 250);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");
    await click("execution-tab-open", () => byText("Execution"));
    await waitFor("execution-visible", () =>
      text().includes("Trace evidence") &&
      text().includes("Stored output"),
      60,
      250
    );
    await submitTask("first", [
      "Desktop soak QA turn 1. This is a file-edit task, not a text answer.",
      "You must use tools in this turn: read qa_target.txt, write qa_target.txt, then run cat qa_target.txt.",
      "In this project, replace the entire contents of qa_target.txt with exactly:",
      "desktop tool qa ok",
      "Then run `cat qa_target.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop tool qa ok"
    ].join("\n"));
    await expectFile("qa_target.txt", "desktop tool qa ok");
    await expectFile("qa_followup.txt", "pending");
    await expectFile("qa_third.txt", "waiting");
    await submitTask("second", [
      "Desktop soak QA turn 2. This is another file-edit task, not a text answer.",
      "You must use tools again in the same desktop session: read qa_followup.txt, write qa_followup.txt, then run cat qa_followup.txt.",
      "Replace the entire contents of qa_followup.txt with exactly:",
      "desktop soak qa ok",
      "Then run `cat qa_followup.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop soak qa ok"
    ].join("\n"));
    await expectFile("qa_target.txt", "desktop tool qa ok");
    await expectFile("qa_followup.txt", "desktop soak qa ok");
    await expectFile("qa_third.txt", "waiting");
    await submitTask("third", [
      "Desktop soak QA turn 3. This is a third file-edit task, not a text answer.",
      "You must use tools again after two previous turns: read qa_third.txt, write qa_third.txt, then run cat qa_third.txt.",
      "Replace the entire contents of qa_third.txt with exactly:",
      "desktop extended soak qa ok",
      "Then run `cat qa_third.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop extended soak qa ok"
    ].join("\n"));
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after extended soak run");
    }
    await expectFile("qa_target.txt", "desktop tool qa ok");
    await expectFile("qa_followup.txt", "desktop soak qa ok");
    await repairExpectedFile("third", "qa_third.txt", "desktop extended soak qa ok", [
      "Desktop soak QA turn 3 repair. The previous turn did not produce file evidence.",
      "This is still a file-edit task, not a text answer.",
      "You must use tools now: read qa_third.txt, write qa_third.txt, then run cat qa_third.txt.",
      "Replace the entire contents of qa_third.txt with exactly:",
      "desktop extended soak qa ok",
      "Then run `cat qa_third.txt` to verify it.",
      "Do not say the task is done unless the file tool and cat command have actually run.",
      "Finish with a concise final answer containing exactly: desktop extended soak qa ok"
    ].join("\n"));
    steps.push("project-files-verified");
    return await record(`native_extended_soak_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_extended_soak_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_soak_restart_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const waitFor = async (name, predicate, attempts = 120, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      if (await predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };
  const invoke = async (name, args = {}) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    return await window.__TAURI_INTERNALS__.invoke(name, args);
  };

  try {
    await waitFor("app-ready", async () =>
      text().includes("Ask Liz to inspect") ||
      text().includes("What should we build in") ||
      text().includes("Restored session") ||
      text().includes("Priority Agent"),
      60,
      250
    );
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after soak restart");
    }
    steps.push("no-stale-session-error");

    let sessions = [];
    await waitFor("recent-session-visible", async () => {
      sessions = await invoke("list_recent_sessions", { limit: 5 });
      return Array.isArray(sessions) && sessions.length > 0;
    }, 60, 500);
    const latest = sessions[0];
    const messages = await invoke("load_session_messages", { sessionId: latest.id });
    const joinedMessages = (messages || []).map((message) => message.content || "").join("\n").toLowerCase();
    if (!joinedMessages.includes("desktop soak qa turn 1") || !joinedMessages.includes("desktop soak qa turn 2")) {
      throw new Error("restored session is missing soak user turns");
    }
    if (!joinedMessages.includes("desktop tool qa ok") || !joinedMessages.includes("desktop soak qa ok")) {
      throw new Error("restored session is missing soak final answers");
    }
    steps.push("session-messages-verified");

    const firstFile = await invoke("desktop_file_preview", { path: "qa_target.txt", limit: 4096 });
    const secondFile = await invoke("desktop_file_preview", { path: "qa_followup.txt", limit: 4096 });
    if ((firstFile?.content || "").trim() !== "desktop tool qa ok") {
      throw new Error(`qa_target.txt content mismatch: ${(firstFile?.content || "").trim()}`);
    }
    if ((secondFile?.content || "").trim() !== "desktop soak qa ok") {
      throw new Error(`qa_followup.txt content mismatch: ${(secondFile?.content || "").trim()}`);
    }
    steps.push("project-files-verified");

    await waitFor("restored-ui-visible", async () =>
      text().toLowerCase().includes("desktop tool qa ok") &&
      text().toLowerCase().includes("desktop soak qa ok"),
      120,
      500
    );

    return await record(`native_soak_restart_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_soak_restart_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_extended_soak_restart_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const waitFor = async (name, predicate, attempts = 120, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      if (await predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };
  const invoke = async (name, args = {}) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    return await window.__TAURI_INTERNALS__.invoke(name, args);
  };

  try {
    await waitFor("app-ready", async () =>
      text().includes("Ask Liz to inspect") ||
      text().includes("What should we build in") ||
      text().includes("Restored session") ||
      text().includes("Priority Agent"),
      60,
      250
    );
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after extended soak restart");
    }
    steps.push("no-stale-session-error");

    let sessions = [];
    await waitFor("recent-session-visible", async () => {
      sessions = await invoke("list_recent_sessions", { limit: 5 });
      return Array.isArray(sessions) && sessions.length > 0;
    }, 60, 500);
    const latest = sessions[0];
    const messages = await invoke("load_session_messages", { sessionId: latest.id });
    const joinedMessages = (messages || []).map((message) => message.content || "").join("\n").toLowerCase();
    if (
      !joinedMessages.includes("desktop soak qa turn 1") ||
      !joinedMessages.includes("desktop soak qa turn 2") ||
      !joinedMessages.includes("desktop soak qa turn 3")
    ) {
      throw new Error("restored session is missing extended soak user turns");
    }
    if (
      !joinedMessages.includes("desktop tool qa ok") ||
      !joinedMessages.includes("desktop soak qa ok") ||
      !joinedMessages.includes("desktop extended soak qa ok")
    ) {
      throw new Error("restored session is missing extended soak final answers");
    }
    steps.push("session-messages-verified");

    const firstFile = await invoke("desktop_file_preview", { path: "qa_target.txt", limit: 4096 });
    const secondFile = await invoke("desktop_file_preview", { path: "qa_followup.txt", limit: 4096 });
    const thirdFile = await invoke("desktop_file_preview", { path: "qa_third.txt", limit: 4096 });
    if ((firstFile?.content || "").trim() !== "desktop tool qa ok") {
      throw new Error(`qa_target.txt content mismatch: ${(firstFile?.content || "").trim()}`);
    }
    if ((secondFile?.content || "").trim() !== "desktop soak qa ok") {
      throw new Error(`qa_followup.txt content mismatch: ${(secondFile?.content || "").trim()}`);
    }
    if ((thirdFile?.content || "").trim() !== "desktop extended soak qa ok") {
      throw new Error(`qa_third.txt content mismatch: ${(thirdFile?.content || "").trim()}`);
    }
    steps.push("project-files-verified");

    await waitFor("restored-ui-visible", async () =>
      text().toLowerCase().includes("desktop tool qa ok") &&
      text().toLowerCase().includes("desktop soak qa ok") &&
      text().toLowerCase().includes("desktop extended soak qa ok"),
      120,
      500
    );

    return await record(`native_extended_soak_restart_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_extended_soak_restart_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_lab_recovery_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const buttonCandidates = () => Array.from(document.querySelectorAll("button, [role='button']"));
  const visible = (element) => {
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  };
  const byText = (label) => buttonCandidates().find((element) => element.textContent?.trim() === label && visible(element));
  const byTextIncludes = (label) => buttonCandidates().find((element) => element.textContent?.trim().includes(label) && visible(element));
  const setInputValue = (label, value) => {
    const element = document.querySelector(`input[aria-label="${label}"]`);
    if (!element) {
      throw new Error(`missing input ${label}`);
    }
    const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
    steps.push(`typed-${label}`);
  };
  const click = async (name, findElement) => {
    const element = findElement();
    if (!element) {
      throw new Error(`missing ${name}`);
    }
    element.click();
    steps.push(name);
    await sleep(350);
  };
  const waitFor = async (name, predicate, attempts = 120, delay = 250) => {
    for (let index = 0; index < attempts; index += 1) {
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      throw new Error("missing tauri invoke");
    }
    await waitFor("app-ready", () => text().includes("What should we build in") || text().includes("Ask Liz to inspect"));
    if (text().includes("session not found")) {
      throw new Error("stale session error visible");
    }
    steps.push("no-stale-session-error");

    const snapshot = await window.__TAURI_INTERNALS__.invoke("desktop_workbench_snapshot");
    const lab = snapshot?.lab_status;
    if (!lab?.available || lab.state !== "run") {
      throw new Error(`missing LabRun snapshot: ${JSON.stringify(lab)}`);
    }
    if (lab.run_status !== "Paused") {
      throw new Error(`expected paused LabRun, got ${lab.run_status}`);
    }
    if ((lab.artifact_count || 0) < 2 || (lab.meeting_count || 0) < 1) {
      throw new Error(`expected artifacts and meeting, got artifacts=${lab.artifact_count} meetings=${lab.meeting_count}`);
    }
    if (!lab.latest_report_path || !Array.isArray(lab.reports) || lab.reports.length < 2) {
      throw new Error("expected LabRun reports and latest report path");
    }
    steps.push("snapshot-verified");

    const report = await window.__TAURI_INTERNALS__.invoke("desktop_lab_report_page", {
      path: lab.latest_report_path,
      offset: 0,
      limit: 4096,
    });
    if (!report?.content?.includes("Desktop LabRun recovery smoke")) {
      throw new Error("latest LabRun report page did not include recovery smoke topic");
    }
    steps.push("report-page-verified");

    await click("labrun-tab-open", () => byText("LabRun"));
    await waitFor("labrun-visible", () =>
      text().includes("Reports and artifacts") &&
      text().includes("Paused") &&
      text().includes("Desktop LabRun recovery smoke")
    );
    setInputValue("Search LabRun artifacts", "recovery smoke");
    await waitFor("labrun-search-visible", () => text().includes("Desktop LabRun recovery smoke"));
    await click("report-preview-open", () => byTextIncludes("Preview full report"));
    await waitFor("full-report-visible", () =>
      text().includes("Full report viewer") &&
      text().includes("Desktop LabRun recovery smoke")
    );

    return await record(`native_lab_recovery_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_lab_recovery_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}

pub(crate) fn native_restart_smoke_script() -> &'static str {
    r#"
(async () => {
  const steps = [];
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const text = () => document.body?.innerText || "";
  const waitFor = async (name, predicate, attempts = 120, delay = 500) => {
    for (let index = 0; index < attempts; index += 1) {
      if (predicate()) {
        steps.push(name);
        return;
      }
      await sleep(delay);
    }
    throw new Error(`timeout ${name}`);
  };
  const record = async (result) => {
    if (!window.__TAURI_INTERNALS__?.invoke) {
      return result;
    }
    await window.__TAURI_INTERNALS__.invoke("record_native_smoke_result", { result });
    return result;
  };

  try {
    await waitFor("app-ready", () => text().includes("Ask Liz to inspect") || text().includes("What should we build in rust-agent?"), 60, 250);
    if (text().includes("session not found")) {
      throw new Error("stale session error visible after restart");
    }
    steps.push("no-stale-session-error");
    await waitFor("restored-user-message", () =>
      text().includes("Desktop live provider QA. Reply with exactly:"),
      120,
      500
    );
    await waitFor("restored-assistant-answer", () =>
      text().toLowerCase().includes("desktop live qa ok"),
      120,
      500
    );
    await waitFor("restored-session-metadata", () =>
      text().includes("messages") || text().includes("msgs"),
      60,
      500
    );
    return await record(`native_restart_smoke ok=true steps=${steps.join(",")}`);
  } catch (error) {
    return await record(`native_restart_smoke ok=false error=${error?.message || error} steps=${steps.join(",")} text=${text().slice(0, 500)}`);
  }
})()
"#
}
