app-tagline = A quieter place to connect.
preview = Development preview
welcome = Sign in to your space
login-description = Connect to your Matrix homeserver with a password.
homeserver = Homeserver
username = Matrix ID or username
password = Password
sign-in = Sign in
signing-in = Connecting…
privacy-note = Use a test account. Recovery and device verification are not implemented yet.
privacy-detail = Each login creates a new device. Signing out revokes it; closing the window does not.
rooms = Rooms
room-search = Find a conversation…
choose-room = Your conversations, together.
choose-description = Select a room to read recent messages.
composer = Write a message…
send = Send
sending = Sending…
sign-out = Sign out
signing-out = Signing out…
refresh = Refresh
ready = Connected
syncing = Synchronizing…
reconnecting = Connection interrupted. Retrying…
loading = Loading recent messages…
empty-history = No text messages in the latest 50 events.
no-rooms = No conversations yet. Use + to join a room.
recent-only = Plain-text history · up to 500 events
unavailable-message = [Unsupported message]
login-failed = Could not sign in. Check your server,
    credentials and supported login method.
invalid-server = Enter an HTTPS homeserver address.
    Leave out credentials, queries and fragments.
missing-credentials = Enter your username and password.
load-failed = Could not load messages. Use Refresh to retry.
send-failed = Delivery is unconfirmed. Retry sends the same transaction; your draft is preserved.
logout-failed = Could not revoke this session. Check your connection and try signing out again.
queue-full = An operation is already queued. Please wait.
preview-fixture = Offline UI fixture · No Matrix connection
login-preview = Development preview · Test accounts only
login-safety = Saved sign-in uses your system keychain.
    Verification and recovery are still in development.
    Use Sign out to revoke this device.
temporary-session = Use a temporary session
retry-session = Retry saved sign-in
restoring-session = Opening your saved session…
temporary-notice = Temporary session · Sign-in is not saved
session-saved = Sign-in saved securely on this computer
vault-unavailable = Unlock your system keychain and retry,
    or use a temporary session.
vault-key-invalid = The saved key could not be read.
    Your saved sign-in has been kept.
vault-key-missing = This profile's key is missing from the keychain.
    Restore access to your keychain and retry.
storage-failed = Could not read or save the local profile.
    Check disk space and directory permissions.
profile-in-use = Archaic is already using this profile.
    Close the other instance and retry.
session-invalid = Could not open the saved session.
    Your saved sign-in has been kept.
session-store-missing = The saved encryption database is missing.
    Restore your profile backup before retrying.
session-exists = A saved session already exists. Restart
    Archaic to restore it, or use a temporary session.
session-not-saved = Signed in, but saving failed. This session is temporary.
    Sign out before closing to revoke this device.
logout-cleanup-failed = The server session was revoked, but local cleanup failed.
    Check disk space and permissions, then click Sign out again.
temporary-login-notice = Temporary session · Sign out before closing
    to revoke this device.
session-key-mismatch = The device keys do not match the saved session.
    Restore your profile backup before retrying.
session-expired = This session was revoked or expired. Use Sign out to clear
    the saved sign-in, then sign in again. Local keys are retained.

appearance = Appearance
theme-system = System
theme-light = Light
theme-dark = Dark
appearance-save-failed = Theme changed, but could not be saved.
join-room = Join a room
join-address = Room alias or ID
join = Join
joining = Joining…
cancel = Cancel
signed-in-as = Signed in as
join-invalid = Enter #room:server or !room:server.
join-forbidden = An invitation may be required.
    The server denied access.
join-not-found = Room not found.
    Check the alias and retry.
join-failed = Could not join. Check the address
    and connection, then retry.

invitation = Invitation
invited-by = Invited by
accept-invitation = Accept
decline-invitation = Decline
invite-user = Invite
invite-user-placeholder = @person:server
leave-room = Leave room
confirm = Confirm
leave-confirmation = Leave this room? Your unsent draft will be discarded.
    You may need an invitation to return. Older versions of this room are also left.
decline-confirmation = Decline this invitation? You may need a new invitation to return.
room-action-pending = Contacting the server…
room-left = Room removed from your conversations
room-unavailable = This room is no longer available. Refresh your conversations.
room-membership-changed = Your membership changed. Refresh and try again.
invite-invalid-user = Enter a complete Matrix ID, such as @person:server.
room-action-forbidden = The server denied this operation. Check your room permissions.
room-action-failed = The operation could not be confirmed. Check your connection and retry.
invite-sent = Invitation sent.

