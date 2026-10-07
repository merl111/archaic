//! Application translations for the shared native chat components.
use archaic_i18n::Translator;
use cui::{ChatComponent, Result, sys};
pub fn apply(chat: &ChatComponent, tr: &Translator) -> Result<()> {
    for (key, label) in [
        (sys::CUI_CHAT_LABEL_MESSAGE, "chat-message"),
        (sys::CUI_CHAT_LABEL_SEND, "chat-send"),
        (sys::CUI_CHAT_LABEL_CONTEXT, "chat-context"),
        (sys::CUI_CHAT_LABEL_ATTACHMENTS, "chat-attachments"),
        (sys::CUI_CHAT_LABEL_CANCEL_CONTEXT, "chat-cancel-context"),
        (sys::CUI_CHAT_LABEL_ATTACH, "chat-attach"),
        (sys::CUI_CHAT_LABEL_EMOJI, "chat-emoji"),
        (sys::CUI_CHAT_LABEL_CREATE_POLL, "chat-create-poll"),
        (sys::CUI_CHAT_LABEL_MORE, "chat-more"),
        (sys::CUI_CHAT_LABEL_COMPOSER_HELP, "chat-composer-help"),
        (sys::CUI_CHAT_LABEL_EDITING, "chat-editing"),
        (sys::CUI_CHAT_LABEL_REPLYING, "chat-replying"),
        (sys::CUI_CHAT_LABEL_THREAD_ONE, "chat-thread-one"),
        (sys::CUI_CHAT_LABEL_THREAD_MANY, "chat-thread-many"),
        (sys::CUI_CHAT_LABEL_REPLY_ONE, "chat-reply-one"),
        (sys::CUI_CHAT_LABEL_REPLY_MANY, "chat-reply-many"),
        (sys::CUI_CHAT_LABEL_VOTE_ONE, "chat-vote-one"),
        (sys::CUI_CHAT_LABEL_VOTE_MANY, "chat-vote-many"),
        (sys::CUI_CHAT_LABEL_POLL_CLOSED, "chat-poll-closed"),
        (sys::CUI_CHAT_LABEL_POLL_SELECT, "chat-poll-select"),
        (sys::CUI_CHAT_LABEL_POLL_TAP, "chat-poll-tap"),
        (sys::CUI_CHAT_LABEL_POLL_RESULTS, "chat-poll-results"),
        (sys::CUI_CHAT_LABEL_SEND_FAILED, "chat-send-failed"),
        (sys::CUI_CHAT_LABEL_DELIVERED, "chat-delivered"),
        (sys::CUI_CHAT_LABEL_SENDING, "chat-sending"),
        (
            sys::CUI_CHAT_LABEL_CONVERSATION_PANE,
            "chat-conversation-pane",
        ),
    ] {
        chat.set_label(key, Some(&tr.text(label)))?;
    }
    Ok(())
}
