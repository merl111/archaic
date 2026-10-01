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
rooms = Conversations
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
unavailable-message = [Encrypted or unsupported message]
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
message-action-done = Message action completed
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
