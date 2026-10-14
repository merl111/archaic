import Foundation
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let mode = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "window"
let all = mode == "all"
let windows = CGWindowListCopyWindowInfo(all ? [.optionAll] : [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
if all {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: windows, options: [.prettyPrinted, .sortedKeys]))
    exit(0)
}
let matches = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }
guard let window = matches.max(by: { a, b in
    let aa = a[kCGWindowBounds as String] as? [String: Double] ?? [:]
    let bb = b[kCGWindowBounds as String] as? [String: Double] ?? [:]
    return (aa["Width"] ?? 0) * (aa["Height"] ?? 0) < (bb["Width"] ?? 0) * (bb["Height"] ?? 0)
}) else { fputs("No visible Archaic window\n", stderr); exit(1) }
if mode == "settings" {
    guard CGPreflightPostEventAccess() else { fputs("Runner does not grant synthetic input access\n", stderr); exit(2) }
    let bounds = window[kCGWindowBounds as String] as! [String: Double]
    // The right-aligned Settings button observed in the native chat captures.
    let point = CGPoint(x: bounds["X"]! + bounds["Width"]! - 50, y: bounds["Y"]! + 54)
    CGEvent(mouseEventSource: nil, mouseType: .leftMouseDown, mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
    CGEvent(mouseEventSource: nil, mouseType: .leftMouseUp, mouseCursorPosition: point, mouseButton: .left)?.post(tap: .cghidEventTap)
    exit(0)
}
let data = try JSONSerialization.data(withJSONObject: window, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