history-older = Load older
history-latest = Latest
history-available = Older messages available
history-start = Beginning of available history
history-gap = History gap: showing latest. Load older to continue.
history-limit = History limit reached. Latest resets the window.
history-load-failed = Could not load history. Retry Load older or Latest.
history-invalid-page = Invalid history page. Retry or return to Latest.
history-stalled = The server repeated a history cursor. Retry or return to Latest.
history-unavailable = History unavailable

new-short = New
new-conversation = New conversation
create-kind = Conversation type
create-direct = Direct chat
create-group = Private group
create-person = Matrix ID
create-name = Group name
create-topic = Topic (optional)
create-invitees = Invite people (optional, comma-separated Matrix IDs; up to 20)
create-encryption = New rooms are private and request encryption. Verification and recovery are still unfinished.
create-existing = Direct chats open an existing joined conversation when available, keeping its settings.
create-open = Open conversation
create-working = Opening…
create-forbidden = Your server does not allow this room to be created.
create-unconfirmed = Creation could not be confirmed. Check your room list before retrying; a room may already exist.
create-direct-tag-failed = Room created, but its direct-chat label could not be confirmed. The room is usable; do not recreate it.
create-invalid-name = Enter a group name (1–255 UTF-8 bytes, without control characters).
create-invalid-topic = The topic must be at most 4096 UTF-8 bytes and contain no NUL characters.
create-invalid-user = Enter complete Matrix IDs, such as @person:example.org.
create-self-invite = You are already included. Remove your own Matrix ID from the invitees.
create-too-many-invites = Invite at most 20 different people when creating a group.

message-select = Select a message for actions
message-reply = Reply
message-edit = Edit
message-delete = Delete
message-react = React
message-action-body = Reply, edited text, or reaction
message-apply = Confirm
message-delete-confirm = Delete your message? This cannot be undone.
reaction-hint = Emoji or short text. Confirm toggles your matching reaction.
message-edited = edited
message-deleted = Message deleted
message-action-done = Message action queued
message-action-failed = Could not complete the action. Your input is preserved; try again.
message-unavailable = The message is no longer available for this action.
message-forbidden = You do not have permission to change this message or reaction.
message-body-invalid = Enter between 1 and 32,000 bytes of non-blank text.
reaction-invalid = Enter a short reaction (up to 128 bytes), without control characters.

room-tools = Room tools
tool-files = Attachments
tool-search = Search messages
tool-details = Message details
tool-receipts = Read receipts
upload-hint = Choose a file, then send it. Maximum 25 MiB.
choose-file = Choose file…
upload-file = Send attachment
no-file = No file selected
search-local = Local history scan (including decrypted messages)
search-server = Server search (unencrypted rooms only)
search-scope = Search method
search-query = Find text in this room…
search-more = Continue search
search-open = Open result
search-results = Results
search-scanned = Events examined
search-hint = Search is limited to this room. Continue to scan older pages.
page-more = More pages available
page-complete = Search exhausted
details-load = Load / refresh details
details-more = More relations
reply-original = Replied-to message
download-file = Save attachment…
receipt-hint = Mark the selected message as read. Nothing is sent automatically.
receipt-private = Private — only my devices
receipt-public = Public — visible to the room
receipt-visibility = Receipt visibility
mark-read = Mark selected as read
tool-target = Selected message
tool-working = Working…
tool-ready = Ready
upload-done = Attachment sent
download-done = Attachment saved
receipt-done = Read receipt sent
search-done = Search page loaded
details-done = Message details loaded
details-hint = Select a message below, then load its details.
reply-unavailable = The replied-to message is unavailable. Refresh to retry.
read-by = Read by (known public receipts)
edit-history = Edit revisions
relations-loaded = Relations loaded
relations-complete = All server-returned relation pages loaded
page-stalled = The server repeated a page cursor. Refresh to restart.
tool-limit = This operation reached its safety limit; results are incomplete.
file-read-failed = Could not read the chosen file. Choose it again.
file-size-limit = Only regular files up to 25 MiB are supported.
file-save-failed = Could not save the attachment. Choose another destination.
file-exists = That destination already exists. Choose a new filename.
attachment-unavailable = No downloadable attachment is available for this message.
file-dialog-failed = Could not open the native file dialog.
search-invalid = Enter a search term of 1–1024 bytes.
search-restart = The selection or query changed. Start a new search or refresh details.
search-encrypted = Use local history scan for encrypted rooms; the query was not sent.
search-failed = Search failed. Retry with the same query.

