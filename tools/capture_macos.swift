import Foundation
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let all = CommandLine.arguments.count > 2
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
let data = try JSONSerialization.data(withJSONObject: window, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(data)
