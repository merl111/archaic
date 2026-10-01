# CUI changes required by Archaic

Baseline: CUI commit `6249092`, October 1, 2026. These are proposed framework work packages, not existing APIs. CUI remains a separate reusable C library. Each addition includes ownership/error contracts, platform implementations, all language bindings, native tests and documentation before Archaic updates its dependency pin.

## Existing foundation we can use

The current headers provide native windows, controls, split/grid/wrapping layouts, commands/menus, file dialogs, trees, editable tables, search/pickers, tooltips, icons, images, themes/fonts/scaling, drawing surfaces and window panels. Native text provides a baseline for shaping and IME. GTK trees/tables/search results have virtualization, but that does not provide a generic variable-height chat timeline.

Evidence: [public core API](../../cui/include/cui.h), [desktop API](../../cui/include/cui_desktop.h), [drawing API](../../cui/include/cui_draw.h), [component limitations](../../cui/docs/components.md), [Rust lifetime/thread rules](../../cui/docs/guides/rust.md), [accessibility limitations](../../cui/docs/guides/accessibility.md).

The current chat composition uses plain copied text and reusable slots. It lacks rich messages and virtualized history. Rust callback storage lasts until app destruction, and widget handles are app-owned. Windows/macOS native verification remains deferred. These constraints matter for a client intended to stay open for days.

## Work packages

| ID | Capability | Present state and required work | Archaic need | Exit evidence |
| --- | --- | --- | --- | --- |
| CUI-01 | UI-thread dispatch | Timers exist; no worker-safe dispatcher. Add a narrow thread-safe enqueue/wakeup endpoint with owned payload cleanup, cancellation and shutdown guarantees. Ordinary CUI handles stay main-thread-only. | SDK events, image results, network progress | Worker floods coalesce safely; closing app/views frees queued data; no callbacks after destruction; backend wakeups tested |
| CUI-02 | Disposable widget and callback lifetimes | App-owned widgets and retained callback data can grow over long sessions. Design subtree disposal, subscriptions and callback destructors with safe invalidation of Rust and other binding handles. | Room switching, recycled rows, temporary dialogs | Repeated open/close and recycle tests plateau in allocation count; no stale-handle use or callback reentrancy UAF |
| CUI-03 | Virtualized variable-height collection | Tables and trees are insufficient for arbitrary message rows. Add stable row IDs, incremental diffs, row reuse, measured heights, overscan, independent scrolling, scroll-to-ID and anchor preservation. | Room list, history, threads, search, member lists | 100,000-row fixture only realizes the visible region; prepend/edits/media resize preserve viewport; keyboard and accessibility work |
| CUI-04 | Rich document view | Plain labels/textareas and drawing text cannot deliver full message semantics. Add typed inline/block runs, links, selectable text, lists, quotes, code, spoiler spans, inline images and accessible structure. | Matrix formatted messages and previews | Safe link activation and copy; Unicode/bidi selection; streamed edits; useful screen-reader hierarchy; no HTML/JS execution |
| CUI-05 | Composer editing contract | Native text entry exists; structured editing, selection ranges and rich composing need an explicit cross-platform API. Add caret/range queries, insertion/replacement, composition state, attributed spans, undo grouping and format commands. | Mentions, emoji, drafts, formatted composition, edit messages | Grapheme-aware selection; CJK IME Enter does not send early; undo across edits; paste strips unsupported content predictably |
| CUI-06 | Anchored overlays and focus | Window panels exist. Audit/extending them is preferable to a second unrelated popup system. Add reliable anchor/focus/escape/click-away behavior suitable for child views. | Emoji/reaction pickers, mention suggestions, message menus | Resize, DPI changes, scrolling and window movement retain correct placement; keyboard focus restores |
| CUI-07 | Clipboard and drag/drop data | Public clipboard support is text-write; no general typed paste/drop contract. Add async reads, text/HTML/image/file representations, accepted effects and cancellation. | Pasting screenshots, file upload, dragging saved attachments | Native clipboard/drop tests on all platforms; bounded data transfer; no implicit network execution |
| CUI-08 | Images and presentation surfaces | RGBA images and static decoded icons exist; bounded async media lifecycle, scaling/cropping and interactive preview ergonomics need work. Add reusable viewport/avatar/image presentation and frame updates as required. | Avatars, attachments, zoomable gallery, video frames | DPI-correct images, stale response cancellation, keyboard zoom and accessible descriptions; bounded caches |
| CUI-09 | Desktop application integration | In-layout toasts are not OS notifications. Add reusable native notifications/actions, app badges, URI open events and tray/status-item hooks where platform conventions support them. | Unread counts, notification navigation, links, background client | Activation opens correct account/room/event; missing Linux services degrade clearly; no leaked sensitive text under privacy settings |
| CUI-10 | Localization hooks | UTF-8 exists, but compositions contain English labels such as Send/Copy/Previous. Add a stable built-in message resolver or configurable labels and locale-change invalidation. | Fully translated controls and app chrome | No hardcoded visible English in translatable library controls; stable IDs independent of text; no Fluent dependency in CUI |
| CUI-11 | Logical-direction layout | Complete RTL behavior is not established. Add inherited start/end direction, alignment/margin semantics, correct focus order, popup anchors and per-text bidi direction. | Arabic/Hebrew and mixed-script messages | RTL UI with LTR URLs/code/MXIDs; mirrored navigation where appropriate; native text direction tested |
| CUI-12 | Accessibility semantics | Basic names/help exist; rich custom content, composed tabs and virtualized views need stronger roles/state/action exposure. | Timeline reading, composer, device verification, calls | AT-SPI/Orca, UIA/Narrator and macOS AX/VoiceOver acceptance tests; non-disruptive live updates and focus preservation |
| CUI-13 | Theme/system preferences | Existing theme, font, DPI and semantic roles are useful. Audit high contrast, reduced motion, focus visuals, long labels and per-monitor changes; add missing preference notifications. | Professional native light/dark experience | 100–300% scale and large text, high contrast, reduced motion; no clipped or inaccessible actions |
| CUI-14 | Event and model ergonomics | Extend typed events and update batching where needed; avoid rebuilding entire widget trees or replacing callbacks each sync. | Frequent room/timeline diffs | One logical update causes bounded work, selection preserved, no quadratic updates or growing callback arena |