events-unavailable = Events without readable content

security-title = Security & recovery
security-close = Back to conversations
security-devices = Devices & verification
security-recovery = Recovery & key backup
security-keys = Encrypted room-key files
security-verify = Verify selected device
security-accept = Accept request
security-start = Start comparison
security-confirm = They match
security-mismatch = They do not match
security-compare-hint = Confirm only after comparing every emoji or all three numbers on both devices.
security-verification-none = Select another device, or accept an incoming verification request.
security-verification-waiting = Waiting for the other device…
security-verification-requested = Incoming verification request
security-verification-ready = Ready to start comparison
security-verification-start = The other device started a comparison. Press Start comparison.
security-verification-compare = Compare the codes on both devices
security-verification-confirmed = Confirmed locally; waiting for the other device
security-verification-done = Verification completed
security-verification-cancelled = Verification cancelled or timed out
security-recovery-hint = Room keys are backed up encrypted on your homeserver.
    Unlock an existing backup with your recovery key, or set up a new one.
    Keep your recovery key safe; uploads run automatically.
security-bootstrap = Set up identity for a new account
security-recovery-key = Recovery key or recovery passphrase
security-recover = Unlock recovery
security-enable = Set up key backup & recovery
security-key-saved = I saved this key securely — hide it
security-keyfile-hint = Key files are encrypted with a passphrase of at least 12 characters.
    Choose a new destination file when exporting.
security-keyfile-password = Key-file passphrase
security-export = Export room keys…
security-import = Import room keys…
security-room-keys = Recover selected room keys from backup
security-recovery-enabled = Recovery enabled
security-recovery-disabled = Recovery not set up
security-recovery-incomplete = Recovery needs attention
security-recovery-unknown = Checking recovery state
security-identity = Identity keys
security-backup = Key backup
security-available = Available
security-missing = Not available
security-current = This device
security-verified = Verified
security-unverified = Unverified
security-flow-stale = This verification step is no longer available. Refresh and check the other device.
security-flow-active = Finish or cancel the active verification first.
security-device-invalid = Choose another available device.
security-failed = The security operation failed. Check the connection and retry.
security-recover-first = Recovery already exists. Unlock it with the existing key; nothing was reset.
security-bootstrap-first = Set up or recover your identity keys first.
security-secret-required = Enter the required password or recovery key.
security-recovery-failed = Recovery could not be unlocked. Check the key/passphrase and connection.
security-import-failed = Could not import the key file. Check its passphrase and format.
security-export-password = Use a key-file passphrase of at least 12 characters.

outbox-title = Outgoing messages
outbox-retry = Retry selected message
outbox-cancel = Cancel selected message
outbox-paused = Needs retry
outbox-queued = Queued / sending
outbox-failed = The outgoing queue could not be updated. Your queued messages have been kept.
outbox-stale = This message has already left the outgoing queue.

sso-login = Sign in with your browser (SSO)
sso-timeout = Browser sign-in timed out. Please try again.
sso-cancelled = Browser sign-in cancelled.
sso-browser-failed = Could not open the browser.

activity-typing = typing…
activity-share-typing = Share typing status
activity-private-read = Mark read privately while composing

timeline-messages = Messages
timeline-select = Select a message to inspect it or use the actions below.

message-thread = Reply in thread
message-forward = Forward text
message-forward-hint = Enter the destination room ID (!room:server). The selected text will be sent with its original sender.
message-forward-text-only = Only text messages can be forwarded here.
thread-replies = Thread replies (loaded relations)
thread-reply = Thread reply

history-newer = Newer messages
message-context = Show surrounding messages

sso-unavailable = This homeserver does not advertise browser SSO. Use a supported sign-in method.

oauth-login = Sign in with OAuth / OIDC
oauth-unavailable = OAuth discovery or client registration failed. Your server may require an administrator-registered desktop client.
oauth-callback-failed = The browser authorization could not be verified. Try signing in again.

local-data-invalid = Encrypted local data could not be opened. It has not been overwritten.
draft-save-failed = Could not save the draft. Keep this window open and retry.

