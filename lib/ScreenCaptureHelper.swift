import AppKit
import Foundation

/// Bootstrap NSApplication for CLI processes that need AppKit UI (e.g. the
/// SCContentSharingPicker). Must be called from the main thread.
///
/// Idempotent across repeated calls — if NSApp is already running it only
/// re-applies the activation policy and brings the app forward.
@_cdecl("screen_capture_helper_bootstrap_nsapp")
public func screenCaptureHelperBootstrapNsapp() -> Bool {
    guard Thread.isMainThread else {
        fputs("screen_capture_helper: bootstrap_nsapp must be called on the main thread\n", stderr)
        return false
    }

    let app = NSApplication.shared
    if !app.isRunning {
        app.setActivationPolicy(.regular)
        app.finishLaunching()
    } else {
        app.setActivationPolicy(.regular)
    }
    app.activate(ignoringOtherApps: true)
    return true
}

/// Pump the main CFRunLoop for up to `timeoutMs` milliseconds, returning
/// after the first source is handled. Returns `true` if a source was
/// handled, `false` on timeout.
///
/// Call this in a loop alongside a cancellation / outcome channel so the
/// AppKit run loop processes events (including GCD main-queue blocks)
/// without calling `NSApplication.run()`.
///
/// This function is re-entrant but **not** thread-safe — it must always be
/// invoked from the main thread.
@_cdecl("screen_capture_helper_pump_main_run_loop")
public func screenCaptureHelperPumpMainRunLoop(_ timeoutMs: Double) -> Bool {
    let timeout = timeoutMs / 1000.0
    let result = CFRunLoopRunInMode(.defaultMode, timeout, true)
    switch result {
    case .handledSource:
        return true
    default:
        return false
    }
}