## Ordering and API discipline

P0 validates designs for CUI-01/02/03/04/05/10/11. P1 implements the minimum coherent versions needed by the first real client. Clipboard, presentation and desktop integration grow in P2–P4; accessibility and localization are tested continuously, not bolted on at release.

CUI-02 is the highest ownership risk: a widget destroyed before its app cannot be validated by the current app-only weak lifetime check. Design a generation/identity or equivalent lifetime mechanism first, including nested widgets, in-flight callbacks and retained foreign handles. Never add a raw destroy function and leave safe bindings capable of dereferencing freed pointers.

CUI-01 is a deliberate exception to the main-thread rule for a dispatcher endpoint only. Specify which side owns each payload, which thread destroys it, what enqueue returns during shutdown, and how panics/exceptions are contained at language boundaries. Keep Python/Go/Zig/Rust closure cleanup consistent with the C contract.

For CUI-04/05, use native text machinery where it preserves shaping, input methods, selection and accessibility. A single giant image/canvas for the whole conversation is not an acceptable substitute. Matrix HTML parsing/sanitization belongs in Archaic; CUI receives a safe typed document model. Generic document/layout operations remain protocol-independent.

Names, structures and signatures are intentionally not fixed in this plan. Specify additive ABI changes and versioning before implementation, then update the generated Rust raw bindings and all convenience bindings with matching contract tests. Windows/AppKit/GTK capability differences must be explicit.

## What must stay out of CUI

Matrix event types, HTTP/TLS, accounts, tokens, encryption keys, SDK adapters, database schemas, locale catalogs for Archaic, network image fetching, URL preview policy, audio codecs/WebRTC/LiveKit and widget permission policy belong in Archaic. CUI may expose generic media display and desktop events without becoming a media/network framework.

A widget compatibility host, if selected, belongs in Archaic's optional integration layer. Do not make WebKit/Chromium a dependency of CUI or silently replace native controls with web views.

## Verification policy

Keep existing CUI sanitizers, binding checks and docs generation in place. Add tests around lifetime, diff, scroll, Unicode and platform-specific semantics that this client exercises. Linux native tests can run immediately. The user's deferral of Windows/macOS runtime testing is respected during development; a production Archaic release on those OSes still requires their actual native verification.
