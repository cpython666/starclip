// Read window metadata for the runner-owned PID. No screen capture, UI actions,
// accessibility permissions, clipboard reads, or application test hooks.
import AppKit
import CoreGraphics

guard CommandLine.arguments.count == 2,
      let pid = Int32(CommandLine.arguments[1]), pid > 0 else {
    fatalError("Expected the smoke application's PID")
}
let app = NSRunningApplication(processIdentifier: pid)
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    as? [[String: Any]] ?? []
let visible = windows.contains { window in
    guard window[kCGWindowOwnerPID as String] as? Int32 == pid,
          window[kCGWindowLayer as String] as? Int == 0,
          let bounds = window[kCGWindowBounds as String] as? [String: CGFloat] else { return false }
    return (bounds["Width"] ?? 0) >= 860 && (bounds["Height"] ?? 0) >= 620
}
let ready = app?.isFinishedLaunching == true && app?.isTerminated == false && visible
print("{\"window\":\(ready ? "true" : "false")}")