receipt-thread = Mark read inside this thread only

reauthenticate = Resume this device
reauth-failed = Reauthentication failed. Check your password and retry.
reauth-unavailable = This session cannot be resumed with a password.

search-indexed = Encrypted index (offline search)
search-filter-hint = Indexed search: text from:@user:server has:file after:TIMESTAMP before:TIMESTAMP · More indexes older history

transfer-busy = A transfer is already running.
transfer-cancelled = Transfer cancelled. A completed server operation cannot be recalled.
cancel-transfer = Cancel transfer
preview-file = Preview image
preview-ready = Image verified and decoded
upload-queued = Attachment queued. Progress, retry and cancel are available in the outgoing queue.
preview-invalid = Unsupported image or image dimensions exceed the safe preview limit.

organization = Rooms & organization
org-fields = Target: room/space ID. Value: query, setting, user or child ID. Extra: topic, power level or reason.
org-target = Target room / space ID
org-value = Value / user / search
org-extra = Extra / power level / reason
org-confirm = Apply this change to the named target
org-run = Run selected action
org-select = Use selected result
org-confirm-required = Confirm the target and change before applying.
organization-invalid = Invalid room administration input.
organization-failed = Could not read room permissions or settings.
org-directory = Browse public directory
org-spaces = Joined spaces
org-hierarchy = Browse space hierarchy
org-inspect = Room settings & permissions
org-members = Room members
org-create-space = Create space
org-name = Change room name
org-topic = Change room topic
org-join-rule = Join rule: invite / public / knock
org-history = History: joined / invited / shared / world_readable
org-guests = Guests: forbidden / can_join
org-power = Set user power level
org-invite = Invite user
org-kick = Kick user
org-ban = Ban user
org-unban = Unban user
org-favorite = Add to favorites
org-unfavorite = Remove favorite
org-block = Block user
org-unblock = Unblock user
org-blocked = Blocked users
org-add-child = Add space child
org-remove-child = Remove space child

profile-name = Account profile name
profile-switch = Switch account
profile-invalid = Use 1–40 letters, numbers, underscores or hyphens for a profile name. Each profile has separate encrypted storage.

matrix-link = Paste matrix: or matrix.to link
open-link = Open Matrix link
link-invalid = This is not a valid Matrix link.
desktop-notifications = Desktop notifications (no message text)
notification-activity = New activity in

shortcut-rooms = Find a conversation
shortcut-navigation = Navigate

thread-read-by = Read in this thread by

shutdown-failed = Could not confirm the final draft save.
shutdown-timeout = Timed out waiting for the final draft save.
search-truncated = Showing 1,000 matches; narrow your filters.

desktop-show = Show Archaic
desktop-quit = Quit Archaic
desktop-no-unread = No unread activity
desktop-unread = Unread activity
desktop-tray-unavailable = The desktop status item is unavailable on this desktop.

profile-saving = Saving drafts before switching accounts…

org-alias-add = Create room alias
org-alias-remove = Remove room alias
org-canonical = Set canonical alias
org-publish = Publish in directory
org-unpublish = Unpublish from directory
org-low-priority = Mark low priority
org-normal-priority = Remove low priority
org-avatar = Upload room avatar (file path)
org-clear-avatar = Remove room avatar
org-restricted = Restrict membership to rooms
org-event-power = Set event permission
org-retention = Set retention (milliseconds)
org-upgrade = Upgrade room version
org-report = Report event
org-redact = Redact event as moderator
org-knock = Request to join (knock)
org-parent-add = Add canonical parent space
org-parent-remove = Remove parent space
org-threads = Browse threads
org-participated-threads = Threads I participated in
org-subscribe-thread = Subscribe to thread
org-unsubscribe-thread = Unsubscribe from thread

profile-saved = Saved account profiles
desktop-preference-failed = Could not save desktop preferences. Your current selection remains active.

daylight-home = Home
daylight-account = Account & preferences
daylight-about = About this room

daylight-members = Members
daylight-encryption = Encryption
daylight-encrypted = E2EE
daylight-unencrypted = None
daylight-unknown = Unknown

daylight-message-actions = Message actions
daylight-copy = Copy message

daylight-thread = Thread
daylight-thread-reply = Reply in thread…
daylight-thread-empty = Start the conversation with a reply.

daylight-emoji = Emoji
daylight-emoji-search = Search names, :shortcodes: or paste an emoji…
daylight-emoji-empty = No matches. Paste any Unicode emoji above.
daylight-emoji-insert = Use typed emoji

encrypted-message = Encrypted message · Unlock history in Security & recovery

daylight-reactions = Reactions
daylight-emoji-more = More emojis

direct-messages = Direct messages
members-loading = Loading…
settings-title = Settings
settings-account = Account
settings-notifications = Notifications & privacy
settings-advanced = Advanced
sidebar-toggle = Collapse or expand conversations

security-settings-description = Verify devices, restore encrypted history, and manage recovery keys.

favorites = Favorites

profile-back = ← Room details
profile-role = Role
profile-identity = Identity
profile-verified = Verified
profile-unverified = Unverified
profile-admin = Admin
profile-moderator = Moderator
profile-member = Member
profile-message = Send message
profile-receipt = Jump to read receipt
profile-share = Copy profile link
profile-mention = Mention
profile-copy = Copy Matrix ID
profile-ignore = Ignore user
profile-unignore = Stop ignoring
link-open-failed = Could not open this link in your browser.

message-layout = Message layout
message-bubbles = Bubbles
message-compact = Compact

settings-account-hint = Your identity and saved accounts.
settings-appearance-hint = Make Archaic feel like yours.
settings-notifications-hint = Choose how Archaic gets your attention.
settings-advanced-hint = Links and desktop tools.

emoji-previous = Previous
emoji-next = Next

history-sync-title = Background history sync
history-sync-waiting = Waiting for the initial sync…
history-sync-temporary = Temporary session: background history storage is off.
history-sync-rooms = Rooms fully synced
history-sync-events = Events cached locally
history-sync-encrypted = Encrypted events awaiting keys
history-sync-retrying = Rooms retrying after an error

composer-more = More message options
composer-sticker = Sticker
composer-voice = Voice message
composer-poll = Poll
poll-question = Question
poll-answers = Answers — one per line (2–20)
poll-invalid = Enter a question and 2–20 different answers.
voice-hint = Record up to two minutes. Your microphone is used only after you press Record.
voice-record = Record
voice-stop = Stop recording
voice-recording = Recording
voice-ready = Ready to send
voice-unavailable = The microphone is unavailable or this audio format is unsupported.
voice-invalid = The recording is empty or invalid.

account-main = Main account
account-saved = Saved accounts on this device
account-add = Another account
account-open = Open account
account-slot-placeholder = e.g. work
account-slot-hint = Choose a local name for another account.
    Your Matrix display name stays the same.

security-backup-active = Encrypted key backup is enabled on this device.
security-backup-locked = A key backup exists. Unlock it with your recovery key below.
security-backup-none = No server key backup is set up yet.
security-backup-unknown = Could not check the server key backup. Try Refresh.
security-backup-unlock-first = A backup already exists. Unlock it with your recovery key first.

message-read-by = Read by

delivery-sending = Sending…
delivery-delivered = Delivered to server
delivery-failed = Could not send. Click to retry or cancel.

# Shared CUI chat labels. Literal placeholders are expanded by CUI.
chat-message = Message
chat-send = Send
chat-context = Reply or edit context
chat-attachments = Attachments
chat-cancel-context = Cancel reply or edit
chat-attach = Attach file
chat-emoji = Emoji
chat-create-poll = Create poll
chat-more = More message options
chat-composer-help = Enter sends; Shift+Enter inserts a newline; Escape cancels
chat-editing = Editing message · Esc to cancel
chat-replying = Replying to { "{author}" } · { "{preview}" }
chat-thread-one = { "{count}" } reply in thread →
chat-thread-many = { "{count}" } replies in thread →
chat-reply-one = { "{count}" } reply
chat-reply-many = { "{count}" } replies
chat-vote-one = { "{count}" } vote
chat-vote-many = { "{count}" } votes
chat-poll-closed = Poll closed
chat-poll-select = Select an option
chat-poll-tap = Tap an option to vote
chat-poll-results = Vote to see results
chat-send-failed = Failed to send
chat-delivered = Delivered
chat-sending = Sending
chat-conversation-pane = Conversation pane { "{count}" }
text-size = Text size
keyboard-shortcuts = Keyboard shortcuts
navigation-spaces = Spaces
